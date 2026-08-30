//! Builtin procedures.
//!
//! 内建过程以 `Value::Primitive(&'static str)` 绑定进全局环境，应用时按
//! 名字分派。大多数是纯函数（返回 `State::Return`）；需要驱动求值器的
//! （map/for-each/apply/call/cc/dynamic-wind/force/call-with-values 等）
//! 通过往 Machine 上压续延帧、返回 `State::Apply`/`State::Eval` 来完成，
//! 因此它们同样享受尾调用与续延语义（map 因而是 call/cc 安全的）。

use std::cell::RefCell;
use std::rc::Rc;

use crate::env::Env;
use crate::eval::{ContKind, Machine, State};
use crate::number::{self};
use crate::port::{self, Port};
use crate::printer::{display_to_string, write_to_string};
use crate::reader::{self, CharSource};
use crate::value::{
    cons, intern, list_from_vec, list_to_vec, proper_list, scm_eq, scm_equal, scm_eqv, sym_str,
    Pair, Value,
};

thread_local! {
    static GLOBAL_ENV: RefCell<Option<Rc<Env>>> = const { RefCell::new(None) };
}

pub fn global_env() -> Rc<Env> {
    GLOBAL_ENV.with(|g| g.borrow().clone().expect("global env not initialized"))
}

pub const PRIMITIVES: &[&str] = &[
    // numbers
    "+",
    "-",
    "*",
    "/",
    "=",
    "<",
    ">",
    "<=",
    ">=",
    "quotient",
    "remainder",
    "modulo",
    "gcd",
    "lcm",
    "numerator",
    "denominator",
    "floor",
    "ceiling",
    "truncate",
    "round",
    "rationalize",
    "expt",
    "sqrt",
    "abs",
    "max",
    "min",
    "exact->inexact",
    "inexact->exact",
    "number->string",
    "string->number",
    "zero?",
    "positive?",
    "negative?",
    "odd?",
    "even?",
    "exact?",
    "inexact?",
    "number?",
    "complex?",
    "real?",
    "rational?",
    "integer?",
    "exp",
    "log",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "atan",
    // equivalence
    "eq?",
    "eqv?",
    "equal?",
    // pairs and lists
    "cons",
    "car",
    "cdr",
    "set-car!",
    "set-cdr!",
    "null?",
    "pair?",
    "list?",
    "list",
    "length",
    "append",
    "reverse",
    "list-tail",
    "list-ref",
    "memq",
    "memv",
    "member",
    "assq",
    "assv",
    "assoc",
    // symbols
    "symbol?",
    "symbol->string",
    "string->symbol",
    // chars
    "char?",
    "char=?",
    "char<?",
    "char>?",
    "char<=?",
    "char>=?",
    "char-ci=?",
    "char-ci<?",
    "char-ci>?",
    "char-ci<=?",
    "char-ci>=?",
    "char-alphabetic?",
    "char-numeric?",
    "char-whitespace?",
    "char-upper-case?",
    "char-lower-case?",
    "char->integer",
    "integer->char",
    "char-upcase",
    "char-downcase",
    // strings
    "string?",
    "make-string",
    "string",
    "string-length",
    "string-ref",
    "string-set!",
    "string=?",
    "string-ci=?",
    "string<?",
    "string>?",
    "string<=?",
    "string>=?",
    "string-ci<?",
    "string-ci>?",
    "string-ci<=?",
    "string-ci>=?",
    "substring",
    "string-append",
    "string->list",
    "list->string",
    "string-copy",
    "string-fill!",
    // vectors
    "vector?",
    "make-vector",
    "vector",
    "vector-length",
    "vector-ref",
    "vector-set!",
    "vector->list",
    "list->vector",
    "vector-fill!",
    // control
    "procedure?",
    "apply",
    "map",
    "for-each",
    "force",
    "call-with-current-continuation",
    "call/cc",
    "values",
    "call-with-values",
    "dynamic-wind",
    "eval",
    "scheme-report-environment",
    "null-environment",
    "interaction-environment",
    // io
    "input-port?",
    "output-port?",
    "current-input-port",
    "current-output-port",
    "open-input-file",
    "open-output-file",
    "close-input-port",
    "close-output-port",
    "read",
    "read-char",
    "peek-char",
    "eof-object?",
    "char-ready?",
    "write",
    "display",
    "newline",
    "write-char",
    "load",
    "call-with-input-file",
    "call-with-output-file",
    "with-input-from-file",
    "with-output-to-file",
    "open-input-string",
    "open-output-string",
    "get-output-string",
    "call-with-output-string",
    "flush-output",
    "error",
    // misc
    "not",
    "boolean?",
];

pub fn standard_env() -> Rc<Env> {
    let env = Env::new(None);
    for name in PRIMITIVES {
        env.define(intern(name), Value::Primitive(name));
    }
    // caar..cddddr (all 28 combinations of 2-4 a/d letters)
    for len in 2..=4usize {
        for bits in 0..(1u32 << len) {
            let mut middle = String::new();
            for i in 0..len {
                middle.push(if bits & (1 << i) != 0 { 'd' } else { 'a' });
            }
            // leak to get &'static str; done once at startup
            let name: &'static str = Box::leak(format!("c{}r", middle).into_boxed_str());
            env.define(intern(name), Value::Primitive(name));
        }
    }
    GLOBAL_ENV.with(|g| *g.borrow_mut() = Some(env.clone()));
    env
}

// ---------------------------------------------------------------------------
// Argument helpers

fn arity(name: &str, args: &[Value], n: usize) -> Result<(), String> {
    if args.len() != n {
        Err(format!("{}: expected {} args, got {}", name, n, args.len()))
    } else {
        Ok(())
    }
}

fn want_pair(name: &str, v: &Value) -> Result<Rc<RefCell<Pair>>, String> {
    match v {
        Value::Pair(p) => Ok(p.clone()),
        _ => Err(format!("{}: not a pair: {}", name, write_to_string(v))),
    }
}

fn want_str(name: &str, v: &Value) -> Result<Rc<RefCell<String>>, String> {
    match v {
        Value::Str(s) => Ok(s.clone()),
        _ => Err(format!("{}: not a string: {}", name, write_to_string(v))),
    }
}

fn want_char(name: &str, v: &Value) -> Result<char, String> {
    match v {
        Value::Char(c) => Ok(*c),
        _ => Err(format!("{}: not a char: {}", name, write_to_string(v))),
    }
}

fn want_usize(name: &str, v: &Value) -> Result<usize, String> {
    match v {
        Value::Int(i) => i
            .to_string()
            .parse::<usize>()
            .map_err(|_| format!("{}: bad index: {}", name, i)),
        _ => Err(format!("{}: not an integer: {}", name, write_to_string(v))),
    }
}

fn want_vec(name: &str, v: &Value) -> Result<Rc<RefCell<Vec<Value>>>, String> {
    match v {
        Value::Vector(x) => Ok(x.clone()),
        _ => Err(format!("{}: not a vector: {}", name, write_to_string(v))),
    }
}

fn want_port_in(name: &str, v: &Value) -> Result<Rc<Port>, String> {
    match v {
        Value::Port(p) if p.input => Ok(p.clone()),
        _ => Err(format!("{}: not an input port", name)),
    }
}

fn want_port_out(name: &str, v: &Value) -> Result<Rc<Port>, String> {
    match v {
        Value::Port(p) if p.output => Ok(p.clone()),
        _ => Err(format!("{}: not an output port", name)),
    }
}

fn ret(v: Value) -> Result<State, String> {
    Ok(State::Return(v))
}

fn boolv(b: bool) -> Value {
    Value::Bool(b)
}

fn make_string(s: String) -> Value {
    Value::Str(Rc::new(RefCell::new(s)))
}

// ---------------------------------------------------------------------------
// Dispatch

pub fn dispatch(m: &mut Machine, name: &str, args: Vec<Value>) -> Result<State, String> {
    match name {
        // ---- numbers
        "+" => ret(number::add(&args)?),
        "-" => ret(number::sub(&args)?),
        "*" => ret(number::mul(&args)?),
        "/" => ret(number::div(&args)?),
        "=" | "<" | ">" | "<=" | ">=" => ret(number::compare(name, &args)?),
        "quotient" => ret(number::quotient(&args)?),
        "remainder" => ret(number::remainder(&args)?),
        "modulo" => ret(number::modulo(&args)?),
        "gcd" => ret(number::gcd(&args)?),
        "lcm" => ret(number::lcm(&args)?),
        "numerator" => {
            arity(name, &args, 1)?;
            ret(number::numerator(&args[0])?)
        }
        "denominator" => {
            arity(name, &args, 1)?;
            ret(number::denominator(&args[0])?)
        }
        "floor" => {
            arity(name, &args, 1)?;
            ret(number::floor_op(&args[0])?)
        }
        "ceiling" => {
            arity(name, &args, 1)?;
            ret(number::ceiling_op(&args[0])?)
        }
        "truncate" => {
            arity(name, &args, 1)?;
            ret(number::truncate_op(&args[0])?)
        }
        "round" => {
            arity(name, &args, 1)?;
            ret(number::round_op(&args[0])?)
        }
        "rationalize" => {
            arity(name, &args, 2)?;
            ret(number::rationalize(&args[0], &args[1])?)
        }
        "expt" => {
            arity(name, &args, 2)?;
            ret(number::expt(&args)?)
        }
        "sqrt" => {
            arity(name, &args, 1)?;
            ret(number::sqrt_op(&args[0])?)
        }
        "abs" => {
            arity(name, &args, 1)?;
            ret(number::abs_op(&args[0])?)
        }
        "max" => ret(number::max_min(true, &args)?),
        "min" => ret(number::max_min(false, &args)?),
        "exp" | "log" | "sin" | "cos" | "tan" | "asin" | "acos" => {
            arity(name, &args, 1)?;
            let x = number::to_f64(&args[0])?;
            let f = match name {
                "exp" => x.exp(),
                "log" => x.ln(),
                "sin" => x.sin(),
                "cos" => x.cos(),
                "tan" => x.tan(),
                "asin" => x.asin(),
                _ => x.acos(),
            };
            ret(Value::Real(f))
        }
        "atan" => {
            if args.len() == 2 {
                let y = number::to_f64(&args[0])?;
                let x = number::to_f64(&args[1])?;
                ret(Value::Real(y.atan2(x)))
            } else if args.len() == 1 {
                let x = number::to_f64(&args[0])?;
                ret(Value::Real(x.atan()))
            } else {
                Err("atan: needs 1 or 2 args".into())
            }
        }
        "exact->inexact" => {
            arity(name, &args, 1)?;
            ret(number::exact_to_inexact(&args[0])?)
        }
        "inexact->exact" => {
            arity(name, &args, 1)?;
            ret(number::inexact_to_exact(&args[0])?)
        }
        "number->string" => {
            let radix = if args.len() == 2 {
                want_usize(name, &args[1])? as u32
            } else {
                10
            };
            if args.is_empty() {
                return Err("number->string: needs args".into());
            }
            ret(make_string(number::number_to_string(&args[0], radix)?))
        }
        "string->number" => {
            if args.is_empty() {
                return Err("string->number: needs args".into());
            }
            let s = want_str(name, &args[0])?;
            let radix = if args.len() == 2 {
                want_usize(name, &args[1])? as u32
            } else {
                10
            };
            let v = number::parse_number_radix(&s.borrow(), radix);
            ret(match v {
                Some(v) => v,
                None => Value::Bool(false),
            })
        }
        "zero?" | "positive?" | "negative?" | "odd?" | "even?" | "exact?" | "inexact?"
        | "number?" | "complex?" | "real?" | "rational?" | "integer?" => {
            arity(name, &args, 1)?;
            let v = &args[0];
            let b = match name {
                "number?" | "complex?" | "real?" => number::is_number(v),
                "rational?" => match v {
                    Value::Int(_) | Value::Rational(_) => true,
                    Value::Real(f) => f.is_finite(),
                    _ => false,
                },
                "integer?" => number::is_integer_valued(v),
                "exact?" => number::is_exact(v),
                "inexact?" => matches!(v, Value::Real(_)),
                "zero?" => {
                    number::compare("=", &[v.clone(), Value::Int(num_bigint::BigInt::from(0))])?
                        .is_truthy()
                }
                "positive?" => {
                    number::compare(">", &[v.clone(), Value::Int(num_bigint::BigInt::from(0))])?
                        .is_truthy()
                }
                "negative?" => {
                    number::compare("<", &[v.clone(), Value::Int(num_bigint::BigInt::from(0))])?
                        .is_truthy()
                }
                "odd?" => match v {
                    Value::Int(i) => i % 2 != num_bigint::BigInt::from(0),
                    _ => return Err("odd?: not an integer".into()),
                },
                "even?" => match v {
                    Value::Int(i) => i % 2 == num_bigint::BigInt::from(0),
                    _ => return Err("even?: not an integer".into()),
                },
                _ => unreachable!(),
            };
            ret(boolv(b))
        }

        // ---- equivalence
        "eq?" => {
            arity(name, &args, 2)?;
            ret(boolv(scm_eq(&args[0], &args[1])))
        }
        "eqv?" => {
            arity(name, &args, 2)?;
            ret(boolv(scm_eqv(&args[0], &args[1])))
        }
        "equal?" => {
            arity(name, &args, 2)?;
            ret(boolv(scm_equal(&args[0], &args[1])))
        }

        // ---- pairs / lists
        "cons" => {
            arity(name, &args, 2)?;
            ret(cons(args[0].clone(), args[1].clone()))
        }
        "car" => {
            arity(name, &args, 1)?;
            let p = want_pair(name, &args[0])?;
            let v = p.borrow().0.clone();
            ret(v)
        }
        "cdr" => {
            arity(name, &args, 1)?;
            let p = want_pair(name, &args[0])?;
            let v = p.borrow().1.clone();
            ret(v)
        }
        "set-car!" => {
            arity(name, &args, 2)?;
            let p = want_pair(name, &args[0])?;
            p.borrow_mut().0 = args[1].clone();
            ret(Value::Unspecified)
        }
        "set-cdr!" => {
            arity(name, &args, 2)?;
            let p = want_pair(name, &args[0])?;
            p.borrow_mut().1 = args[1].clone();
            ret(Value::Unspecified)
        }
        "null?" => {
            arity(name, &args, 1)?;
            ret(boolv(args[0].is_nil()))
        }
        "pair?" => {
            arity(name, &args, 1)?;
            ret(boolv(matches!(args[0], Value::Pair(_))))
        }
        "list?" => {
            arity(name, &args, 1)?;
            ret(boolv(proper_list(&args[0]).is_some()))
        }
        "list" => ret(list_from_vec(args)),
        "length" => {
            arity(name, &args, 1)?;
            match proper_list(&args[0]) {
                Some(v) => ret(Value::Int(num_bigint::BigInt::from(v.len()))),
                None => Err("length: improper or circular list".into()),
            }
        }
        "append" => {
            if args.is_empty() {
                return ret(Value::Nil);
            }
            let mut result = args.last().unwrap().clone();
            for l in args[..args.len() - 1].iter().rev() {
                match list_to_vec(l) {
                    Some((items, tail)) => {
                        if !tail.is_nil() {
                            return Err("append: not a list".into());
                        }
                        for x in items.into_iter().rev() {
                            result = cons(x, result);
                        }
                    }
                    None => return Err("append: circular list".into()),
                }
            }
            ret(result)
        }
        "reverse" => {
            arity(name, &args, 1)?;
            match proper_list(&args[0]) {
                Some(v) => {
                    let mut out = Value::Nil;
                    for x in v {
                        out = cons(x, out);
                    }
                    ret(out)
                }
                None => Err("reverse: improper list".into()),
            }
        }
        "list-tail" => {
            arity(name, &args, 2)?;
            let k = want_usize(name, &args[1])?;
            let mut cur = args[0].clone();
            for _ in 0..k {
                match cur {
                    Value::Pair(p) => cur = p.borrow().1.clone(),
                    _ => return Err("list-tail: index out of range".into()),
                }
            }
            ret(cur)
        }
        "list-ref" => {
            arity(name, &args, 2)?;
            let k = want_usize(name, &args[1])?;
            let mut cur = args[0].clone();
            for _ in 0..k {
                match cur {
                    Value::Pair(p) => cur = p.borrow().1.clone(),
                    _ => return Err("list-ref: index out of range".into()),
                }
            }
            match cur {
                Value::Pair(p) => ret(p.borrow().0.clone()),
                _ => Err("list-ref: index out of range".into()),
            }
        }
        "memq" | "memv" | "member" => {
            arity(name, &args, 2)?;
            let mut cur = args[1].clone();
            loop {
                match cur {
                    Value::Pair(p) => {
                        let (a, d) = {
                            let b = p.borrow();
                            (b.0.clone(), b.1.clone())
                        };
                        let hit = match name {
                            "memq" => scm_eq(&args[0], &a),
                            "memv" => scm_eqv(&args[0], &a),
                            _ => scm_equal(&args[0], &a),
                        };
                        if hit {
                            return ret(Value::Pair(p.clone()));
                        }
                        cur = d;
                    }
                    Value::Nil => return ret(Value::Bool(false)),
                    _ => return Err(format!("{}: improper list", name)),
                }
            }
        }
        "assq" | "assv" | "assoc" => {
            arity(name, &args, 2)?;
            let mut cur = args[1].clone();
            loop {
                match cur {
                    Value::Pair(p) => {
                        let (a, d) = {
                            let b = p.borrow();
                            (b.0.clone(), b.1.clone())
                        };
                        if let Value::Pair(entry) = &a {
                            let key = entry.borrow().0.clone();
                            let hit = match name {
                                "assq" => scm_eq(&args[0], &key),
                                "assv" => scm_eqv(&args[0], &key),
                                _ => scm_equal(&args[0], &key),
                            };
                            if hit {
                                return ret(a);
                            }
                        }
                        cur = d;
                    }
                    Value::Nil => return ret(Value::Bool(false)),
                    _ => return Err(format!("{}: improper list", name)),
                }
            }
        }

        // ---- symbols
        "symbol?" => {
            arity(name, &args, 1)?;
            ret(boolv(matches!(args[0], Value::Symbol(_))))
        }
        "symbol->string" => {
            arity(name, &args, 1)?;
            match &args[0] {
                Value::Symbol(s) => ret(make_string(sym_str(*s))),
                _ => Err("symbol->string: not a symbol".into()),
            }
        }
        "string->symbol" => {
            arity(name, &args, 1)?;
            let s = want_str(name, &args[0])?;
            let t = s.borrow().clone();
            ret(Value::Symbol(intern(&t)))
        }

        // ---- chars
        "char?" => {
            arity(name, &args, 1)?;
            ret(boolv(matches!(args[0], Value::Char(_))))
        }
        "char=?" | "char<?" | "char>?" | "char<=?" | "char>=?" | "char-ci=?" | "char-ci<?"
        | "char-ci>?" | "char-ci<=?" | "char-ci>=?" => {
            if args.len() < 2 {
                return ret(Value::Bool(true));
            }
            let ci = name.starts_with("char-ci");
            let norm = |c: char| -> String {
                if ci {
                    c.to_lowercase().collect()
                } else {
                    c.to_string()
                }
            };
            let mut result = true;
            for w in args.windows(2) {
                let a = norm(want_char(name, &w[0])?);
                let b = norm(want_char(name, &w[1])?);
                let suffix = name
                    .trim_start_matches("char-ci")
                    .trim_start_matches("char");
                let ok = match suffix {
                    "=?" => a == b,
                    "<?" => a < b,
                    ">?" => a > b,
                    "<=?" => a <= b,
                    ">=?" => a >= b,
                    _ => unreachable!(),
                };
                result = result && ok;
            }
            ret(boolv(result))
        }
        "char-alphabetic?" | "char-numeric?" | "char-whitespace?" | "char-upper-case?"
        | "char-lower-case?" => {
            arity(name, &args, 1)?;
            let c = want_char(name, &args[0])?;
            let b = match name {
                "char-alphabetic?" => c.is_alphabetic(),
                "char-numeric?" => c.is_numeric(),
                "char-whitespace?" => c.is_whitespace(),
                "char-upper-case?" => c.is_uppercase(),
                _ => c.is_lowercase(),
            };
            ret(boolv(b))
        }
        "char->integer" => {
            arity(name, &args, 1)?;
            let c = want_char(name, &args[0])?;
            ret(Value::Int(num_bigint::BigInt::from(c as u32)))
        }
        "integer->char" => {
            arity(name, &args, 1)?;
            let n = want_usize(name, &args[0])?;
            match char::from_u32(n as u32) {
                Some(c) => ret(Value::Char(c)),
                None => Err("integer->char: bad code point".into()),
            }
        }
        "char-upcase" => {
            arity(name, &args, 1)?;
            let c = want_char(name, &args[0])?;
            ret(Value::Char(c.to_uppercase().next().unwrap_or(c)))
        }
        "char-downcase" => {
            arity(name, &args, 1)?;
            let c = want_char(name, &args[0])?;
            ret(Value::Char(c.to_lowercase().next().unwrap_or(c)))
        }

        // ---- strings
        "string?" => {
            arity(name, &args, 1)?;
            ret(boolv(matches!(args[0], Value::Str(_))))
        }
        "make-string" => {
            if args.is_empty() || args.len() > 2 {
                return Err("make-string: bad args".into());
            }
            let k = want_usize(name, &args[0])?;
            let fill = if args.len() == 2 {
                want_char(name, &args[1])?
            } else {
                ' '
            };
            ret(make_string(std::iter::repeat_n(fill, k).collect()))
        }
        "string" => {
            let mut s = String::new();
            for a in &args {
                s.push(want_char(name, a)?);
            }
            ret(make_string(s))
        }
        "string-length" => {
            arity(name, &args, 1)?;
            let s = want_str(name, &args[0])?;
            let n = s.borrow().chars().count();
            ret(Value::Int(num_bigint::BigInt::from(n)))
        }
        "string-ref" => {
            arity(name, &args, 2)?;
            let s = want_str(name, &args[0])?;
            let k = want_usize(name, &args[1])?;
            let c = s.borrow().chars().nth(k);
            match c {
                Some(c) => ret(Value::Char(c)),
                None => Err("string-ref: index out of range".into()),
            }
        }
        "string-set!" => {
            arity(name, &args, 3)?;
            let s = want_str(name, &args[0])?;
            let k = want_usize(name, &args[1])?;
            let c = want_char(name, &args[2])?;
            let mut chars: Vec<char> = s.borrow().chars().collect();
            if k >= chars.len() {
                return Err("string-set!: index out of range".into());
            }
            chars[k] = c;
            *s.borrow_mut() = chars.into_iter().collect();
            ret(Value::Unspecified)
        }
        "string=?" | "string-ci=?" | "string<?" | "string>?" | "string<=?" | "string>=?"
        | "string-ci<?" | "string-ci>?" | "string-ci<=?" | "string-ci>=?" => {
            if args.len() < 2 {
                return ret(Value::Bool(true));
            }
            let ci = name.starts_with("string-ci");
            let norm = |v: &Value| -> Result<String, String> {
                let s = want_str(name, v)?;
                let b = s.borrow();
                Ok(if ci { b.to_lowercase() } else { b.clone() })
            };
            let mut result = true;
            for w in args.windows(2) {
                let a = norm(&w[0])?;
                let b = norm(&w[1])?;
                let suffix = name
                    .trim_start_matches("string-ci")
                    .trim_start_matches("string");
                let ok = match suffix {
                    "=?" => a == b,
                    "<?" => a < b,
                    ">?" => a > b,
                    "<=?" => a <= b,
                    ">=?" => a >= b,
                    _ => unreachable!(),
                };
                result = result && ok;
            }
            ret(boolv(result))
        }
        "substring" => {
            arity(name, &args, 3)?;
            let s = want_str(name, &args[0])?;
            let start = want_usize(name, &args[1])?;
            let end = want_usize(name, &args[2])?;
            let chars: Vec<char> = s.borrow().chars().collect();
            if end > chars.len() || start > end {
                return Err("substring: bad range".into());
            }
            ret(make_string(chars[start..end].iter().collect()))
        }
        "string-append" => {
            let mut out = String::new();
            for a in &args {
                out.push_str(&want_str(name, a)?.borrow());
            }
            ret(make_string(out))
        }
        "string->list" => {
            arity(name, &args, 1)?;
            let s = want_str(name, &args[0])?;
            let chars: Vec<Value> = s.borrow().chars().map(Value::Char).collect();
            ret(list_from_vec(chars))
        }
        "list->string" => {
            arity(name, &args, 1)?;
            let items =
                proper_list(&args[0]).ok_or_else(|| "list->string: not a list".to_string())?;
            let mut s = String::new();
            for it in items {
                s.push(want_char(name, &it)?);
            }
            ret(make_string(s))
        }
        "string-copy" => {
            arity(name, &args, 1)?;
            let s = want_str(name, &args[0])?;
            let c = s.borrow().clone();
            ret(make_string(c))
        }
        "string-fill!" => {
            arity(name, &args, 2)?;
            let s = want_str(name, &args[0])?;
            let c = want_char(name, &args[1])?;
            let n = s.borrow().chars().count();
            *s.borrow_mut() = std::iter::repeat_n(c, n).collect();
            ret(Value::Unspecified)
        }

        // ---- vectors
        "vector?" => {
            arity(name, &args, 1)?;
            ret(boolv(matches!(args[0], Value::Vector(_))))
        }
        "make-vector" => {
            if args.is_empty() || args.len() > 2 {
                return Err("make-vector: bad args".into());
            }
            let k = want_usize(name, &args[0])?;
            let fill = if args.len() == 2 {
                args[1].clone()
            } else {
                Value::Int(num_bigint::BigInt::from(0))
            };
            ret(Value::Vector(Rc::new(RefCell::new(vec![fill; k]))))
        }
        "vector" => ret(Value::Vector(Rc::new(RefCell::new(args)))),
        "vector-length" => {
            arity(name, &args, 1)?;
            let v = want_vec(name, &args[0])?;
            let n = v.borrow().len();
            ret(Value::Int(num_bigint::BigInt::from(n)))
        }
        "vector-ref" => {
            arity(name, &args, 2)?;
            let v = want_vec(name, &args[0])?;
            let k = want_usize(name, &args[1])?;
            let x = v.borrow().get(k).cloned();
            match x {
                Some(x) => ret(x),
                None => Err("vector-ref: index out of range".into()),
            }
        }
        "vector-set!" => {
            arity(name, &args, 3)?;
            let v = want_vec(name, &args[0])?;
            let k = want_usize(name, &args[1])?;
            let mut b = v.borrow_mut();
            if k >= b.len() {
                return Err("vector-set!: index out of range".into());
            }
            b[k] = args[2].clone();
            ret(Value::Unspecified)
        }
        "vector->list" => {
            arity(name, &args, 1)?;
            let v = want_vec(name, &args[0])?;
            let items = v.borrow().clone();
            ret(list_from_vec(items))
        }
        "list->vector" => {
            arity(name, &args, 1)?;
            let items =
                proper_list(&args[0]).ok_or_else(|| "list->vector: not a list".to_string())?;
            ret(Value::Vector(Rc::new(RefCell::new(items))))
        }
        "vector-fill!" => {
            arity(name, &args, 2)?;
            let v = want_vec(name, &args[0])?;
            for x in v.borrow_mut().iter_mut() {
                *x = args[1].clone();
            }
            ret(Value::Unspecified)
        }

        // ---- control
        "procedure?" => {
            arity(name, &args, 1)?;
            ret(boolv(matches!(
                args[0],
                Value::Primitive(_) | Value::Closure(_) | Value::Continuation(_)
            )))
        }
        "apply" => {
            if args.len() < 2 {
                return Err("apply: needs at least 2 args".into());
            }
            let proc = args[0].clone();
            let mut call_args: Vec<Value> = args[1..args.len() - 1].to_vec();
            match list_to_vec(args.last().unwrap()) {
                Some((items, tail)) => {
                    if !tail.is_nil() {
                        return Err("apply: last argument not a list".into());
                    }
                    call_args.extend(items);
                }
                None => return Err("apply: circular list".into()),
            }
            Ok(State::Apply(proc, call_args))
        }
        "map" | "for-each" => {
            if args.len() < 2 {
                return Err(format!("{}: needs at least 2 args", name));
            }
            let f = args[0].clone();
            let lists: Vec<Value> = args[1..].to_vec();
            crate::eval::kick_map(m, f, lists, name == "map")
        }
        "force" => {
            arity(name, &args, 1)?;
            match &args[0] {
                Value::Promise(p) => {
                    if p.borrow().forced {
                        ret(p.borrow().value.clone())
                    } else {
                        // R5RS 允许 forcing 中重入（参考实现的 count 示例），
                        // 但深度超限即无进展自引用，报错终止。
                        {
                            let mut b = p.borrow_mut();
                            if b.forcing >= crate::value::MAX_FORCE_DEPTH {
                                return Err(
                                    "force: re-entrant promise (forcing in progress)".into()
                                );
                            }
                            b.forcing += 1;
                        }
                        let (expr, env) = {
                            let b = p.borrow();
                            (b.expr.clone(), b.env.clone())
                        };
                        m.push(ContKind::Force { promise: p.clone() });
                        Ok(State::Eval(expr, env))
                    }
                }
                other => ret(other.clone()),
            }
        }
        "call-with-current-continuation" | "call/cc" => {
            arity(name, &args, 1)?;
            let k = Value::Continuation(Rc::new(crate::eval::ContObj {
                cont: m.cont.clone(),
                winds: m.winds.clone(),
            }));
            Ok(State::Apply(args[0].clone(), vec![k]))
        }
        "values" => {
            if args.len() == 1 {
                ret(args.into_iter().next().unwrap())
            } else {
                ret(Value::Values(Rc::new(args)))
            }
        }
        "call-with-values" => {
            arity(name, &args, 2)?;
            let consumer = args[1].clone();
            m.push(ContKind::CallWithValues { consumer });
            Ok(State::Apply(args[0].clone(), vec![]))
        }
        "dynamic-wind" => {
            arity(name, &args, 3)?;
            m.push(ContKind::DynWindBefore {
                before: args[0].clone(),
                thunk: args[1].clone(),
                after: args[2].clone(),
            });
            Ok(State::Apply(args[0].clone(), vec![]))
        }
        "eval" => {
            arity(name, &args, 2)?;
            let env = match &args[1] {
                Value::Env(e) => e.clone(),
                _ => return Err("eval: not an environment specifier".into()),
            };
            let v = crate::eval::run(State::Eval(args[0].clone(), env))?;
            ret(v)
        }
        "scheme-report-environment" | "null-environment" | "interaction-environment" => {
            ret(Value::Env(global_env()))
        }

        // ---- io
        "input-port?" => {
            arity(name, &args, 1)?;
            ret(boolv(matches!(&args[0], Value::Port(p) if p.input)))
        }
        "output-port?" => {
            arity(name, &args, 1)?;
            ret(boolv(matches!(&args[0], Value::Port(p) if p.output)))
        }
        "current-input-port" => ret(Value::Port(port::current_input())),
        "current-output-port" => ret(Value::Port(port::current_output())),
        "open-input-file" => {
            arity(name, &args, 1)?;
            let s = want_str(name, &args[0])?;
            let path = s.borrow().clone();
            ret(Value::Port(Port::open_input_file(&path)?))
        }
        "open-output-file" => {
            arity(name, &args, 1)?;
            let s = want_str(name, &args[0])?;
            let path = s.borrow().clone();
            ret(Value::Port(Port::open_output_file(&path)?))
        }
        "close-input-port" | "close-output-port" => {
            arity(name, &args, 1)?;
            match &args[0] {
                Value::Port(p) => p.close(),
                _ => return Err(format!("{}: not a port", name)),
            }
            ret(Value::Unspecified)
        }
        "read" => {
            let p = if args.is_empty() {
                port::current_input()
            } else {
                want_port_in(name, &args[0])?
            };
            let mut src = PortSource(p);
            match reader::read_datum(&mut src) {
                Ok(v) => ret(v),
                Err(reader::ReadError::Eof) => ret(Value::Eof),
                Err(reader::ReadError::Msg(m)) => Err(format!("read: {}", m)),
            }
        }
        "read-char" => {
            let p = if args.is_empty() {
                port::current_input()
            } else {
                want_port_in(name, &args[0])?
            };
            match p.read_char() {
                Some(c) => ret(Value::Char(c)),
                None => ret(Value::Eof),
            }
        }
        "peek-char" => {
            let p = if args.is_empty() {
                port::current_input()
            } else {
                want_port_in(name, &args[0])?
            };
            match p.peek_char() {
                Some(c) => ret(Value::Char(c)),
                None => ret(Value::Eof),
            }
        }
        "eof-object?" => {
            arity(name, &args, 1)?;
            ret(boolv(matches!(args[0], Value::Eof)))
        }
        "char-ready?" => {
            let _p = if args.is_empty() {
                port::current_input()
            } else {
                want_port_in(name, &args[0])?
            };
            ret(Value::Bool(true))
        }
        "write" | "display" => {
            if args.is_empty() {
                return Err(format!("{}: needs an argument", name));
            }
            let p = if args.len() >= 2 {
                want_port_out(name, &args[1])?
            } else {
                port::current_output()
            };
            let s = if name == "write" {
                write_to_string(&args[0])
            } else {
                display_to_string(&args[0])
            };
            p.write_str(&s)?;
            ret(Value::Unspecified)
        }
        "newline" => {
            let p = if args.is_empty() {
                port::current_output()
            } else {
                want_port_out(name, &args[0])?
            };
            p.write_str("\n")?;
            ret(Value::Unspecified)
        }
        "write-char" => {
            if args.is_empty() {
                return Err("write-char: needs an argument".into());
            }
            let c = want_char(name, &args[0])?;
            let p = if args.len() >= 2 {
                want_port_out(name, &args[1])?
            } else {
                port::current_output()
            };
            p.write_str(&c.to_string())?;
            ret(Value::Unspecified)
        }
        "load" => {
            arity(name, &args, 1)?;
            let s = want_str(name, &args[0])?;
            let path = s.borrow().clone();
            let content =
                std::fs::read_to_string(&path).map_err(|e| format!("load: {}: {}", path, e))?;
            let forms = reader::read_all(&content).map_err(|e| match e {
                reader::ReadError::Eof => "load: unexpected eof".to_string(),
                reader::ReadError::Msg(m) => format!("load: {}", m),
            })?;
            if forms.is_empty() {
                return ret(Value::Unspecified);
            }
            let env = global_env();
            let mut f = forms;
            let first = f.remove(0);
            m.push(ContKind::Load {
                rest: f,
                env: env.clone(),
            });
            Ok(State::Eval(first, env))
        }
        "call-with-input-file" | "call-with-output-file" => {
            arity(name, &args, 2)?;
            let s = want_str(name, &args[0])?;
            let path = s.borrow().clone();
            let p = if name == "call-with-input-file" {
                Port::open_input_file(&path)?
            } else {
                Port::open_output_file(&path)?
            };
            m.push(ContKind::ClosePortAfter { port: p.clone() });
            Ok(State::Apply(args[1].clone(), vec![Value::Port(p)]))
        }
        "with-input-from-file" | "with-output-to-file" => {
            arity(name, &args, 2)?;
            let s = want_str(name, &args[0])?;
            let path = s.borrow().clone();
            let saved_in = port::current_input();
            let saved_out = port::current_output();
            if name == "with-input-from-file" {
                port::set_current_input(Port::open_input_file(&path)?);
            } else {
                port::set_current_output(Port::open_output_file(&path)?);
            }
            m.push(ContKind::RestorePorts {
                saved_in,
                saved_out,
            });
            Ok(State::Apply(args[1].clone(), vec![]))
        }
        "open-input-string" => {
            arity(name, &args, 1)?;
            let s = want_str(name, &args[0])?;
            let t = s.borrow().clone();
            ret(Value::Port(Port::open_input_string(&t)))
        }
        "open-output-string" => ret(Value::Port(Port::open_output_string())),
        "get-output-string" => {
            arity(name, &args, 1)?;
            match &args[0] {
                Value::Port(p) => ret(make_string(p.get_output_string()?)),
                _ => Err("get-output-string: not a port".into()),
            }
        }
        "call-with-output-string" => {
            arity(name, &args, 1)?;
            let p = Port::open_output_string();
            m.push(ContKind::GetOutputString { port: p.clone() });
            Ok(State::Apply(args[0].clone(), vec![Value::Port(p)]))
        }
        "flush-output" => {
            let p = if args.is_empty() {
                port::current_output()
            } else {
                want_port_out(name, &args[0])?
            };
            p.flush()?;
            ret(Value::Unspecified)
        }
        "error" => {
            let mut msg = String::from("error:");
            for a in &args {
                msg.push(' ');
                msg.push_str(&display_to_string(a));
            }
            Err(msg)
        }

        // ---- misc
        "not" => {
            arity(name, &args, 1)?;
            ret(boolv(!args[0].is_truthy()))
        }
        "boolean?" => {
            arity(name, &args, 1)?;
            ret(boolv(matches!(args[0], Value::Bool(_))))
        }

        other => cxr_dispatch(other, &args),
    }
}

struct PortSource(Rc<Port>);

impl CharSource for PortSource {
    fn peek_char(&mut self) -> Option<char> {
        self.0.peek_char()
    }
    fn next_char(&mut self) -> Option<char> {
        self.0.read_char()
    }
}

/// Generic caar..cddddr (2-4 letters between c and r).
fn cxr_dispatch(name: &str, args: &[Value]) -> Result<State, String> {
    let bytes = name.as_bytes();
    let n = bytes.len();
    if (4..=6).contains(&n)
        && bytes[0] == b'c'
        && bytes[n - 1] == b'r'
        && bytes[1..n - 1].iter().all(|b| *b == b'a' || *b == b'd')
    {
        arity(name, args, 1)?;
        let mut v = args[0].clone();
        for b in bytes[1..n - 1].iter().rev() {
            let p = want_pair(name, &v)?;
            v = if *b == b'a' {
                p.borrow().0.clone()
            } else {
                p.borrow().1.clone()
            };
        }
        ret(v)
    } else {
        Err(format!("unbound variable: {}", name))
    }
}
