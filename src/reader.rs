//! Datum reader: full R5RS lexical syntax.
//!
//! 把字符流读成 Value（datum），对源码和 `read` 过程共用。按 R5RS 第 2
//! 章，标识符不区分大小写，reader 统一折叠为小写（`string->symbol` 不
//! 经过这里，所以仍能构造大写符号）。`ReadError::Eof` 单独成枚举，好让
//! REPL 区分"输入结束"与"datum 未写完，需要续行"。

use crate::number;
use crate::value::{cons, intern, list_from_vec, Value};

#[derive(Debug)]
pub enum ReadError {
    /// Unexpected end of input (more input could complete the datum).
    Eof,
    Msg(String),
}

pub type ReadResult<T> = Result<T, ReadError>;

fn err<T>(m: impl Into<String>) -> ReadResult<T> {
    Err(ReadError::Msg(m.into()))
}

/// A source of characters with one-char lookahead.
pub trait CharSource {
    fn peek_char(&mut self) -> Option<char>;
    fn next_char(&mut self) -> Option<char>;
}

pub struct StrSource {
    chars: Vec<char>,
    pos: usize,
}

impl StrSource {
    pub fn new(s: &str) -> StrSource {
        StrSource {
            chars: s.chars().collect(),
            pos: 0,
        }
    }
}

impl CharSource for StrSource {
    fn peek_char(&mut self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
    fn next_char(&mut self) -> Option<char> {
        let c = self.peek_char();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }
}

fn is_delimiter(c: char) -> bool {
    // R5RS 没有 |...| 符号语法，'|' 只是普通符号字符，不是分隔符
    c.is_whitespace() || matches!(c, '(' | ')' | '"' | ';' | '\'' | '`' | ',')
}

fn skip_ws(src: &mut dyn CharSource) {
    loop {
        match src.peek_char() {
            Some(c) if c.is_whitespace() => {
                src.next_char();
            }
            Some(';') => {
                while let Some(c) = src.next_char() {
                    if c == '\n' {
                        break;
                    }
                }
            }
            _ => break,
        }
    }
}

pub fn read_datum(src: &mut dyn CharSource) -> ReadResult<Value> {
    skip_ws(src);
    let c = match src.next_char() {
        None => return Err(ReadError::Eof),
        Some(c) => c,
    };
    match c {
        '(' => read_list(src),
        ')' => err("unexpected )"),
        '\'' => Ok(list_from_vec(vec![Value::sym("quote"), read_datum(src)?])),
        '`' => Ok(list_from_vec(vec![
            Value::sym("quasiquote"),
            read_datum(src)?,
        ])),
        ',' => {
            let sym = if src.peek_char() == Some('@') {
                src.next_char();
                "unquote-splicing"
            } else {
                "unquote"
            };
            Ok(list_from_vec(vec![Value::sym(sym), read_datum(src)?]))
        }
        '"' => read_string(src),
        '#' => read_hash(src),
        _ => read_atom(src, c),
    }
}

fn read_list(src: &mut dyn CharSource) -> ReadResult<Value> {
    let mut items = Vec::new();
    loop {
        skip_ws(src);
        match src.peek_char() {
            None => return Err(ReadError::Eof),
            Some(')') => {
                src.next_char();
                return Ok(list_from_vec(items));
            }
            Some('.') => {
                // could be a dotted pair or a symbol starting with '.'
                src.next_char();
                match src.peek_char() {
                    Some(c) if is_delimiter(c) => {
                        let tail = read_datum(src)?;
                        skip_ws(src);
                        if src.next_char() != Some(')') {
                            return err("expected ) after dotted pair");
                        }
                        let mut out = tail;
                        for x in items.into_iter().rev() {
                            out = cons(x, out);
                        }
                        return Ok(out);
                    }
                    _ => {
                        items.push(read_atom(src, '.')?);
                    }
                }
            }
            _ => items.push(read_datum(src)?),
        }
    }
}

fn read_string(src: &mut dyn CharSource) -> ReadResult<Value> {
    let mut s = String::new();
    loop {
        match src.next_char() {
            None => return Err(ReadError::Eof),
            Some('"') => {
                return Ok(Value::Str(std::rc::Rc::new(std::cell::RefCell::new(s))));
            }
            Some('\\') => match src.next_char() {
                None => return Err(ReadError::Eof),
                Some('n') => s.push('\n'),
                Some('t') => s.push('\t'),
                Some('r') => s.push('\r'),
                Some('\\') => s.push('\\'),
                Some('"') => s.push('"'),
                Some('0') => s.push('\0'),
                Some(c) => s.push(c),
            },
            Some(c) => s.push(c),
        }
    }
}

fn read_hash(src: &mut dyn CharSource) -> ReadResult<Value> {
    match src.next_char() {
        None => Err(ReadError::Eof),
        Some(c) if c == 't' || c == 'T' => {
            // #t or #true
            let rest = read_token(src, "");
            if rest.is_empty() || rest == "rue" {
                Ok(Value::Bool(true))
            } else {
                err(format!("bad # syntax: #{}{}", c, rest))
            }
        }
        Some(c) if c == 'f' || c == 'F' => {
            let rest = read_token(src, "");
            if rest.is_empty() || rest == "alse" {
                Ok(Value::Bool(false))
            } else {
                err(format!("bad # syntax: #{}{}", c, rest))
            }
        }
        Some('\\') => {
            let first = match src.next_char() {
                None => return Err(ReadError::Eof),
                Some(c) => c,
            };
            let rest = read_token(src, "");
            if rest.is_empty() {
                return Ok(Value::Char(first));
            }
            let name: String = std::iter::once(first).chain(rest.chars()).collect();
            match name.to_ascii_lowercase().as_str() {
                "space" => Ok(Value::Char(' ')),
                "newline" => Ok(Value::Char('\n')),
                "tab" => Ok(Value::Char('\t')),
                "return" => Ok(Value::Char('\r')),
                "null" => Ok(Value::Char('\0')),
                _ => err(format!("unknown character name: #\\{}", name)),
            }
        }
        Some('(') => {
            let lst = read_list(src)?;
            let items = crate::value::proper_list(&lst)
                .ok_or_else(|| ReadError::Msg("bad vector literal".into()))?;
            Ok(Value::Vector(std::rc::Rc::new(std::cell::RefCell::new(
                items,
            ))))
        }
        Some(c) if "bodxei".contains(c.to_ascii_lowercase()) => {
            // number with radix/exactness prefix; token may contain more prefixes
            let tok = read_token(src, &c.to_string());
            let full = format!("#{}", tok);
            match number::parse_number(&full) {
                Some(v) => Ok(v),
                None => err(format!("bad number: {}", full)),
            }
        }
        Some(c) => err(format!("unknown # syntax: #{}", c)),
    }
}

/// Read the remainder of a token (until delimiter/EOF), prefixed by `init`.
fn read_token(src: &mut dyn CharSource, init: &str) -> String {
    let mut tok = init.to_string();
    loop {
        match src.peek_char() {
            Some(c) if !is_delimiter(c) => {
                tok.push(c);
                src.next_char();
            }
            _ => break,
        }
    }
    tok
}

fn read_atom(src: &mut dyn CharSource, first: char) -> ReadResult<Value> {
    let tok = read_token(src, &first.to_string());
    // number?
    if let Some(v) = number::parse_number(&tok) {
        return Ok(v);
    }
    // must look like it *could* be a number to be an error; otherwise symbol
    if looks_like_bad_number(&tok) {
        return err(format!("bad number syntax: {}", tok));
    }
    // R5RS: case is not distinguished in identifiers (but string->symbol
    // preserves case, so folding happens only here in the reader)
    Ok(Value::Symbol(intern(&tok.to_lowercase())))
}

fn looks_like_bad_number(tok: &str) -> bool {
    // e.g. "12abc", "1/0", "3.4.5"
    let t = tok.trim_start_matches(['+', '-']);
    !t.is_empty() && t.chars().next().unwrap().is_ascii_digit()
}

/// Read all datums from a string.
pub fn read_all(s: &str) -> ReadResult<Vec<Value>> {
    let mut src = StrSource::new(s);
    let mut out = Vec::new();
    loop {
        match read_datum(&mut src) {
            Ok(v) => out.push(v),
            Err(ReadError::Eof) => return Ok(out),
            Err(e) => return Err(e),
        }
    }
}

/// Like `read_all`, but an unterminated final datum is an error (Eof),
/// instead of being silently ignored. Used by the REPL to decide whether to
/// ask for a continuation line.
///
/// 与 read_all 的区别：末尾 datum 未写完时返回 Err(Eof)，REPL 据此进入
/// 续行模式；read_all 则会把它静默丢掉。
pub fn read_all_strict(s: &str) -> ReadResult<Vec<Value>> {
    let mut src = StrSource::new(s);
    let mut out = Vec::new();
    loop {
        skip_ws(&mut src);
        if src.peek_char().is_none() {
            return Ok(out);
        }
        out.push(read_datum(&mut src)?);
    }
}
