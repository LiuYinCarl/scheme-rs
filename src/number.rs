//! Numeric tower: exact integers (BigInt), exact rationals (BigRational),
//! inexact reals (f64). Complex numbers are intentionally omitted.
//!
//! # 设计要点（规则出处均为 R5RS 6.2 节）
//!
//! - 精确有理数始终约分存储（`norm` 在每次运算后规范化，分母为 1 时
//!   退化为整数），这样 `equal?`/`eqv?` 直接比较分子分母即可。
//! - 精确/inexact 传染：一次运算中只要有一个操作数是 inexact (f64)，
//!   结果一律走 f64；否则全程用 BigRational 精确计算（6.2.2）。
//! - `round` 是"逢半取偶"（round half to even，6.2.5 明确要求：
//!   (round 7/2) ⇒ 4, (round 5/2) ⇒ 2）。
//! - `quotient/remainder/modulo/gcd/lcm` 接受整数值的 inexact 参数，
//!   结果保持 inexact（6.2.5：(remainder -13 -4.0) ⇒ -1.0）。
//! - 数字字面量里的 `#` 是"未指定数字"（6.2.4），出现即为 inexact。

use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, Signed, ToPrimitive, Zero};

use crate::value::Value;

pub fn is_number(v: &Value) -> bool {
    matches!(v, Value::Int(_) | Value::Rational(_) | Value::Real(_))
}

pub fn is_exact(v: &Value) -> bool {
    matches!(v, Value::Int(_) | Value::Rational(_))
}

/// Normalize a rational: denominator 1 becomes an Int.
pub fn norm(r: BigRational) -> Value {
    if r.is_integer() {
        Value::Int(r.to_integer())
    } else {
        Value::Rational(std::rc::Rc::new(r))
    }
}

pub fn to_exact(v: &Value) -> Option<BigRational> {
    match v {
        Value::Int(i) => Some(BigRational::from_integer(i.clone())),
        Value::Rational(r) => Some(r.as_ref().clone()),
        _ => None,
    }
}

pub fn to_f64(v: &Value) -> Result<f64, String> {
    match v {
        Value::Int(i) => Ok(i.to_f64().unwrap_or(f64::INFINITY)),
        Value::Rational(r) => Ok(r.to_f64().unwrap_or(f64::INFINITY)),
        Value::Real(f) => Ok(*f),
        _ => Err(format!(
            "not a number: {}",
            crate::printer::write_to_string(v)
        )),
    }
}

fn to_f64_lossy(v: &Value) -> f64 {
    match v {
        Value::Int(i) => i.to_f64().unwrap_or(f64::INFINITY),
        Value::Rational(r) => r.to_f64().unwrap_or(f64::INFINITY),
        Value::Real(f) => *f,
        _ => f64::NAN,
    }
}

pub fn want_num(name: &str, v: &Value) -> Result<(), String> {
    if is_number(v) {
        Ok(())
    } else {
        Err(format!(
            "{}: not a number: {}",
            name,
            crate::printer::write_to_string(v)
        ))
    }
}

// ---------------------------------------------------------------------------
// Arithmetic

pub fn add(args: &[Value]) -> Result<Value, String> {
    if args.iter().any(|a| matches!(a, Value::Real(_))) {
        let mut s = 0.0;
        for a in args {
            s += to_f64(a)?;
        }
        return Ok(Value::Real(s));
    }
    let mut s = BigRational::zero();
    for a in args {
        s += to_exact(a).ok_or_else(|| "+: not a number".to_string())?;
    }
    Ok(norm(s))
}

pub fn sub(args: &[Value]) -> Result<Value, String> {
    if args.is_empty() {
        return Err("-: needs at least one argument".into());
    }
    if args.iter().any(|a| matches!(a, Value::Real(_))) {
        let mut it = args.iter();
        let mut s = to_f64(it.next().unwrap())?;
        if args.len() == 1 {
            return Ok(Value::Real(-s));
        }
        for a in it {
            s -= to_f64(a)?;
        }
        return Ok(Value::Real(s));
    }
    let first = to_exact(&args[0]).ok_or("-: not a number")?;
    if args.len() == 1 {
        return Ok(norm(-first));
    }
    let mut s = first;
    for a in &args[1..] {
        s -= to_exact(a).ok_or("-: not a number")?;
    }
    Ok(norm(s))
}

pub fn mul(args: &[Value]) -> Result<Value, String> {
    if args.iter().any(|a| matches!(a, Value::Real(_))) {
        let mut s = 1.0;
        for a in args {
            s *= to_f64(a)?;
        }
        return Ok(Value::Real(s));
    }
    let mut s = BigRational::one();
    for a in args {
        s *= to_exact(a).ok_or("*: not a number")?;
    }
    Ok(norm(s))
}

pub fn div(args: &[Value]) -> Result<Value, String> {
    if args.is_empty() {
        return Err("/: needs at least one argument".into());
    }
    if args.iter().any(|a| matches!(a, Value::Real(_))) {
        let mut it = args.iter();
        let mut s = to_f64(it.next().unwrap())?;
        if args.len() == 1 {
            return Ok(Value::Real(1.0 / s));
        }
        for a in it {
            s /= to_f64(a)?;
        }
        return Ok(Value::Real(s));
    }
    let first = to_exact(&args[0]).ok_or("/: not a number")?;
    let mut s = first;
    let divide = |s: BigRational, a: &Value| -> Result<BigRational, String> {
        let d = to_exact(a).ok_or("/: not a number")?;
        if d.is_zero() {
            return Err("/: division by zero".into());
        }
        Ok(s / d)
    };
    if args.len() == 1 {
        return Ok(norm(divide(BigRational::one(), &args[0])?));
    }
    for a in &args[1..] {
        s = divide(s, a)?;
    }
    Ok(norm(s))
}

// ---------------------------------------------------------------------------
// Comparisons

fn cmp_exact(a: &BigRational, b: &BigRational) -> std::cmp::Ordering {
    a.cmp(b)
}

pub fn compare(op: &str, args: &[Value]) -> Result<Value, String> {
    if args.len() < 2 {
        return Ok(Value::Bool(true));
    }
    let inexact = args.iter().any(|a| matches!(a, Value::Real(_)));
    let mut result = true;
    for w in args.windows(2) {
        want_num(op, &w[0])?;
        want_num(op, &w[1])?;
        let ord = if inexact {
            to_f64_lossy(&w[0])
                .partial_cmp(&to_f64_lossy(&w[1]))
                .unwrap_or(std::cmp::Ordering::Equal)
        } else {
            cmp_exact(&to_exact(&w[0]).unwrap(), &to_exact(&w[1]).unwrap())
        };
        let ok = match op {
            "=" => ord == std::cmp::Ordering::Equal,
            "<" => ord == std::cmp::Ordering::Less,
            ">" => ord == std::cmp::Ordering::Greater,
            "<=" => ord != std::cmp::Ordering::Greater,
            ">=" => ord != std::cmp::Ordering::Less,
            _ => unreachable!(),
        };
        result = result && ok;
    }
    Ok(Value::Bool(result))
}

// ---------------------------------------------------------------------------
// Integer division

/// Accepts exact integers and inexact integer-valued reals (R5RS 6.2.5:
/// "If any of the arguments is inexact, the result is inexact").
/// Returns (value, is_inexact).
fn want_integer(name: &str, v: &Value) -> Result<(BigInt, bool), String> {
    match v {
        Value::Int(i) => Ok((i.clone(), false)),
        Value::Real(f) if f.fract() == 0.0 && f.is_finite() => {
            let r = BigRational::from_float(*f)
                .ok_or_else(|| format!("{}: not an integer: {}", name, f))?;
            Ok((r.to_integer(), true))
        }
        _ => Err(format!(
            "{}: not an integer: {}",
            name,
            crate::printer::write_to_string(v)
        )),
    }
}

fn int_result(i: BigInt, inexact: bool) -> Value {
    if inexact {
        Value::Real(i.to_f64().unwrap_or(f64::INFINITY))
    } else {
        Value::Int(i)
    }
}

pub fn quotient(args: &[Value]) -> Result<Value, String> {
    let (a, ia) = want_integer("quotient", &args[0])?;
    let (b, ib) = want_integer("quotient", &args[1])?;
    if b.is_zero() {
        return Err("quotient: division by zero".into());
    }
    Ok(int_result(a / b, ia || ib))
}

pub fn remainder(args: &[Value]) -> Result<Value, String> {
    let (a, ia) = want_integer("remainder", &args[0])?;
    let (b, ib) = want_integer("remainder", &args[1])?;
    if b.is_zero() {
        return Err("remainder: division by zero".into());
    }
    Ok(int_result(a % b, ia || ib))
}

pub fn modulo(args: &[Value]) -> Result<Value, String> {
    let (a, ia) = want_integer("modulo", &args[0])?;
    let (b, ib) = want_integer("modulo", &args[1])?;
    if b.is_zero() {
        return Err("modulo: division by zero".into());
    }
    let r = &a % &b;
    // result has the sign of the divisor
    let m = if r.is_zero() || r.sign() == b.sign() {
        r
    } else {
        r + &b
    };
    Ok(int_result(m, ia || ib))
}

fn gcd2(a: BigInt, b: BigInt) -> BigInt {
    let (mut a, mut b) = (a.abs(), b.abs());
    while !b.is_zero() {
        let t = &a % &b;
        a = b;
        b = t;
    }
    a
}

pub fn gcd(args: &[Value]) -> Result<Value, String> {
    let mut g = BigInt::zero();
    let mut inexact = false;
    for a in args {
        let (v, i) = want_integer("gcd", a)?;
        inexact = inexact || i;
        g = gcd2(g, v);
    }
    Ok(int_result(g, inexact))
}

pub fn lcm(args: &[Value]) -> Result<Value, String> {
    let mut l = BigInt::one();
    let mut inexact = false;
    for a in args {
        let (v, i) = want_integer("lcm", a)?;
        inexact = inexact || i;
        if v.is_zero() {
            return Ok(int_result(BigInt::zero(), inexact));
        }
        let g = gcd2(l.clone(), v.clone());
        l = (l / g) * v.abs();
    }
    Ok(int_result(l, inexact))
}

// ---------------------------------------------------------------------------
// Rounding etc.

pub fn floor_op(v: &Value) -> Result<Value, String> {
    match v {
        Value::Int(_) => Ok(v.clone()),
        Value::Rational(r) => Ok(Value::Int(r.floor().to_integer())),
        Value::Real(f) => Ok(Value::Real(f.floor())),
        _ => Err("floor: not a number".into()),
    }
}

pub fn ceiling_op(v: &Value) -> Result<Value, String> {
    match v {
        Value::Int(_) => Ok(v.clone()),
        Value::Rational(r) => Ok(Value::Int(r.ceil().to_integer())),
        Value::Real(f) => Ok(Value::Real(f.ceil())),
        _ => Err("ceiling: not a number".into()),
    }
}

pub fn truncate_op(v: &Value) -> Result<Value, String> {
    match v {
        Value::Int(_) => Ok(v.clone()),
        Value::Rational(r) => Ok(Value::Int(r.trunc().to_integer())),
        Value::Real(f) => Ok(Value::Real(f.trunc())),
        _ => Err("truncate: not a number".into()),
    }
}

pub fn round_op(v: &Value) -> Result<Value, String> {
    match v {
        Value::Int(_) => Ok(v.clone()),
        Value::Rational(r) => {
            // R5RS: round half to even
            let fl = r.floor();
            let frac = r.as_ref() - &fl;
            let half = BigRational::new(BigInt::one(), BigInt::from(2));
            let out = if frac < half {
                fl
            } else if frac > half {
                fl + BigRational::one()
            } else {
                let fi = fl.to_integer();
                if fi % 2 == BigInt::zero() {
                    fl
                } else {
                    fl + BigRational::one()
                }
            };
            Ok(Value::Int(out.to_integer()))
        }
        Value::Real(f) => Ok(Value::Real(f.round_ties_even())),
        _ => Err("round: not a number".into()),
    }
}

pub fn rationalize(x: &Value, e: &Value) -> Result<Value, String> {
    // simplest rational within e of x
    if matches!(x, Value::Real(_)) || matches!(e, Value::Real(_)) {
        let xf = to_f64(x)?;
        let ef = to_f64(e)?;
        let lo = BigRational::from_float(xf - ef).ok_or("rationalize: bad range")?;
        let hi = BigRational::from_float(xf + ef).ok_or("rationalize: bad range")?;
        let r = simplest_between(&lo, &hi);
        return Ok(Value::Real(r.to_f64().unwrap_or(f64::NAN)));
    }
    let xe = to_exact(x).ok_or("rationalize: not a number")?;
    let ee = to_exact(e).ok_or("rationalize: not a number")?;
    let lo = &xe - &ee;
    let hi = &xe + &ee;
    Ok(norm(simplest_between(&lo, &hi)))
}

/// Simplest rational in [lo, hi] via continued fractions (Stern-Brocot style).
fn simplest_between(lo: &BigRational, hi: &BigRational) -> BigRational {
    if lo >= hi {
        return lo.clone();
    }
    let flo = lo.floor();
    if flo == *lo {
        return flo;
    }
    let ceil = &flo + BigRational::one();
    if ceil <= *hi {
        return ceil;
    }
    // no integer inside; recurse on reciprocals of fractional parts
    let lo_frac = lo - &flo;
    let hi_frac = hi - &flo;
    let inv = simplest_between(
        &(BigRational::one() / hi_frac),
        &(BigRational::one() / lo_frac),
    );
    flo + BigRational::one() / inv
}

pub fn abs_op(v: &Value) -> Result<Value, String> {
    match v {
        Value::Int(i) => Ok(Value::Int(i.abs())),
        Value::Rational(r) => Ok(norm(r.abs())),
        Value::Real(f) => Ok(Value::Real(f.abs())),
        _ => Err("abs: not a number".into()),
    }
}

pub fn max_min(is_max: bool, args: &[Value]) -> Result<Value, String> {
    if args.is_empty() {
        return Err("max/min: needs arguments".into());
    }
    let mut best = args[0].clone();
    want_num("max/min", &best)?;
    let mut inexact = matches!(best, Value::Real(_));
    for a in &args[1..] {
        want_num("max/min", a)?;
        if matches!(a, Value::Real(_)) {
            inexact = true;
        }
        let ord = if inexact || matches!(a, Value::Real(_)) || matches!(best, Value::Real(_)) {
            to_f64_lossy(a)
                .partial_cmp(&to_f64_lossy(&best))
                .unwrap_or(std::cmp::Ordering::Equal)
        } else {
            to_exact(a).unwrap().cmp(&to_exact(&best).unwrap())
        };
        let better = if is_max {
            ord == std::cmp::Ordering::Greater
        } else {
            ord == std::cmp::Ordering::Less
        };
        if better {
            best = a.clone();
        }
    }
    if inexact && !matches!(best, Value::Real(_)) {
        Ok(Value::Real(to_f64_lossy(&best)))
    } else {
        Ok(best)
    }
}

pub fn expt(args: &[Value]) -> Result<Value, String> {
    let base = &args[0];
    let exp = &args[1];
    want_num("expt", base)?;
    want_num("expt", exp)?;
    // exact integer exponent
    if let Value::Int(e) = exp {
        if let Some(b) = to_exact(base) {
            if e.is_zero() {
                return Ok(Value::Int(BigInt::one()));
            }
            let (sign, mag) = e.clone().into_parts();
            let neg = sign == num_bigint::Sign::Minus;
            let n = mag.to_u32().unwrap_or(u32::MAX);
            let r = b.pow(n as i32);
            return Ok(norm(if neg { BigRational::one() / r } else { r }));
        }
    }
    let b = to_f64(base)?;
    let e = to_f64(exp)?;
    Ok(Value::Real(b.powf(e)))
}

pub fn sqrt_op(v: &Value) -> Result<Value, String> {
    want_num("sqrt", v)?;
    if let Some(r) = to_exact(v) {
        if !r.is_negative() {
            // perfect square?
            let n = r.numer().clone();
            let d = r.denom().clone();
            if let (Some(sn), Some(sd)) = (bigint_sqrt(&n), bigint_sqrt(&d)) {
                return Ok(norm(BigRational::new(sn, sd)));
            }
        }
    }
    let f = to_f64(v)?;
    if f < 0.0 {
        return Err("sqrt: negative number (complex numbers unsupported)".into());
    }
    Ok(Value::Real(f.sqrt()))
}

fn bigint_sqrt(n: &BigInt) -> Option<BigInt> {
    if n.is_negative() {
        return None;
    }
    if n.is_zero() {
        return Some(BigInt::zero());
    }
    // Newton's method
    let mut x: BigInt = BigInt::one() << ((n.bits() as usize + 2) / 2);
    loop {
        let y = (&x + n / &x) >> 1;
        if y >= x {
            if &x * &x == *n {
                return Some(x);
            }
            return None;
        }
        x = y;
    }
}

pub fn numerator(v: &Value) -> Result<Value, String> {
    match v {
        Value::Int(i) => Ok(Value::Int(i.clone())),
        Value::Rational(r) => Ok(Value::Int(r.numer().clone())),
        Value::Real(f) => {
            let r = BigRational::from_float(*f).ok_or("numerator: not finite")?;
            Ok(Value::Real(r.numer().to_f64().unwrap_or(f64::INFINITY)))
        }
        _ => Err("numerator: not a number".into()),
    }
}

pub fn denominator(v: &Value) -> Result<Value, String> {
    match v {
        Value::Int(_) => Ok(Value::Int(BigInt::one())),
        Value::Rational(r) => Ok(Value::Int(r.denom().clone())),
        Value::Real(f) => {
            let r = BigRational::from_float(*f).ok_or("denominator: not finite")?;
            Ok(Value::Real(r.denom().to_f64().unwrap_or(f64::INFINITY)))
        }
        _ => Err("denominator: not a number".into()),
    }
}

pub fn exact_to_inexact(v: &Value) -> Result<Value, String> {
    match v {
        Value::Int(_) | Value::Rational(_) => Ok(Value::Real(to_f64(v)?)),
        Value::Real(_) => Ok(v.clone()),
        _ => Err("exact->inexact: not a number".into()),
    }
}

pub fn inexact_to_exact(v: &Value) -> Result<Value, String> {
    match v {
        Value::Real(f) => {
            if !f.is_finite() {
                return Err("inexact->exact: not finite".into());
            }
            Ok(norm(BigRational::from_float(*f).unwrap()))
        }
        Value::Int(_) | Value::Rational(_) => Ok(v.clone()),
        _ => Err("inexact->exact: not a number".into()),
    }
}

// ---------------------------------------------------------------------------
// Radix conversions

pub fn number_to_string(v: &Value, radix: u32) -> Result<String, String> {
    match v {
        Value::Int(i) => Ok(i.to_str_radix(radix)),
        Value::Rational(r) => {
            if radix == 10 {
                Ok(format!("{}/{}", r.numer(), r.denom()))
            } else {
                Ok(format!(
                    "{}/{}",
                    r.numer().to_str_radix(radix),
                    r.denom().to_str_radix(radix)
                ))
            }
        }
        Value::Real(f) => {
            if radix == 10 {
                Ok(crate::printer::fmt_real(*f))
            } else {
                Err("number->string: inexact numbers need radix 10".into())
            }
        }
        _ => Err("number->string: not a number".into()),
    }
}

/// Parse a number token per R5RS lexical syntax (used by reader and
/// string->number). Returns None if the token is not a number.
pub fn parse_number(s: &str) -> Option<Value> {
    parse_number_radix(s, 10)
}

pub fn parse_number_radix(s: &str, default_radix: u32) -> Option<Value> {
    let mut radix = default_radix;
    let mut exactness: Option<bool> = None; // true = exact
    let mut rest = s;
    // up to two prefixes
    for _ in 0..2 {
        if rest.len() >= 2 && rest.starts_with('#') {
            let c = rest.chars().nth(1)?.to_ascii_lowercase();
            match c {
                'b' => radix = 2,
                'o' => radix = 8,
                'd' => radix = 10,
                'x' => radix = 16,
                'e' => exactness = Some(true),
                'i' => exactness = Some(false),
                _ => break,
            }
            rest = &rest[2..];
        } else {
            break;
        }
    }
    match rest {
        "+inf.0" => return Some(Value::Real(f64::INFINITY)),
        "-inf.0" => return Some(Value::Real(f64::NEG_INFINITY)),
        "+nan.0" | "-nan.0" => return Some(Value::Real(f64::NAN)),
        _ => {}
    }
    let body = parse_real_body(rest, radix)?;
    Some(match exactness {
        Some(true) => inexact_to_exact(&body).ok()?,
        Some(false) => exact_to_inexact(&body).ok()?,
        None => body,
    })
}

/// Parse "123", "-3/4", "1.5e3" etc. in the given radix.
fn parse_real_body(s: &str, radix: u32) -> Option<Value> {
    if s.is_empty() {
        return None;
    }
    // '#' is an unspecified digit; a number containing it is inexact (R5RS 6.2.4)
    if s.contains('#') {
        if !s
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '#' | '.' | '+' | '-' | '/'))
        {
            return None;
        }
        let cleaned: String = s.chars().map(|c| if c == '#' { '0' } else { c }).collect();
        let v = parse_real_body(&cleaned, radix)?;
        return exact_to_inexact(&v).ok();
    }
    // fraction
    if let Some(slash) = s.find('/') {
        let (a, b) = (&s[..slash], &s[slash + 1..]);
        let n = BigInt::parse_bytes(a.as_bytes(), radix)?;
        let d = BigInt::parse_bytes(b.as_bytes(), radix)?;
        if d.is_zero() {
            return None;
        }
        return Some(norm(BigRational::new(n, d)));
    }
    let has_point = s.contains('.');
    let has_exp = radix == 10 && {
        let t = s.trim_start_matches(['+', '-']);
        t.len() > 1 && (t[1..].contains('e') || t[1..].contains('E'))
    };
    if !has_point && !has_exp {
        return BigInt::parse_bytes(s.as_bytes(), radix).map(Value::Int);
    }
    if radix != 10 {
        return None;
    }
    // validate scheme-ish float: digits with optional single dot and exponent
    let f: f64 = s.parse().ok()?;
    // Ensure it looked like a number (e.g. not "abc" parsed via inf)
    Some(Value::Real(f))
}

pub fn is_integer_valued(v: &Value) -> bool {
    match v {
        Value::Int(_) => true,
        Value::Rational(_) => false,
        Value::Real(f) => f.fract() == 0.0 && f.is_finite(),
        _ => false,
    }
}
