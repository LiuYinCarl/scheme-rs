//! Tree-walking evaluator with an explicit, persistent continuation stack
//! (trampoline): proper tail calls, first-class multi-shot continuations,
//! and dynamic-wind with correct re-entry semantics.
//!
//! # 设计要点（为什么这么做）
//!
//! 求值器不用 Rust 原生递归表达 Scheme 层的控制流，而是把求值压平成一个
//! 循环（trampoline）。原因是 R5RS 的两个硬性要求：
//!
//! 1. 正确的尾递归（proper tail recursion）：尾位置的过程调用必须在常数
//!    栈空间内完成。如果用 Rust 函数递归求值，Scheme 的尾调用会消耗
//!    Rust 栈，深度稍大就爆栈。
//! 2. 一等续延（call/cc，可多次重入）：捕获续延必须便宜，且捕获后不能被
//!    后续求值破坏。
//!
//! 做法：机器状态 = 当前 State + 一条显式的续延帧链表（`Cont`）。帧链表
//! 是 persistent（不可变共享）结构：resume 一个帧时并不原地修改它，而是
//! 克隆其中的数据、构造新帧压回栈上。因此"捕获续延"只需保存当前栈指针
//! （Rc 克隆，O(1)），"调用续延"只需把整根栈指针换回去；同一份续延可以
//! 被多次、交错地重入（pitfall 7.1–7.4 依赖这一点）。

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;

use crate::env::{aux_name, lookup_var, resolve, Env, Meaning};
use crate::printer::write_to_string;
use crate::reader;
use crate::syntax_rules;
use crate::value::{
    cons, gensym, intern, list_from_vec, list_to_vec, proper_list, sym_str, Pair, Promise, Sym,
    Value,
};

// ---------------------------------------------------------------------------
// Continuation infrastructure

/// 续延栈：persistent 单链表，None 表示"停机"（把值返回给 run 的调用者）。
pub type Cont = Option<Rc<ContFrame>>;

pub struct ContFrame {
    pub kind: ContKind,
    pub parent: Cont,
}

pub struct DynWind {
    pub before: WindHook,
    pub after: WindHook,
}

/// dynamic-wind 的 before/after 钩子：多数是 Scheme 层 thunk；端口切换
/// （with-input-from-file/with-output-to-file）是原生操作，没有对应的
/// Scheme 过程可调用，用专用钩子挂在同一套 wind 机制上，从而自动获得
/// 逃逸/重入时的正确执行时机。
#[derive(Clone)]
pub enum WindHook {
    Thunk(Value),
    /// 进入动态范围：保存当前端口，切换到新端口。
    PortEnter(Rc<PortSwitch>),
    /// 离开动态范围：恢复保存的端口，并关闭新端口（close 内含 flush，
    /// 写文件不丢数据）。
    PortLeave(Rc<PortSwitch>),
}

pub struct PortSwitch {
    pub is_input: bool,
    pub new_port: Rc<crate::port::Port>,
    /// 每次进入时保存的当时端口（重入时 before 会再次执行，必须重新保存）。
    pub saved: RefCell<Option<Rc<crate::port::Port>>>,
}

/// 执行一个 wind 钩子。Scheme thunk 走 Apply；原生端口钩子直接生效，
/// 返回 Unspecified 交给续延（效果等同于一个立即返回的 thunk）。
pub fn apply_hook(hook: &WindHook) -> State {
    match hook {
        WindHook::Thunk(v) => State::Apply(v.clone(), vec![]),
        WindHook::PortEnter(sw) => {
            let saved = if sw.is_input {
                crate::port::current_input()
            } else {
                crate::port::current_output()
            };
            *sw.saved.borrow_mut() = Some(saved);
            if sw.is_input {
                crate::port::set_current_input(sw.new_port.clone());
            } else {
                crate::port::set_current_output(sw.new_port.clone());
            }
            State::Return(Value::Unspecified)
        }
        WindHook::PortLeave(sw) => {
            if let Some(saved) = sw.saved.borrow_mut().take() {
                if sw.is_input {
                    crate::port::set_current_input(saved);
                } else {
                    crate::port::set_current_output(saved);
                }
            }
            sw.new_port.close();
            State::Return(Value::Unspecified)
        }
    }
}

/// dynamic-wind 的动态环境也是 persistent 链表。每个节点记录深度，
/// 恢复续延时可以据此在 O(深度差) 内找到两条链的公共后缀。
pub struct WindNode {
    pub wind: Rc<DynWind>,
    pub parent: WindList,
    pub depth: u32,
}

pub type WindList = Option<Rc<WindNode>>;

/// 被捕获的续延 = 续延栈指针 + 当时的 dynamic-wind 链，捕获是 O(1)。
pub struct ContObj {
    pub cont: Cont,
    pub winds: WindList,
}

fn wind_depth(w: &WindList) -> u32 {
    w.as_ref().map(|n| n.depth).unwrap_or(0)
}

fn wind_push(wind: Rc<DynWind>, w: &WindList) -> WindList {
    Some(Rc::new(WindNode {
        wind,
        depth: wind_depth(w) + 1,
        parent: w.clone(),
    }))
}

fn wind_eq(a: &WindList, b: &WindList) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(x), Some(y)) => Rc::ptr_eq(x, y),
        _ => false,
    }
}

/// Compute afters (innermost first) to exit and befores (outermost first) to
/// enter when moving the dynamic environment from `cur` to `tgt`.
///
/// 经典的 common-tail 算法：两条 wind 链都是从同一根（None）长出来的
/// persistent 链，必有公共后缀（共享尾部节点，可用指针相等判断）。每次
/// 弹出较深链的表头（深度相等时两边交替弹），直到两链指针相等。cur 侧
/// 弹出的节点按"由内到外"收集 after thunk；tgt 侧弹出的节点收集后反转，
/// 按"由外到内"收集 before thunk。
fn wind_diff(cur: &WindList, tgt: &WindList) -> (Vec<Rc<DynWind>>, Vec<Rc<DynWind>>) {
    let mut a = cur.clone();
    let mut b = tgt.clone();
    let mut afters = Vec::new();
    let mut befores = Vec::new();
    while !wind_eq(&a, &b) {
        let da = wind_depth(&a);
        let db = wind_depth(&b);
        if da >= db {
            let n = a.unwrap();
            afters.push(n.wind.clone());
            a = n.parent.clone();
        } else {
            let n = b.unwrap();
            befores.push(n.wind.clone());
            b = n.parent.clone();
        }
    }
    befores.reverse();
    (afters, befores)
}

pub enum ContKind {
    If {
        conseq: Value,
        alt: Option<Value>,
        env: Rc<Env>,
    },
    Begin {
        rest: Vec<Value>,
        env: Rc<Env>,
    },
    Define {
        name: Sym,
        env: Rc<Env>,
    },
    Set {
        name: Sym,
        env: Rc<Env>,
    },
    OpDone {
        operands: Vec<Value>,
        env: Rc<Env>,
    },
    ArgDone {
        proc: Value,
        collected: Vec<Value>,
        pending: Vec<Value>,
        env: Rc<Env>,
    },
    And {
        rest: Vec<Value>,
        env: Rc<Env>,
    },
    Or {
        rest: Vec<Value>,
        env: Rc<Env>,
    },
    BodyInit {
        temps: Vec<Value>,
        pending: Vec<Value>,
        locs: Rc<Vec<Rc<RefCell<Value>>>>,
        body: Vec<Value>,
        env: Rc<Env>,
    },
    DynWindBefore {
        before: WindHook,
        thunk: Value,
        after: WindHook,
    },
    DynWindBody {
        wind: Rc<DynWind>,
    },
    DynWindAfter {
        value: Value,
    },
    WindSteps {
        steps: Vec<(WindHook, WindList)>,
        value: Value,
        target: Rc<ContObj>,
    },
    Force {
        promise: Rc<RefCell<Promise>>,
    },
    Map {
        f: Value,
        lists: Vec<Value>,
        collected: Vec<Value>,
    },
    ForEach {
        f: Value,
        lists: Vec<Value>,
    },
    CallWithValues {
        consumer: Value,
    },
    Load {
        rest: Vec<Value>,
        env: Rc<Env>,
    },
    ClosePortAfter {
        port: Rc<crate::port::Port>,
    },
    GetOutputString {
        port: Rc<crate::port::Port>,
    },
    /// trace 用的透传帧：被跟踪过程返回时打印结果（缩进即调用深度）。
    TraceReturn {
        depth: usize,
    },
}

pub enum State {
    /// 待求值的表达式及其词法环境。
    Eval(Value, Rc<Env>),
    /// 一个值已经算出来，要交付给栈顶续延帧（弹帧并 resume）。
    Return(Value),
    /// 运算符与全部操作数都已求值完毕，执行一次过程应用。
    /// 进入 Apply 时驱动参数求值的那些帧已经被弹出，所以闭包应用的
    /// body 直接在调用者的续延下求值——这正是尾调用不压栈的关键。
    Apply(Value, Vec<Value>),
}

pub struct Machine {
    pub cont: Cont,
    pub winds: WindList,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

impl Machine {
    pub fn new() -> Machine {
        Machine {
            cont: None,
            winds: None,
        }
    }

    pub fn push(&mut self, kind: ContKind) {
        let parent = self.cont.take();
        self.cont = Some(Rc::new(ContFrame { kind, parent }));
    }
}

// ---------------------------------------------------------------------------
// Main trampoline
//
// 主循环只有三件事：求值一个表达式（Eval）、把值交给续延（Return）、
// 应用一个过程（Apply）。Rust 层面没有任何随 Scheme 调用深度增长的递归，
// 所以 Scheme 的尾递归只消耗堆上的帧对象，不消耗 Rust 栈。

pub fn run(state: State) -> Result<Value, String> {
    let mut m = Machine::new();
    let mut state = state;
    loop {
        state = match state {
            State::Eval(expr, env) => eval_step(&mut m, expr, env)?,
            State::Return(v) => match m.cont.take() {
                None => return Ok(v),
                Some(frame) => {
                    m.cont = frame.parent.clone();
                    resume(&mut m, &frame.kind, v)?
                }
            },
            State::Apply(p, args) => apply(&mut m, p, args)?,
        };
    }
}

fn eval_step(m: &mut Machine, expr: Value, env: Rc<Env>) -> Result<State, String> {
    match &expr {
        Value::Symbol(s) => match lookup_var(&env, *s) {
            Some(loc) => Ok(State::Return(loc.borrow().clone())),
            None => Err(crate::value::unbound_msg(*s)),
        },
        Value::Pair(p) => {
            let head = p.borrow().0.clone();
            if let Value::Symbol(s) = &head {
                match resolve(&env, *s) {
                    Meaning::Macro(t) => {
                        let out = syntax_rules::expand(&t, &expr, &env)?;
                        Ok(State::Eval(out, env))
                    }
                    Meaning::Keyword(k) => special(m, k, &expr, env),
                    Meaning::Var(_) => combination(m, &expr, env),
                    Meaning::Unbound => Err(crate::value::unbound_msg(*s)),
                }
            } else {
                combination(m, &expr, env)
            }
        }
        _ => Ok(State::Return(expr.clone())),
    }
}

fn combination(m: &mut Machine, expr: &Value, env: Rc<Env>) -> Result<State, String> {
    let (mut items, rest) = match list_to_vec(expr) {
        Some(x) => x,
        None => return Err("circular form".into()),
    };
    if !rest.is_nil() {
        return Err(format!("malformed application: {}", write_to_string(expr)));
    }
    let head = items.remove(0);
    m.push(ContKind::OpDone {
        operands: items,
        env: env.clone(),
    });
    Ok(State::Eval(head, env))
}

/// Evaluate a sequence; the last form is in tail position.
///
/// 尾位置的实现方式：最后一个形式不压 Begin 帧，直接以当前续延 Eval
/// （帧被"替换"而非"叠加"），所以 begin/body/闭包体的尾调用都不耗栈。
fn seq(m: &mut Machine, forms: Vec<Value>, env: Rc<Env>) -> State {
    let mut forms = forms;
    match forms.len() {
        0 => State::Return(Value::Unspecified),
        1 => State::Eval(forms.remove(0), env),
        _ => {
            let rest = forms.split_off(1);
            m.push(ContKind::Begin {
                rest,
                env: env.clone(),
            });
            State::Eval(forms.remove(0), env)
        }
    }
}

fn resume(m: &mut Machine, kind: &ContKind, v: Value) -> Result<State, String> {
    match kind {
        ContKind::If { conseq, alt, env } => {
            if v.is_truthy() {
                Ok(State::Eval(conseq.clone(), env.clone()))
            } else {
                match alt {
                    Some(a) => Ok(State::Eval(a.clone(), env.clone())),
                    None => Ok(State::Return(Value::Unspecified)),
                }
            }
        }
        ContKind::Begin { rest, env } => Ok(seq(m, rest.clone(), env.clone())),
        ContKind::Define { name, env } => {
            env.define(*name, v);
            Ok(State::Return(Value::Unspecified))
        }
        ContKind::Set { name, env } => match lookup_var(env, *name) {
            Some(loc) => {
                *loc.borrow_mut() = v;
                Ok(State::Return(Value::Unspecified))
            }
            None => Err(format!("set!: {}", crate::value::unbound_msg(*name))),
        },
        ContKind::TraceReturn { depth } => {
            // 被跟踪过程返回：打印结果后原样透传给下一帧
            println!("{}{}", "| ".repeat(*depth), write_to_string(&v));
            Ok(State::Return(v))
        }
        ContKind::OpDone { operands, env } => {
            let proc = v;
            if operands.is_empty() {
                Ok(State::Apply(proc, vec![]))
            } else {
                let mut ops = operands.clone();
                let first = ops.remove(0);
                m.push(ContKind::ArgDone {
                    proc,
                    collected: Vec::new(),
                    pending: ops,
                    env: env.clone(),
                });
                Ok(State::Eval(first, env.clone()))
            }
        }
        ContKind::ArgDone {
            proc,
            collected,
            pending,
            env,
        } => {
            let mut collected = collected.clone();
            collected.push(v);
            if pending.is_empty() {
                Ok(State::Apply(proc.clone(), collected))
            } else {
                let mut pend = pending.clone();
                let next = pend.remove(0);
                m.push(ContKind::ArgDone {
                    proc: proc.clone(),
                    collected,
                    pending: pend,
                    env: env.clone(),
                });
                Ok(State::Eval(next, env.clone()))
            }
        }
        ContKind::And { rest, env } => {
            if !v.is_truthy() {
                Ok(State::Return(Value::Bool(false)))
            } else if rest.len() == 1 {
                Ok(State::Eval(rest[0].clone(), env.clone()))
            } else {
                let mut r = rest.clone();
                let next = r.remove(0);
                m.push(ContKind::And {
                    rest: r,
                    env: env.clone(),
                });
                Ok(State::Eval(next, env.clone()))
            }
        }
        ContKind::Or { rest, env } => {
            if v.is_truthy() {
                Ok(State::Return(v))
            } else if rest.len() == 1 {
                Ok(State::Eval(rest[0].clone(), env.clone()))
            } else {
                let mut r = rest.clone();
                let next = r.remove(0);
                m.push(ContKind::Or {
                    rest: r,
                    env: env.clone(),
                });
                Ok(State::Eval(next, env.clone()))
            }
        }
        ContKind::BodyInit {
            temps,
            pending,
            locs,
            body,
            env,
        } => {
            let mut temps = temps.clone();
            temps.push(v);
            if pending.is_empty() {
                for (loc, val) in locs.iter().zip(temps) {
                    *loc.borrow_mut() = val;
                }
                Ok(seq(m, body.clone(), env.clone()))
            } else {
                let mut pend = pending.clone();
                let next = pend.remove(0);
                m.push(ContKind::BodyInit {
                    temps,
                    pending: pend,
                    locs: locs.clone(),
                    body: body.clone(),
                    env: env.clone(),
                });
                Ok(State::Eval(next, env.clone()))
            }
        }
        ContKind::DynWindBefore {
            before,
            thunk,
            after,
        } => {
            let wind = Rc::new(DynWind {
                before: before.clone(),
                after: after.clone(),
            });
            m.winds = wind_push(wind.clone(), &m.winds);
            m.push(ContKind::DynWindBody { wind });
            Ok(State::Apply(thunk.clone(), vec![]))
        }
        ContKind::DynWindBody { wind } => {
            // leave the dynamic extent
            if let Some(n) = &m.winds {
                m.winds = n.parent.clone();
            }
            m.push(ContKind::DynWindAfter { value: v });
            Ok(apply_hook(&wind.after))
        }
        ContKind::DynWindAfter { value } => Ok(State::Return(value.clone())),
        ContKind::WindSteps {
            steps,
            value,
            target,
        } => {
            if steps.is_empty() {
                m.cont = target.cont.clone();
                m.winds = target.winds.clone();
                Ok(State::Return(value.clone()))
            } else {
                let mut steps = steps.clone();
                let (hook, w) = steps.remove(0);
                m.winds = w;
                m.push(ContKind::WindSteps {
                    steps,
                    value: value.clone(),
                    target: target.clone(),
                });
                Ok(apply_hook(&hook))
            }
        }
        ContKind::Force { promise } => {
            // R5RS make-promise 参考实现语义：proc 的返回值就是缓存值
            // （哪怕它本身是另一个 promise——不做链式塌缩），且外层 promise
            // 一定在这里被标记 forced，保证"一个 promise 只求值一次"。
            let mut pb = promise.borrow_mut();
            pb.forcing = pb.forcing.saturating_sub(1);
            pb.forced = true;
            pb.value = v.clone();
            Ok(State::Return(v))
        }
        ContKind::Map {
            f,
            lists,
            collected,
        } => {
            let mut collected = collected.clone();
            collected.push(v);
            advance_map(m, f, lists, collected, true)
        }
        ContKind::ForEach { f, lists } => advance_map(m, f, lists, vec![], false),
        ContKind::CallWithValues { consumer } => match &v {
            Value::Values(xs) => Ok(State::Apply(consumer.clone(), xs.as_ref().clone())),
            _ => Ok(State::Apply(consumer.clone(), vec![v])),
        },
        ContKind::Load { rest, env } => {
            if rest.is_empty() {
                Ok(State::Return(v))
            } else {
                let mut r = rest.clone();
                let next = r.remove(0);
                m.push(ContKind::Load {
                    rest: r,
                    env: env.clone(),
                });
                Ok(State::Eval(next, env.clone()))
            }
        }
        ContKind::ClosePortAfter { port } => {
            port.close();
            Ok(State::Return(v))
        }
        ContKind::GetOutputString { port } => {
            let s = port.get_output_string()?;
            Ok(State::Return(Value::Str(Rc::new(RefCell::new(s)))))
        }
    }
}

fn advance_map(
    m: &mut Machine,
    f: &Value,
    lists: &[Value],
    collected: Vec<Value>,
    is_map: bool,
) -> Result<State, String> {
    let mut new_lists = Vec::with_capacity(lists.len());
    let mut cars = Vec::with_capacity(lists.len());
    for l in lists {
        match l {
            Value::Nil => {
                return Ok(State::Return(if is_map {
                    list_from_vec(collected)
                } else {
                    Value::Unspecified
                }))
            }
            Value::Pair(p) => {
                let (a, d) = {
                    let b = p.borrow();
                    (b.0.clone(), b.1.clone())
                };
                cars.push(a);
                new_lists.push(d);
            }
            _ => return Err("map/for-each: improper list".into()),
        }
    }
    if is_map {
        m.push(ContKind::Map {
            f: f.clone(),
            lists: new_lists,
            collected,
        });
    } else {
        m.push(ContKind::ForEach {
            f: f.clone(),
            lists: new_lists,
        });
    }
    Ok(State::Apply(f.clone(), cars))
}

// ---------------------------------------------------------------------------
// Trace support（扩展功能，见 docs/extensions.md）

/// 过程的稳定标识。闭包用创建时分配的 id（见 Closure::id）；内建过程用
/// 名字的 'static 字符串指针（稳定）。两个子空间分开，互不冲突。
#[derive(PartialEq, Eq, Hash)]
enum TraceKey {
    Closure(usize),
    Prim(usize),
}

thread_local! {
    /// 被跟踪的过程：key 为过程的稳定标识，value 为展示名。
    static TRACED: RefCell<HashMap<TraceKey, String>> = RefCell::new(HashMap::new());
}

fn trace_key(proc: &Value) -> Option<TraceKey> {
    match proc {
        Value::Closure(c) => Some(TraceKey::Closure(c.id)),
        Value::Primitive(name) => Some(TraceKey::Prim(name.as_ptr() as usize)),
        _ => None,
    }
}

/// 注册跟踪一个过程（闭包或内建过程）。
pub fn trace_add(proc: &Value, label: String) -> Result<(), String> {
    match trace_key(proc) {
        Some(k) => {
            TRACED.with(|t| t.borrow_mut().insert(k, label));
            Ok(())
        }
        None => Err(format!("trace: not a procedure: {}", write_to_string(proc))),
    }
}

/// 取消对一个过程的跟踪；返回是否之前在跟踪。
pub fn trace_remove(proc: &Value) -> bool {
    match trace_key(proc) {
        Some(k) => TRACED.with(|t| t.borrow_mut().remove(&k).is_some()),
        None => false,
    }
}

pub fn trace_clear() {
    TRACED.with(|t| t.borrow_mut().clear());
}

fn trace_label_of(proc: &Value) -> Option<String> {
    let k = trace_key(proc)?;
    TRACED.with(|t| t.borrow().get(&k).cloned())
}

/// 当前续延栈上的 TraceReturn 帧数 = 被跟踪调用的嵌套深度。
fn trace_depth(cont: &Cont) -> usize {
    let mut n = 0;
    let mut cur = cont;
    while let Some(f) = cur {
        if matches!(f.kind, ContKind::TraceReturn { .. }) {
            n += 1;
        }
        cur = &f.parent;
    }
    n
}

// ---------------------------------------------------------------------------
// Procedure application

pub fn apply(m: &mut Machine, proc: Value, args: Vec<Value>) -> Result<State, String> {
    if let Some(label) = trace_label_of(&proc) {
        let depth = trace_depth(&m.cont);
        let mut line = format!("{}({}", "| ".repeat(depth), label);
        for a in &args {
            line.push(' ');
            line.push_str(&write_to_string(a));
        }
        line.push(')');
        println!("{}", line);
        m.push(ContKind::TraceReturn { depth });
    }
    match &proc {
        Value::Primitive(name) => crate::builtins::dispatch(m, name, args),
        Value::Closure(c) => {
            let c = c.clone();
            if args.len() < c.fixed.len() || (c.rest.is_none() && args.len() > c.fixed.len()) {
                return Err(format!(
                    "wrong number of arguments (got {}, expected {}{})",
                    args.len(),
                    c.fixed.len(),
                    if c.rest.is_some() { "+" } else { "" }
                ));
            }
            let env2 = Env::new(Some(c.env.clone()));
            for (i, name) in c.fixed.iter().enumerate() {
                env2.define_loc(*name, args[i].clone());
            }
            if let Some(r) = c.rest {
                env2.define_loc(r, list_from_vec(args[c.fixed.len()..].to_vec()));
            }
            let prep = prepare_body(&c.body, &env2)?;
            Ok(kick_body(m, prep, env2))
        }
        Value::Continuation(k) => {
            // R5RS 6.4: an escape procedure accepts the same number of
            // arguments as the continuation it passes them to; multiple
            // arguments are delivered as multiple values (this is how the
            // report's own `values` definition works).
            let v = if args.len() == 1 {
                args.into_iter().next().unwrap()
            } else {
                Value::Values(Rc::new(args))
            };
            // 恢复续延 = 整根续延栈指针换掉；但换之前要先把 dynamic-wind
            // 的"离开/进入"序列按顺序跑完：afters 由内到外、befores 由外到
            // 内。每个 thunk 求值时动态环境要处在"已离开/未进入"的中间
            // 状态，所以 steps 里为每个 thunk 记下当时的 wind 链。
            let (afters, befores) = wind_diff(&m.winds, &k.winds);
            let mut steps: Vec<(WindHook, WindList)> = Vec::new();
            let mut w = m.winds.clone();
            for dw in &afters {
                w = wind_parent(&w);
                steps.push((dw.after.clone(), w.clone()));
            }
            for dw in &befores {
                steps.push((dw.before.clone(), w.clone()));
                w = wind_push(dw.clone(), &w);
            }
            if steps.is_empty() {
                m.cont = k.cont.clone();
                m.winds = k.winds.clone();
                Ok(State::Return(v))
            } else {
                let first = steps.remove(0);
                m.push(ContKind::WindSteps {
                    steps,
                    value: v,
                    target: k.clone(),
                });
                m.winds = first.1;
                Ok(apply_hook(&first.0))
            }
        }
        _ => Err(format!("not a procedure: {}", write_to_string(&proc))),
    }
}

fn wind_parent(w: &WindList) -> WindList {
    match w {
        Some(n) => n.parent.clone(),
        None => None,
    }
}

// ---------------------------------------------------------------------------
// Internal defines / body processing

pub struct BodyPrep {
    pub defines: Vec<(Sym, Value)>,
    pub rest: Vec<Value>,
}

/// Scan the front of a body: splice `begin`s, expand macro uses, process
/// `define-syntax`, collect internal `define`s.
pub fn prepare_body(forms: &[Value], env: &Rc<Env>) -> Result<BodyPrep, String> {
    let mut forms: Vec<Value> = forms.to_vec();
    let mut defines: Vec<(Sym, Value)> = Vec::new();
    let mut i = 0;
    while i < forms.len() {
        let f = forms[i].clone();
        let mut action_taken = false;
        if let Value::Pair(p) = &f {
            let head = p.borrow().0.clone();
            if let Value::Symbol(s) = head {
                match resolve(env, s) {
                    Meaning::Macro(t) => {
                        forms[i] = syntax_rules::expand(&t, &f, env)?;
                        action_taken = true;
                    }
                    Meaning::Keyword(k) => {
                        let name = sym_str(k);
                        let cdr = p.borrow().1.clone();
                        match name.as_str() {
                            "begin" => {
                                let (items, rest) = list_to_vec(&cdr)
                                    .ok_or_else(|| "begin: circular".to_string())?;
                                if !rest.is_nil() {
                                    return Err("begin: malformed".into());
                                }
                                forms.splice(i..=i, items);
                                action_taken = true;
                            }
                            "define" => {
                                let (items, _) = list_to_vec(&cdr)
                                    .ok_or_else(|| "define: circular".to_string())?;
                                let (n, rhs) = parse_define(&items)?;
                                defines.push((n, rhs));
                                i += 1;
                                action_taken = true;
                            }
                            "define-syntax" => {
                                let (items, _) = list_to_vec(&cdr)
                                    .ok_or_else(|| "define-syntax: circular".to_string())?;
                                do_define_syntax(&items, env)?;
                                forms.remove(i);
                                action_taken = true;
                            }
                            "let-syntax" | "letrec-syntax" => {
                                let (items, _) = list_to_vec(&cdr)
                                    .ok_or_else(|| "let-syntax: circular".to_string())?;
                                let empty = match items.first() {
                                    Some(b) => matches!(proper_list(b), Some(v) if v.is_empty()),
                                    None => false,
                                };
                                if empty {
                                    // no new scope: splice the body
                                    forms.splice(i..=i, items[1..].iter().cloned());
                                    action_taken = true;
                                }
                            }
                            _ => {}
                        }
                    }
                    _ => {}
                }
            }
        }
        if !action_taken {
            break;
        }
    }
    forms.drain(..i.min(forms.len()));
    Ok(BodyPrep {
        defines,
        rest: forms,
    })
}

pub fn kick_body(m: &mut Machine, prep: BodyPrep, env: Rc<Env>) -> State {
    if prep.defines.is_empty() {
        return seq(m, prep.rest, env);
    }
    // 内部 define 采用 letrec 语义：先把所有名字绑到未指定的 location，
    // 再依次求值各右值，最后统一写回（BodyInit 的 batch 赋值）。统一写回
    // 与 desugar_letrec 的 temps+assign 同理——pitfall 1.1/1.2 要求重入
    // 初始化表达式的续延时能看到后写的值。
    let e2 = Env::new(Some(env));
    let mut locs = Vec::new();
    for (name, _) in &prep.defines {
        locs.push(e2.define_loc(*name, Value::Unspecified));
    }
    let mut rhss: Vec<Value> = prep.defines.into_iter().map(|(_, r)| r).collect();
    let first = rhss.remove(0);
    m.push(ContKind::BodyInit {
        temps: Vec::new(),
        pending: rhss,
        locs: Rc::new(locs),
        body: prep.rest,
        env: e2.clone(),
    });
    State::Eval(first, e2)
}

/// Parse the cdr of a `define` form into (name, value-expression),
/// handling the procedure-definition sugar (including currying).
pub fn parse_define(items: &[Value]) -> Result<(Sym, Value), String> {
    if items.is_empty() {
        return Err("define: malformed".into());
    }
    match &items[0] {
        Value::Symbol(s) => {
            let expr = if items.len() >= 2 {
                items[1].clone()
            } else {
                Value::Unspecified
            };
            Ok((*s, expr))
        }
        Value::Pair(_) => {
            // (define (f . args) body...) with possible currying
            let mut body: Vec<Value> = items[1..].to_vec();
            let mut target = items[0].clone();
            loop {
                match target {
                    Value::Pair(p) => {
                        let (name, largs) = {
                            let b = p.borrow();
                            (b.0.clone(), b.1.clone())
                        };
                        let mut lam = vec![Value::sym("lambda"), largs];
                        lam.extend(body);
                        body = vec![list_from_vec(lam)];
                        target = name;
                    }
                    Value::Symbol(s) => return Ok((s, body.into_iter().next().unwrap())),
                    _ => return Err("define: malformed".into()),
                }
            }
        }
        _ => Err("define: malformed".into()),
    }
}

fn do_define_syntax(items: &[Value], env: &Rc<Env>) -> Result<(), String> {
    if items.len() != 2 {
        return Err("define-syntax: malformed".into());
    }
    let name = match &items[0] {
        Value::Symbol(s) => *s,
        _ => return Err("define-syntax: name must be identifier".into()),
    };
    let t = syntax_rules::parse_transformer(&items[1], env)?;
    env.define_macro(name, t);
    Ok(())
}

// ---------------------------------------------------------------------------
// Special forms

fn special(m: &mut Machine, kw: Sym, expr: &Value, env: Rc<Env>) -> Result<State, String> {
    let cdr = match expr {
        Value::Pair(p) => p.borrow().1.clone(),
        _ => unreachable!(),
    };
    let (args, rest) = list_to_vec(&cdr).ok_or_else(|| "circular form".to_string())?;
    if !rest.is_nil() {
        return Err(format!("malformed special form: {}", write_to_string(expr)));
    }
    match sym_str(kw).as_str() {
        "quote" => {
            if args.len() != 1 {
                return Err("quote: needs one argument".into());
            }
            Ok(State::Return(args[0].clone()))
        }
        "lambda" => {
            if args.is_empty() {
                return Err("lambda: malformed".into());
            }
            let (fixed, rest) = parse_params(&args[0])?;
            Ok(State::Return(Value::Closure(Rc::new(
                crate::value::Closure::new(fixed, rest, Rc::new(args[1..].to_vec()), env),
            ))))
        }
        "if" => {
            if args.len() < 2 || args.len() > 3 {
                return Err("if: needs 2 or 3 arguments".into());
            }
            m.push(ContKind::If {
                conseq: args[1].clone(),
                alt: args.get(2).cloned(),
                env: env.clone(),
            });
            Ok(State::Eval(args[0].clone(), env))
        }
        "define" => {
            let (name, rhs) = parse_define(&args)?;
            m.push(ContKind::Define {
                name,
                env: env.clone(),
            });
            Ok(State::Eval(rhs, env))
        }
        "set!" => {
            if args.len() != 2 {
                return Err("set!: malformed".into());
            }
            let name = match &args[0] {
                Value::Symbol(s) => *s,
                _ => return Err("set!: not an identifier".into()),
            };
            m.push(ContKind::Set {
                name,
                env: env.clone(),
            });
            Ok(State::Eval(args[1].clone(), env))
        }
        "begin" => Ok(seq(m, args, env)),
        "and" => {
            if args.is_empty() {
                return Ok(State::Return(Value::Bool(true)));
            }
            let mut a = args;
            let first = a.remove(0);
            if a.is_empty() {
                Ok(State::Eval(first, env))
            } else {
                m.push(ContKind::And {
                    rest: a,
                    env: env.clone(),
                });
                Ok(State::Eval(first, env))
            }
        }
        "or" => {
            if args.is_empty() {
                return Ok(State::Return(Value::Bool(false)));
            }
            let mut a = args;
            let first = a.remove(0);
            if a.is_empty() {
                Ok(State::Eval(first, env))
            } else {
                m.push(ContKind::Or {
                    rest: a,
                    env: env.clone(),
                });
                Ok(State::Eval(first, env))
            }
        }
        "cond" => {
            let d = desugar_cond(&args, &env)?;
            Ok(State::Eval(d, env))
        }
        "case" => {
            if args.is_empty() {
                return Err("case: malformed".into());
            }
            let d = desugar_case(&args[0], &args[1..], &env)?;
            Ok(State::Eval(d, env))
        }
        "let" => {
            let d = desugar_let(&args)?;
            Ok(State::Eval(d, env))
        }
        "let*" => {
            let d = desugar_letstar(&args)?;
            Ok(State::Eval(d, env))
        }
        "letrec" => {
            let d = desugar_letrec(&args)?;
            Ok(State::Eval(d, env))
        }
        "do" => {
            let d = desugar_do(&args)?;
            Ok(State::Eval(d, env))
        }
        "delay" => {
            if args.len() != 1 {
                return Err("delay: malformed".into());
            }
            Ok(State::Return(Value::Promise(Rc::new(RefCell::new(
                Promise {
                    forced: false,
                    forcing: 0,
                    value: Value::Unspecified,
                    expr: args[0].clone(),
                    env,
                },
            )))))
        }
        "quasiquote" => {
            if args.len() != 1 {
                return Err("quasiquote: malformed".into());
            }
            let v = quasiquote(&args[0], 1, &env)?;
            Ok(State::Return(v))
        }
        "define-syntax" => {
            do_define_syntax(&args, &env)?;
            Ok(State::Return(Value::Unspecified))
        }
        "let-syntax" => let_syntax(m, &args, env, false),
        "letrec-syntax" => let_syntax(m, &args, env, true),
        other => Err(format!("unknown special form: {}", other)),
    }
}

fn parse_params(v: &Value) -> Result<(Vec<Sym>, Option<Sym>), String> {
    let (items, rest) = list_to_vec(v).ok_or_else(|| "lambda: circular params".to_string())?;
    let mut fixed = Vec::new();
    for it in items {
        match it {
            Value::Symbol(s) => fixed.push(s),
            _ => return Err("lambda: parameter must be identifier".into()),
        }
    }
    let rest = match rest {
        Value::Nil => None,
        Value::Symbol(s) => Some(s),
        _ => return Err("lambda: bad rest parameter".into()),
    };
    Ok((fixed, rest))
}

fn let_syntax(
    m: &mut Machine,
    args: &[Value],
    env: Rc<Env>,
    is_rec: bool,
) -> Result<State, String> {
    if args.is_empty() {
        return Err("let-syntax: malformed".into());
    }
    let bindings = proper_list(&args[0]).ok_or_else(|| "let-syntax: bad bindings".to_string())?;
    if bindings.is_empty() {
        // empty bindings: no new scope
        let prep = prepare_body(&args[1..], &env)?;
        return Ok(kick_body(m, prep, env));
    }
    let e2 = Env::new(Some(env.clone()));
    for b in bindings {
        let pair = proper_list(&b).ok_or_else(|| "let-syntax: bad binding".to_string())?;
        if pair.len() != 2 {
            return Err("let-syntax: bad binding".into());
        }
        let name = match &pair[0] {
            Value::Symbol(s) => *s,
            _ => return Err("let-syntax: name must be identifier".into()),
        };
        let def_env = if is_rec { e2.clone() } else { env.clone() };
        let t = syntax_rules::parse_transformer(&pair[1], &def_env)?;
        e2.define_macro(name, t);
    }
    let prep = prepare_body(&args[1..], &e2)?;
    Ok(kick_body(m, prep, e2))
}

// ---------------------------------------------------------------------------
// Derived form desugaring

fn vl(items: Vec<Value>) -> Value {
    list_from_vec(items)
}

fn vs(s: &str) -> Value {
    Value::Symbol(intern(s))
}

fn parse_bindings(v: &Value) -> Result<(Vec<Sym>, Vec<Value>), String> {
    const WANT: &str = "expected (name value)";
    let bs = proper_list(v)
        .ok_or_else(|| format!("bad binding list: {} ({})", write_to_string(v), WANT))?;
    let mut vars = Vec::new();
    let mut inits = Vec::new();
    for b in bs {
        let pair = proper_list(&b)
            .ok_or_else(|| format!("bad binding: {} ({})", write_to_string(&b), WANT))?;
        if pair.len() != 2 {
            return Err(format!("bad binding: {} ({})", write_to_string(&b), WANT));
        }
        match &pair[0] {
            Value::Symbol(s) => vars.push(*s),
            _ => {
                return Err(format!(
                    "binding name must be identifier: {}",
                    write_to_string(&pair[0])
                ))
            }
        }
        inits.push(pair[1].clone());
    }
    Ok((vars, inits))
}

fn make_lambda(params: Vec<Value>, body: Vec<Value>) -> Value {
    let mut l = vec![vs("lambda"), list_from_vec(params)];
    l.extend(body);
    list_from_vec(l)
}

fn desugar_let(args: &[Value]) -> Result<Value, String> {
    if args.is_empty() {
        return Err("let: malformed".into());
    }
    if let Value::Symbol(name) = &args[0] {
        // named let
        if args.len() < 2 {
            return Err("named let: malformed".into());
        }
        let (vars, inits) = parse_bindings(&args[1])?;
        let body = args[2..].to_vec();
        let lam = make_lambda(vars.iter().map(|s| Value::Symbol(*s)).collect(), body);
        // ((letrec ((name lam)) name) inits...) -- inits are evaluated
        // outside the scope of the loop binding.
        let binding = vl(vec![vl(vec![Value::Symbol(*name), lam])]);
        let letrec = vl(vec![vs("letrec"), binding, Value::Symbol(*name)]);
        let mut app = vec![letrec];
        app.extend(inits);
        Ok(list_from_vec(app))
    } else {
        let (vars, inits) = parse_bindings(&args[0])?;
        let body = args[1..].to_vec();
        let lam = make_lambda(vars.iter().map(|s| Value::Symbol(*s)).collect(), body);
        let mut app = vec![lam];
        app.extend(inits);
        Ok(list_from_vec(app))
    }
}

fn desugar_letstar(args: &[Value]) -> Result<Value, String> {
    if args.is_empty() {
        return Err("let*: malformed".into());
    }
    let bs = proper_list(&args[0]).ok_or_else(|| "let*: bad bindings".to_string())?;
    let body = args[1..].to_vec();
    if bs.is_empty() {
        let lam = make_lambda(vec![], body);
        return Ok(vl(vec![lam]));
    }
    let first = bs[0].clone();
    let rest = &args[0];
    let rest_bindings = match list_to_vec(rest) {
        Some((mut items, _)) => {
            items.remove(0);
            list_from_vec(items)
        }
        None => return Err("let*: bad bindings".into()),
    };
    let inner = vl(vec![vs("let*"), rest_bindings]).append_body(body);
    Ok(vl(vec![vs("let"), vl(vec![first]), inner]))
}

trait AppendBody {
    fn append_body(self, body: Vec<Value>) -> Value;
}
impl AppendBody for Value {
    fn append_body(self, body: Vec<Value>) -> Value {
        let (mut items, rest) = list_to_vec(&self).unwrap();
        debug_assert!(rest.is_nil());
        items.extend(body);
        list_from_vec(items)
    }
}

fn desugar_letrec(args: &[Value]) -> Result<Value, String> {
    if args.is_empty() {
        return Err("letrec: malformed".into());
    }
    let (vars, inits) = parse_bindings(&args[0])?;
    let body = args[1..].to_vec();
    // ((lambda (vars...) ((lambda (temps...) (set! v t)... body...) inits...)) <unspec>...)
    let temps: Vec<Sym> = vars.iter().map(|_| gensym("t")).collect();
    let mut inner_body: Vec<Value> = Vec::new();
    for (v, t) in vars.iter().zip(temps.iter()) {
        inner_body.push(vl(vec![vs("set!"), Value::Symbol(*v), Value::Symbol(*t)]));
    }
    inner_body.extend(body);
    let inner_lam = make_lambda(
        temps.iter().map(|s| Value::Symbol(*s)).collect(),
        inner_body,
    );
    let mut inner_app = vec![inner_lam];
    inner_app.extend(inits);
    let outer_lam = make_lambda(
        vars.iter().map(|s| Value::Symbol(*s)).collect(),
        vec![list_from_vec(inner_app)],
    );
    let mut outer_app = vec![outer_lam];
    for _ in &vars {
        outer_app.push(Value::Unspecified);
    }
    Ok(list_from_vec(outer_app))
}

fn desugar_do(args: &[Value]) -> Result<Value, String> {
    if args.len() < 2 {
        return Err("do: malformed".into());
    }
    let bs = proper_list(&args[0]).ok_or_else(|| "do: bad bindings".to_string())?;
    let mut vars = Vec::new();
    let mut inits = Vec::new();
    let mut steps = Vec::new();
    for b in bs {
        let parts = proper_list(&b).ok_or_else(|| "do: bad binding".to_string())?;
        if parts.len() < 2 || parts.len() > 3 {
            return Err("do: bad binding".into());
        }
        match &parts[0] {
            Value::Symbol(s) => vars.push(*s),
            _ => return Err("do: bad binding".into()),
        }
        inits.push(parts[1].clone());
        steps.push(if parts.len() == 3 {
            parts[2].clone()
        } else {
            parts[0].clone()
        });
    }
    let test_clause = proper_list(&args[1]).ok_or_else(|| "do: bad test clause".to_string())?;
    if test_clause.is_empty() {
        return Err("do: bad test clause".into());
    }
    let test = test_clause[0].clone();
    let result_exprs = test_clause[1..].to_vec();
    let commands = args[2..].to_vec();
    let loop_sym = gensym("loop");
    let mut call = vec![Value::Symbol(loop_sym)];
    call.extend(steps);
    let mut else_body = commands;
    else_body.push(list_from_vec(call));
    let if_form = vl(vec![
        vs("if"),
        test,
        vl(vec![vs("begin")]).append_body(result_exprs),
        vl(vec![vs("begin")]).append_body(else_body),
    ]);
    let lam = make_lambda(
        vars.iter().map(|s| Value::Symbol(*s)).collect(),
        vec![if_form],
    );
    let binding = vl(vec![vl(vec![Value::Symbol(loop_sym), lam])]);
    let mut top_call = vec![Value::Symbol(loop_sym)];
    top_call.extend(inits);
    Ok(vl(vec![vs("letrec"), binding, list_from_vec(top_call)]))
}

fn desugar_cond(clauses: &[Value], env: &Rc<Env>) -> Result<Value, String> {
    if clauses.is_empty() {
        return Ok(Value::Unspecified);
    }
    let clause = proper_list(&clauses[0]).ok_or_else(|| "cond: bad clause".to_string())?;
    if clause.is_empty() {
        return Err("cond: empty clause".into());
    }
    let rest = desugar_cond(&clauses[1..], env)?;
    let first = &clause[0];
    // else clause
    if let Value::Symbol(s) = first {
        if aux_name(env, *s)?.as_deref() == Some("else") {
            if clauses.len() != 1 {
                return Err("cond: else must be last".into());
            }
            if clause.len() == 1 {
                return Ok(Value::Bool(true));
            }
            return Ok(vl(vec![vs("begin")]).append_body(clause[1..].to_vec()));
        }
    }
    if clause.len() == 1 {
        let g = gensym("t");
        return Ok(vl(vec![
            vs("let"),
            vl(vec![vl(vec![Value::Symbol(g), first.clone()])]),
            vl(vec![vs("if"), Value::Symbol(g), Value::Symbol(g), rest]),
        ]));
    }
    if let Value::Symbol(s) = &clause[1] {
        if aux_name(env, *s)?.as_deref() == Some("=>") {
            if clause.len() != 3 {
                return Err("cond: bad => clause".into());
            }
            let g = gensym("t");
            return Ok(vl(vec![
                vs("let"),
                vl(vec![vl(vec![Value::Symbol(g), first.clone()])]),
                vl(vec![
                    vs("if"),
                    Value::Symbol(g),
                    vl(vec![clause[2].clone(), Value::Symbol(g)]),
                    rest,
                ]),
            ]));
        }
    }
    Ok(vl(vec![
        vs("if"),
        first.clone(),
        vl(vec![vs("begin")]).append_body(clause[1..].to_vec()),
        rest,
    ]))
}

fn desugar_case(key: &Value, clauses: &[Value], env: &Rc<Env>) -> Result<Value, String> {
    let g = gensym("key");
    let inner = case_chain(&Value::Symbol(g), clauses, env)?;
    Ok(vl(vec![
        vs("let"),
        vl(vec![vl(vec![Value::Symbol(g), key.clone()])]),
        inner,
    ]))
}

fn case_chain(k: &Value, clauses: &[Value], env: &Rc<Env>) -> Result<Value, String> {
    if clauses.is_empty() {
        return Ok(Value::Unspecified);
    }
    let clause = proper_list(&clauses[0]).ok_or_else(|| "case: bad clause".to_string())?;
    if clause.is_empty() {
        return Err("case: empty clause".into());
    }
    let rest = case_chain(k, &clauses[1..], env)?;
    // else clause
    if let Value::Symbol(s) = &clause[0] {
        if aux_name(env, *s)?.as_deref() == Some("else") {
            if clauses.len() != 1 {
                return Err("case: else must be last".into());
            }
            if clause.len() >= 2 {
                if let Value::Symbol(a) = &clause[1] {
                    if aux_name(env, *a)?.as_deref() == Some("=>") && clause.len() == 3 {
                        return Ok(vl(vec![clause[2].clone(), k.clone()]));
                    }
                }
            }
            return Ok(vl(vec![vs("begin")]).append_body(clause[1..].to_vec()));
        }
    }
    let datums = clause[0].clone();
    // => variant
    if clause.len() == 3 {
        if let Value::Symbol(a) = &clause[1] {
            if aux_name(env, *a)?.as_deref() == Some("=>") {
                return Ok(vl(vec![
                    vs("if"),
                    vl(vec![vs("memv"), k.clone(), vl(vec![vs("quote"), datums])]),
                    vl(vec![clause[2].clone(), k.clone()]),
                    rest,
                ]));
            }
        }
    }
    Ok(vl(vec![
        vs("if"),
        vl(vec![vs("memv"), k.clone(), vl(vec![vs("quote"), datums])]),
        vl(vec![vs("begin")]).append_body(clause[1..].to_vec()),
        rest,
    ]))
}

// ---------------------------------------------------------------------------
// Quasiquote

fn quasiquote(t: &Value, depth: usize, env: &Rc<Env>) -> Result<Value, String> {
    match t {
        Value::Pair(p) => {
            let (a, d) = {
                let b = p.borrow();
                (b.0.clone(), b.1.clone())
            };
            if let Value::Symbol(s) = &a {
                if aux_name(env, *s)?.as_deref() == Some("unquote") {
                    let (xs, tail) =
                        list_to_vec(&d).ok_or_else(|| "unquote: circular".to_string())?;
                    if xs.len() == 1 && tail.is_nil() {
                        if depth == 1 {
                            return run(State::Eval(xs[0].clone(), env.clone()));
                        }
                        return Ok(vl(vec![vs("unquote"), quasiquote(&xs[0], depth - 1, env)?]));
                    }
                    return Err("unquote: malformed".into());
                }
                if aux_name(env, *s)?.as_deref() == Some("quasiquote") {
                    let (xs, tail) =
                        list_to_vec(&d).ok_or_else(|| "quasiquote: circular".to_string())?;
                    if xs.len() == 1 && tail.is_nil() {
                        return Ok(vl(vec![
                            vs("quasiquote"),
                            quasiquote(&xs[0], depth + 1, env)?,
                        ]));
                    }
                    return Err("quasiquote: malformed".into());
                }
            }
            if let Value::Pair(ap) = &a {
                let (aa, ad) = {
                    let b = ap.borrow();
                    (b.0.clone(), b.1.clone())
                };
                if let Value::Symbol(s) = &aa {
                    if aux_name(env, *s)?.as_deref() == Some("unquote-splicing") {
                        let (xs, tail) = list_to_vec(&ad)
                            .ok_or_else(|| "unquote-splicing: circular".to_string())?;
                        if xs.len() == 1 && tail.is_nil() {
                            if depth == 1 {
                                let spliced = run(State::Eval(xs[0].clone(), env.clone()))?;
                                let rest = quasiquote(&d, depth, env)?;
                                return qq_append(&spliced, rest);
                            }
                            let rebuilt = vl(vec![
                                vs("unquote-splicing"),
                                quasiquote(&xs[0], depth - 1, env)?,
                            ]);
                            return Ok(cons(rebuilt, quasiquote(&d, depth, env)?));
                        }
                        return Err("unquote-splicing: malformed".into());
                    }
                }
            }
            Ok(cons(
                quasiquote(&a, depth, env)?,
                quasiquote(&d, depth, env)?,
            ))
        }
        Value::Vector(items) => {
            let as_list = list_from_vec(items.borrow().clone());
            let q = quasiquote(&as_list, depth, env)?;
            let v = proper_list(&q).ok_or_else(|| "bad vector quasiquote".to_string())?;
            Ok(Value::Vector(Rc::new(RefCell::new(v))))
        }
        _ => Ok(t.clone()),
    }
}

fn qq_append(spliced: &Value, rest: Value) -> Result<Value, String> {
    if rest.is_nil() {
        return Ok(spliced.clone());
    }
    if spliced.is_nil() {
        return Ok(rest);
    }
    match list_to_vec(spliced) {
        Some((items, tail)) => {
            if !tail.is_nil() {
                return Err("unquote-splicing: not a list".into());
            }
            let mut out = rest;
            for x in items.into_iter().rev() {
                out = cons(x, out);
            }
            Ok(out)
        }
        None => Err("unquote-splicing: circular list".into()),
    }
}

// ---------------------------------------------------------------------------
// Top-level driver

/// Evaluate a sequence of top-level forms, splicing top-level `begin`s and
/// expanding top-level macro uses.
pub fn eval_program(forms: Vec<Value>, env: &Rc<Env>) -> Result<Value, String> {
    let mut queue: VecDeque<Value> = forms.into();
    let mut last = Value::Unspecified;
    'top: while let Some(mut f) = queue.pop_front() {
        loop {
            let mut done = true;
            if let Value::Pair(p) = &f {
                let head = p.borrow().0.clone();
                if let Value::Symbol(s) = head {
                    match resolve(env, s) {
                        Meaning::Macro(t) => {
                            f = syntax_rules::expand(&t, &f, env)?;
                            done = false;
                        }
                        Meaning::Keyword(k) if sym_str(k) == "begin" => {
                            let cdr = p.borrow().1.clone();
                            let (items, rest) =
                                list_to_vec(&cdr).ok_or_else(|| "begin: circular".to_string())?;
                            if !rest.is_nil() {
                                return Err("begin: malformed".into());
                            }
                            for x in items.into_iter().rev() {
                                queue.push_front(x);
                            }
                            continue 'top;
                        }
                        _ => {}
                    }
                }
            }
            if done {
                break;
            }
        }
        last = run(State::Eval(f, env.clone()))?;
    }
    Ok(last)
}

pub fn eval_str(s: &str, env: &Rc<Env>) -> Result<Value, String> {
    let forms = reader::read_all_strict(s).map_err(|e| match e {
        reader::ReadError::Eof => "unexpected end of input".to_string(),
        reader::ReadError::Msg(m) => m,
    })?;
    eval_program(forms, env)
}

/// Start a map/for-each: evaluate (f car...) with a frame collecting results.
pub fn kick_map(
    m: &mut Machine,
    f: Value,
    lists: Vec<Value>,
    is_map: bool,
) -> Result<State, String> {
    let mut new_lists = Vec::with_capacity(lists.len());
    let mut cars = Vec::with_capacity(lists.len());
    for l in &lists {
        match l {
            Value::Nil => {
                return Ok(State::Return(if is_map {
                    Value::Nil
                } else {
                    Value::Unspecified
                }))
            }
            Value::Pair(p) => {
                let (a, d) = {
                    let b = p.borrow();
                    (b.0.clone(), b.1.clone())
                };
                cars.push(a);
                new_lists.push(d);
            }
            _ => return Err("map/for-each: not a list".into()),
        }
    }
    if is_map {
        m.push(ContKind::Map {
            f: f.clone(),
            lists: new_lists,
            collected: Vec::new(),
        });
    } else {
        m.push(ContKind::ForEach {
            f: f.clone(),
            lists: new_lists,
        });
    }
    Ok(State::Apply(f, cars))
}

/// Construct a Pair value (used by builtins).
pub fn make_pair(a: Value, d: Value) -> Value {
    Value::Pair(Rc::new(RefCell::new(Pair(a, d))))
}
