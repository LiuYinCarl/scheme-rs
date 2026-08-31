//! Core value representation, symbol interning, renaming table (for hygiene).
//!
//! # 设计要点
//!
//! - Pair/String/Vector 用 `Rc<RefCell<...>>`：R5RS 有 `set-car!`、
//!   `string-set!`、`vector-set!` 等变更操作，且同一个对象可以被任意多个
//!   位置共享（`eq?` 比较的就是"是不是同一个对象"）。Rc 提供共享所有权，
//!   RefCell 提供可变性；`eq?` 对这类值就是 `Rc::ptr_eq`。
//! - 符号全部 intern 成 u32（`Sym`）：同名符号共享同一个 id，于是 `eq?`
//!   就是整数比较，环境查找也可以用整数做哈希键。
//! - gensym / rename 表服务于宏卫生：syntax-rules 展开时把模板引入的
//!   标识符替换成新鲜符号（名字里带空格，reader 永远读不出来，不会与
//!   用户符号冲突），并在此登记 (原符号, 定义处环境)，供 env.rs 解析时
//!   回退使用。

use num_bigint::BigInt;
use num_rational::BigRational;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use crate::env::Env;

/// Interned symbol id.
pub type Sym = u32;

struct Interner {
    map: HashMap<String, Sym>,
    names: Vec<String>,
}

impl Interner {
    fn new() -> Self {
        Interner {
            map: HashMap::new(),
            names: Vec::new(),
        }
    }
}

thread_local! {
    static INTERNER: RefCell<Interner> = RefCell::new(Interner::new());
    static RENAMES: RefCell<HashMap<Sym, (Sym, Rc<Env>)>> = RefCell::new(HashMap::new());
    static GENSYM: RefCell<u64> = const { RefCell::new(0) };
}

pub fn intern(s: &str) -> Sym {
    INTERNER.with(|i| {
        let mut i = i.borrow_mut();
        if let Some(&id) = i.map.get(s) {
            return id;
        }
        let id = i.names.len() as Sym;
        i.names.push(s.to_string());
        i.map.insert(s.to_string(), id);
        id
    })
}

pub fn sym_str(s: Sym) -> String {
    INTERNER.with(|i| i.borrow().names[s as usize].clone())
}

/// Fresh, un-parseable symbol (contains a space and a dot).
pub fn gensym(prefix: &str) -> Sym {
    GENSYM.with(|g| {
        let mut g = g.borrow_mut();
        *g += 1;
        intern(&format!(" {}.{}", prefix, *g))
    })
}

/// Create a fresh renamed identifier standing for `orig` resolved in `env`.
///
/// 卫生机制的核心：模板里自由出现的标识符（非模式变量）会被换成这里
/// 生成的 fresh 符号，并记住它"原本是谁、在哪个环境里解释"。
pub fn rename_sym(orig: Sym, env: &Rc<Env>) -> Sym {
    let fresh = gensym(&sym_str(orig));
    RENAMES.with(|r| r.borrow_mut().insert(fresh, (orig, env.clone())));
    fresh
}

pub fn get_rename(s: Sym) -> Option<(Sym, Rc<Env>)> {
    RENAMES.with(|r| r.borrow().get(&s).cloned())
}

/// Human-readable name for error messages: follows the rename chain back to
/// the original identifier (renames come from hygienic macro expansion).
/// 纯显示用途：链异常长时（上限同 env.rs 的 MAX_RENAME_CHAIN）就截断显示
/// 当前名字，不报错——语义判定请走 aux_name，那里触顶会显式报错。
pub fn display_name(s: Sym) -> String {
    let mut cur = s;
    for _ in 0..1000 {
        match get_rename(cur) {
            Some((orig, _)) => cur = orig,
            None => break,
        }
    }
    sym_str(cur)
}

/// Error text for an unbound variable, unwrapping hygienic renames.
pub fn unbound_msg(s: Sym) -> String {
    let shown = display_name(s);
    if shown == sym_str(s) {
        format!("unbound variable: {}", shown)
    } else {
        format!(
            "unbound variable: {} (introduced by a macro template)",
            shown
        )
    }
}

pub struct Pair(pub Value, pub Value);

impl Drop for Pair {
    /// 长 cdr 链（如 10 万元素的表）按默认方式 drop 会沿链递归，深度
    /// 足以撑爆 Rust 栈。这里改为迭代拆链：逐个把后继节点的 cdr 换成
    /// Nil 再释放，递归深度恒为 1。深层 car 树仍可能递归（实践中罕见）。
    fn drop(&mut self) {
        let mut cur = std::mem::replace(&mut self.1, Value::Nil);
        while let Value::Pair(rc) = cur {
            match Rc::try_unwrap(rc) {
                Ok(cell) => {
                    cur = std::mem::replace(&mut cell.into_inner().1, Value::Nil);
                }
                Err(_) => break,
            }
        }
    }
}

pub struct Closure {
    /// 稳定标识：创建时从全局递增计数器分配，终身不变。
    /// trace 等功能用它做 key——不能用 Rc 指针，闭包释放后地址会被
    /// 新闭包复用，残留登记会误伤不相干的闭包。
    pub id: usize,
    pub fixed: Vec<Sym>,
    pub rest: Option<Sym>,
    pub body: Rc<Vec<Value>>,
    pub env: Rc<Env>,
}

thread_local! {
    static CLOSURE_ID: Cell<usize> = const { Cell::new(0) };
}

impl Closure {
    pub fn new(fixed: Vec<Sym>, rest: Option<Sym>, body: Rc<Vec<Value>>, env: Rc<Env>) -> Closure {
        let id = CLOSURE_ID.with(|c| {
            let id = c.get();
            c.set(id + 1);
            id
        });
        Closure {
            id,
            fixed,
            rest,
            body,
            env,
        }
    }
}

pub struct Promise {
    pub forced: bool,
    /// 正在 forcing 的重入深度（0 = 未被 force）。R5RS 的 make-promise
    /// 参考实现允许重入（例如报告的 count/(force p) 示例），但
    /// `(delay (force p))` 这种无进展自引用必须能终止：用深度上限把
    /// 真死循环变成错误。
    pub forcing: u32,
    pub value: Value,
    pub expr: Value,
    pub env: Rc<Env>,
}

/// force 重入深度上限：正常代码（含 R5RS 自引用示例）只需要几层，
/// 超过即视为无进展的自引用死循环。
pub const MAX_FORCE_DEPTH: u32 = 10_000;

#[derive(Clone)]
pub enum Value {
    Nil,
    Bool(bool),
    Int(BigInt),
    Rational(Rc<BigRational>),
    Real(f64),
    Char(char),
    Str(Rc<RefCell<String>>),
    Symbol(Sym),
    Pair(Rc<RefCell<Pair>>),
    Vector(Rc<RefCell<Vec<Value>>>),
    Primitive(&'static str),
    Closure(Rc<Closure>),
    Continuation(Rc<crate::eval::ContObj>),
    Port(Rc<crate::port::Port>),
    Eof,
    Unspecified,
    Promise(Rc<RefCell<Promise>>),
    Values(Rc<Vec<Value>>),
    Env(Rc<Env>),
}

impl Value {
    pub fn sym(s: &str) -> Value {
        Value::Symbol(intern(s))
    }
    pub fn is_truthy(&self) -> bool {
        !matches!(self, Value::Bool(false))
    }
    pub fn is_nil(&self) -> bool {
        matches!(self, Value::Nil)
    }
    pub fn pair_ptr(&self) -> Option<usize> {
        match self {
            Value::Pair(p) => Some(Rc::as_ptr(p) as usize),
            _ => None,
        }
    }
}

pub fn cons(a: Value, d: Value) -> Value {
    Value::Pair(Rc::new(RefCell::new(Pair(a, d))))
}

pub fn list_from_vec(v: Vec<Value>) -> Value {
    let mut out = Value::Nil;
    for x in v.into_iter().rev() {
        out = cons(x, out);
    }
    out
}

/// Convert a (possibly improper) list to (items, tail). Cycles produce None.
pub fn list_to_vec(v: &Value) -> Option<(Vec<Value>, Value)> {
    let mut items = Vec::new();
    let mut cur = v.clone();
    let mut seen: std::collections::HashSet<usize> = std::collections::HashSet::new();
    loop {
        match cur {
            Value::Nil => return Some((items, Value::Nil)),
            Value::Pair(p) => {
                if !seen.insert(Rc::as_ptr(&p) as usize) {
                    return None; // cycle
                }
                let (a, d) = {
                    let b = p.borrow();
                    (b.0.clone(), b.1.clone())
                };
                items.push(a);
                cur = d;
            }
            other => return Some((items, other)),
        }
    }
}

/// Proper list only; None on improper or circular.
pub fn proper_list(v: &Value) -> Option<Vec<Value>> {
    match list_to_vec(v) {
        Some((items, Value::Nil)) => Some(items),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Equality

fn num_eqv(a: &Value, b: &Value) -> bool {
    use Value::*;
    match (a, b) {
        (Int(x), Int(y)) => x == y,
        (Int(x), Rational(y)) | (Rational(y), Int(x)) => {
            &BigRational::from_integer(x.clone()) == y.as_ref()
        }
        (Rational(x), Rational(y)) => x == y,
        (Real(x), Real(y)) => x == y,
        _ => false,
    }
}

pub fn scm_eq(a: &Value, b: &Value) -> bool {
    use Value::*;
    match (a, b) {
        (Nil, Nil) => true,
        (Bool(x), Bool(y)) => x == y,
        (Symbol(x), Symbol(y)) => x == y,
        (Char(x), Char(y)) => x == y,
        (Int(_), _) | (Rational(_), _) | (Real(_), _) => num_eqv(a, b),
        (Str(x), Str(y)) => Rc::ptr_eq(x, y),
        (Pair(x), Pair(y)) => Rc::ptr_eq(x, y),
        (Vector(x), Vector(y)) => Rc::ptr_eq(x, y),
        (Primitive(x), Primitive(y)) => x == y,
        (Closure(x), Closure(y)) => Rc::ptr_eq(x, y),
        (Continuation(x), Continuation(y)) => Rc::ptr_eq(x, y),
        (Port(x), Port(y)) => Rc::ptr_eq(x, y),
        (Eof, Eof) => true,
        (Unspecified, Unspecified) => true,
        (Promise(x), Promise(y)) => Rc::ptr_eq(x, y),
        (Env(x), Env(y)) => Rc::ptr_eq(x, y),
        _ => false,
    }
}

pub fn scm_eqv(a: &Value, b: &Value) -> bool {
    use Value::*;
    match (a, b) {
        (Int(_), _) | (Rational(_), _) | (Real(_), _) => num_eqv(a, b),
        (Str(_), Str(_)) => false, // eqv? on distinct strings is #f unless eq?
        _ => scm_eq(a, b),
    }
}

pub fn scm_equal(a: &Value, b: &Value) -> bool {
    let mut visited: HashMap<usize, usize> = HashMap::new();
    equal_go(a, b, &mut visited)
}

/// visited 是"当前递归路径上"的节点对应表：进入节点前插入 (ptr_a →
/// ptr_b)，离开节点后删除。若比较中再次遇到已在表中的 ptr_a，说明遇到了
/// 环（环形 pair/vector），按"假设同构"处理：对应目标相同返回 true，
/// 不同则说明两边回边结构不同，返回 false。路径外（已比较完）的节点会被
/// 移除，因此 DAG 共享结构每次都会如实比较，不会误判。
fn equal_go(a: &Value, b: &Value, visited: &mut HashMap<usize, usize>) -> bool {
    use Value::*;
    if scm_eqv(a, b) {
        return true;
    }
    match (a, b) {
        (Pair(x), Pair(y)) => {
            let (px, py) = (Rc::as_ptr(x) as usize, Rc::as_ptr(y) as usize);
            if let Some(&seen) = visited.get(&px) {
                return seen == py; // 环：按同构假设判定
            }
            visited.insert(px, py);
            let (xa, xd, ya, yd) = {
                let (bx, by) = (x.borrow(), y.borrow());
                (bx.0.clone(), bx.1.clone(), by.0.clone(), by.1.clone())
            };
            let r = equal_go(&xa, &ya, visited) && equal_go(&xd, &yd, visited);
            visited.remove(&px);
            r
        }
        (Str(x), Str(y)) => *x.borrow() == *y.borrow(),
        (Vector(x), Vector(y)) => {
            let (px, py) = (Rc::as_ptr(x) as usize, Rc::as_ptr(y) as usize);
            if let Some(&seen) = visited.get(&px) {
                return seen == py; // 环：按同构假设判定
            }
            if x.borrow().len() != y.borrow().len() {
                return false;
            }
            visited.insert(px, py);
            let r = {
                let (bx, by) = (x.borrow(), y.borrow());
                bx.iter()
                    .zip(by.iter())
                    .all(|(u, v)| equal_go(u, v, visited))
            };
            visited.remove(&px);
            r
        }
        _ => false,
    }
}
