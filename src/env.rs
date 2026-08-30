//! Environments: frames mapping symbols to *locations* (Rc<RefCell<Value>>),
//! plus a macro namespace per frame, plus rename-aware resolution for hygiene.
//!
//! # 设计要点
//!
//! 环境存的是 location（值的盒子），不是值本身。这是 letrec + call/cc
//! 正确性的关键：当初始化表达式捕获的续延被重入、而变量又被 `set!`
//! 改过之后，重入者必须通过同一个 location 看到**后写**的值
//! （pitfall 1.1/1.2 就是卡这个；存值的话续延看到的会是旧拷贝）。
//!
//! 每帧有两个命名空间：普通变量（vars）和宏（macros）。它们共存且互相
//! 遮蔽：R5RS 不允许保留字，局部 `(define foo +)` 可以盖住同名宏，
//! 局部宏也可以盖住变量；`resolve` 逐帧先查 vars 再查 macros，
//! 都没命中才回退到内建关键字（见 is_keyword）。
//!
//! 卫生重命名的解析策略：沿环境链找不到 fresh 符号时，查 rename 表，
//! 回到 (原符号, 定义处环境) 重新解析——这样模板引入的 `+`、`if` 等
//! 始终按宏定义处的绑定解释，不会被使用处的局部绑定捕获。

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::syntax_rules::Transformer;
use crate::value::{get_rename, intern, Sym, Value};

pub struct Env {
    pub vars: RefCell<HashMap<Sym, Rc<RefCell<Value>>>>,
    pub macros: RefCell<HashMap<Sym, Rc<Transformer>>>,
    pub parent: Option<Rc<Env>>,
}

impl Env {
    pub fn new(parent: Option<Rc<Env>>) -> Rc<Env> {
        Rc::new(Env {
            vars: RefCell::new(HashMap::new()),
            macros: RefCell::new(HashMap::new()),
            parent,
        })
    }

    /// Define (or redefine) in *this* frame. Redefinition updates the same
    /// location so existing closures observe the new value.
    pub fn define(&self, s: Sym, v: Value) {
        let mut vars = self.vars.borrow_mut();
        if let Some(loc) = vars.get(&s) {
            *loc.borrow_mut() = v;
        } else {
            vars.insert(s, Rc::new(RefCell::new(v)));
        }
    }

    pub fn define_loc(&self, s: Sym, v: Value) -> Rc<RefCell<Value>> {
        let loc = Rc::new(RefCell::new(v));
        self.vars.borrow_mut().insert(s, loc.clone());
        loc
    }

    pub fn define_macro(&self, s: Sym, t: Rc<Transformer>) {
        self.macros.borrow_mut().insert(s, t);
    }
}

pub enum Meaning {
    Var(Rc<RefCell<Value>>),
    Macro(Rc<Transformer>),
    Keyword(Sym),
    Unbound,
}

const KEYWORDS: &[&str] = &[
    "quote",
    "lambda",
    "if",
    "define",
    "set!",
    "begin",
    "cond",
    "case",
    "and",
    "or",
    "let",
    "let*",
    "letrec",
    "do",
    "delay",
    "quasiquote",
    "define-syntax",
    "let-syntax",
    "letrec-syntax",
];

pub fn is_keyword(s: Sym) -> bool {
    KEYWORDS.contains(&crate::value::sym_str(s).as_str())
}

/// Look up a variable location, following renames for hygienic identifiers.
pub fn lookup_var(env: &Rc<Env>, s: Sym) -> Option<Rc<RefCell<Value>>> {
    let mut e = env.clone();
    loop {
        if let Some(l) = e.vars.borrow().get(&s) {
            return Some(l.clone());
        }
        match &e.parent {
            Some(p) => e = p.clone(),
            None => break,
        }
    }
    if let Some((orig, denv)) = get_rename(s) {
        return lookup_var(&denv, orig);
    }
    None
}

pub fn lookup_macro(env: &Rc<Env>, s: Sym) -> Option<Rc<Transformer>> {
    let mut e = env.clone();
    loop {
        if let Some(t) = e.macros.borrow().get(&s) {
            return Some(t.clone());
        }
        match &e.parent {
            Some(p) => e = p.clone(),
            None => break,
        }
    }
    if let Some((orig, denv)) = get_rename(s) {
        return lookup_macro(&denv, orig);
    }
    None
}

/// Resolve an identifier to its meaning: variable, macro, builtin keyword, or
/// unbound. Variable/macro bindings shadow keywords.
pub fn resolve(env: &Rc<Env>, s: Sym) -> Meaning {
    let mut e = env.clone();
    loop {
        if let Some(l) = e.vars.borrow().get(&s) {
            return Meaning::Var(l.clone());
        }
        if let Some(t) = e.macros.borrow().get(&s) {
            return Meaning::Macro(t.clone());
        }
        match &e.parent {
            Some(p) => e = p.clone(),
            None => break,
        }
    }
    if let Some((orig, denv)) = get_rename(s) {
        return resolve(&denv, orig);
    }
    if is_keyword(s) {
        Meaning::Keyword(s)
    } else {
        Meaning::Unbound
    }
}

/// Is this identifier "auxiliary syntax" (unbound / keyword) in this env, or
/// has it been rebound locally (shadowed)?
pub fn is_locally_bound(env: &Rc<Env>, name: &str) -> bool {
    let s = intern(name);
    matches!(resolve(env, s), Meaning::Var(_) | Meaning::Macro(_))
}

/// free-identifier=? : do two identifiers refer to the same binding?
pub fn free_id_eq(s1: Sym, env1: &Rc<Env>, s2: Sym, env2: &Rc<Env>) -> bool {
    let m1 = resolve(env1, s1);
    let m2 = resolve(env2, s2);
    match (&m1, &m2) {
        (Meaning::Var(a), Meaning::Var(b)) => Rc::ptr_eq(a, b),
        (Meaning::Macro(a), Meaning::Macro(b)) => Rc::ptr_eq(a, b),
        (Meaning::Keyword(a), Meaning::Keyword(b)) => a == b,
        (Meaning::Unbound, Meaning::Unbound) => s1 == s2,
        _ => false,
    }
}

/// Resolve an identifier to its "auxiliary syntax" name, following renames.
/// Returns None if the identifier is bound (shadowed) and thus no longer
/// refers to auxiliary syntax (e.g. a locally bound `else`, `=>`, `unquote`).
pub fn aux_name(env: &Rc<Env>, s: Sym) -> Option<String> {
    // bound anywhere in the current chain?
    let mut e = env.clone();
    loop {
        if e.vars.borrow().contains_key(&s) || e.macros.borrow().contains_key(&s) {
            return None;
        }
        match &e.parent {
            Some(p) => e = p.clone(),
            None => break,
        }
    }
    // follow the rename chain to the original identifier
    let mut cur = s;
    for _ in 0..1000 {
        match get_rename(cur) {
            Some((orig, denv)) => {
                if lookup_var(&denv, orig).is_some() || lookup_macro(&denv, orig).is_some() {
                    return None; // bound at the macro definition site
                }
                cur = orig;
            }
            None => break,
        }
    }
    Some(crate::value::sym_str(cur))
}
