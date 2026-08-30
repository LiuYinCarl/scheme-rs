//! write/display printer.
//!
//! `write` 输出可读回的外部表示（字符串加引号转义、字符加 #\ 前缀），
//! `display` 输出人读形式。对共享/环形结构：cdr 链上的节点在整条链
//! 打印完之前都留在 `seen` 集合里，回环处打印 `. #<cycle>`；car 方向
//! 递归打印时插入/移除 `seen`，因此 DAG 共享（非环）结构照常展开。

use crate::value::{sym_str, Value};
use std::collections::HashSet;
use std::rc::Rc;

pub fn fmt_real(f: f64) -> String {
    if f.is_nan() {
        return "+nan.0".into();
    }
    if f.is_infinite() {
        return if f > 0.0 {
            "+inf.0".into()
        } else {
            "-inf.0".into()
        };
    }
    if f == f.trunc() && f.abs() < 1e15 {
        return format!("{:.1}", f);
    }
    let s = format!("{}", f);
    if !s.contains('.')
        && !s.contains('e')
        && !s.contains('E')
        && !s.contains("inf")
        && !s.contains("NaN")
    {
        format!("{}.0", s)
    } else {
        s
    }
}

pub fn write_to_string(v: &Value) -> String {
    let mut s = String::new();
    let mut seen = HashSet::new();
    fmt_value(v, true, &mut s, &mut seen);
    s
}

pub fn display_to_string(v: &Value) -> String {
    let mut s = String::new();
    let mut seen = HashSet::new();
    fmt_value(v, false, &mut s, &mut seen);
    s
}

fn fmt_char(c: char, out: &mut String) {
    match c {
        ' ' => out.push_str("#\\space"),
        '\n' => out.push_str("#\\newline"),
        '\t' => out.push_str("#\\tab"),
        '\r' => out.push_str("#\\return"),
        '\0' => out.push_str("#\\null"),
        c => {
            out.push_str("#\\");
            out.push(c);
        }
    }
}

fn fmt_string(s: &str, out: &mut String) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
}

fn fmt_value(v: &Value, write: bool, out: &mut String, seen: &mut HashSet<usize>) {
    match v {
        Value::Nil => out.push_str("()"),
        Value::Bool(true) => out.push_str("#t"),
        Value::Bool(false) => out.push_str("#f"),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Rational(r) => out.push_str(&format!("{}/{}", r.numer(), r.denom())),
        Value::Real(f) => out.push_str(&fmt_real(*f)),
        Value::Char(c) => {
            if write {
                fmt_char(*c, out);
            } else {
                out.push(*c);
            }
        }
        Value::Str(s) => {
            if write {
                fmt_string(&s.borrow(), out);
            } else {
                out.push_str(&s.borrow());
            }
        }
        Value::Symbol(s) => out.push_str(&sym_str(*s)),
        Value::Pair(p) => fmt_pair(p, write, out, seen),
        Value::Vector(items) => {
            let ptr = Rc::as_ptr(items) as usize;
            if !seen.insert(ptr) {
                out.push_str("#<cycle>");
                return;
            }
            out.push_str("#(");
            for (i, x) in items.borrow().iter().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                fmt_value(x, write, out, seen);
            }
            out.push(')');
            seen.remove(&ptr);
        }
        Value::Primitive(name) => out.push_str(&format!("#<procedure {}>", name)),
        Value::Closure(_) => out.push_str("#<procedure>"),
        Value::Continuation(_) => out.push_str("#<continuation>"),
        Value::Port(p) => {
            if p.input {
                out.push_str("#<input-port>");
            } else {
                out.push_str("#<output-port>");
            }
        }
        Value::Eof => out.push_str("#<eof>"),
        Value::Unspecified => out.push_str("#<unspecified>"),
        Value::Promise(_) => out.push_str("#<promise>"),
        Value::Values(xs) => {
            out.push_str("#<values");
            for x in xs.iter() {
                out.push(' ');
                fmt_value(x, write, out, seen);
            }
            out.push('>');
        }
        Value::Env(_) => out.push_str("#<environment>"),
    }
}

fn fmt_pair(
    p: &Rc<std::cell::RefCell<crate::value::Pair>>,
    write: bool,
    out: &mut String,
    seen: &mut HashSet<usize>,
) {
    // quote abbreviations
    {
        let b = p.borrow();
        if let Value::Symbol(s) = &b.0 {
            let name = sym_str(*s);
            let abbrev = match name.as_str() {
                "quote" => Some("'"),
                "quasiquote" => Some("`"),
                "unquote" => Some(","),
                "unquote-splicing" => Some(",@"),
                _ => None,
            };
            if let Some(ab) = abbrev {
                if let Value::Pair(d) = &b.1 {
                    let db = d.borrow();
                    if db.1.is_nil() {
                        out.push_str(ab);
                        fmt_value(&db.0, write, out, seen);
                        return;
                    }
                }
            }
        }
    }
    out.push('(');
    let mut first = true;
    let mut cur = Value::Pair(p.clone());
    // Pairs of the current cdr-chain stay in `seen` until the whole chain is
    // printed, so a tail that loops back into the chain is detected. Cars are
    // printed recursively with their own insert/remove (DAG sharing is fine).
    let mut chain: Vec<usize> = Vec::new();
    loop {
        match cur {
            Value::Pair(pp) => {
                let ptr2 = Rc::as_ptr(&pp) as usize;
                if first {
                    first = false;
                } else {
                    out.push(' ');
                }
                if !seen.insert(ptr2) {
                    out.push_str(". #<cycle>");
                    break;
                }
                chain.push(ptr2);
                let (a, d) = {
                    let b = pp.borrow();
                    (b.0.clone(), b.1.clone())
                };
                fmt_value(&a, write, out, seen);
                cur = d;
            }
            Value::Nil => break,
            other => {
                out.push_str(" . ");
                fmt_value(&other, write, out, seen);
                break;
            }
        }
    }
    for ptr2 in chain {
        seen.remove(&ptr2);
    }
    out.push(')');
}
