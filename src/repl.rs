//! Jupyter 风格的交互式 REPL。
//!
//! 特性：`In [n]:` / `Out[n]:` 计数提示符、括号未配平时的续行模式、
//! ANSI 颜色（非 TTY 自动关闭）、rustyline 提供的 Tab 补全（内建过程 +
//! 全局环境中用户 define 的符号 + 特殊形式，动态读取）、历史记录持久化、
//! Ctrl-C 丢弃当前输入不退出、Ctrl-D / `(exit)` 退出。
//!
//! 文件执行模式不走这里（见 main.rs）。

use std::io::IsTerminal;
use std::path::PathBuf;
use std::rc::Rc;

use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Editor, Helper};

use crate::env::{is_keyword, Env};
use crate::eval;
use crate::printer::write_to_string;
use crate::reader::{self, ReadError};
use crate::value::{sym_str, Sym, Value};

// ---------------------------------------------------------------------------
// 纯逻辑部分（可单元测试）

/// 一段输入文本的状态：能否直接求值，还是需要续行。
pub enum InputStatus {
    /// 可以求值（含解析出的全部 datum）。
    Complete(Vec<Value>),
    /// datum 未写完（括号/字符串未闭合），需要续行。
    Incomplete,
    /// 词法错误，应当报告并丢弃。
    Error(String),
}

pub fn check_input(src: &str) -> InputStatus {
    match reader::read_all_strict(src) {
        Ok(forms) => InputStatus::Complete(forms),
        Err(ReadError::Eof) => InputStatus::Incomplete,
        Err(ReadError::Msg(m)) => InputStatus::Error(m),
    }
}

/// `(exit)` 是 REPL 内建的退出指令（不经过求值器）。
fn is_exit_form(v: &Value) -> bool {
    if let Value::Pair(p) = v {
        let b = p.borrow();
        matches!(&b.0, Value::Symbol(s) if sym_str(*s) == "exit") && b.1.is_nil()
    } else {
        false
    }
}

/// 补全词表：全局环境里的变量与宏（过滤掉宏展开产生的重命名符号，
/// 它们的名字带空格、用户敲不出来），加上内建特殊形式关键字。
pub fn completion_words(env: &Rc<Env>) -> Vec<String> {
    // 走到根帧（全局环境），REPL 的 define 都落在那里。
    let mut root = env.clone();
    while let Some(p) = root.parent.clone() {
        root = p;
    }
    let mut words: Vec<String> = Vec::new();
    let mut collect = |s: Sym| {
        let name = sym_str(s);
        if !name.contains(' ') && !words.contains(&name) {
            words.push(name);
        }
    };
    for s in root.vars.borrow().keys() {
        collect(*s);
    }
    for s in root.macros.borrow().keys() {
        collect(*s);
    }
    // 常用特殊形式（is_keyword 覆盖全部内建关键字）
    for kw in [
        "quote",
        "quasiquote",
        "unquote",
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
        "define-syntax",
        "let-syntax",
        "letrec-syntax",
        "else",
        "=>",
    ] {
        let s = crate::value::intern(kw);
        if is_keyword(s) || true {
            collect(s);
        }
    }
    words.sort();
    words
}

/// 找到一行中光标前的"当前词元"起点（补全替换区间用）。
pub fn token_start(line: &str, pos: usize) -> usize {
    line[..pos]
        .rfind(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '\'' | '`' | ',' | '"'))
        .map(|i| i + 1)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// 颜色（非 TTY 自动关闭）

#[derive(Clone, Copy)]
struct Colors(bool);

impl Colors {
    fn paint(&self, code: &str, s: &str) -> String {
        if self.0 {
            format!("\x1b[{}m{}\x1b[0m", code, s)
        } else {
            s.to_string()
        }
    }
    fn in_prompt(&self, n: usize) -> String {
        self.paint("1;32", &format!("In [{}]:", n)) + " "
    }
    fn cont_prompt(&self, n: usize) -> String {
        // 与 "In [n]: " 等宽右对齐
        let width = format!("In [{}]: ", n).len();
        self.paint("32", &format!("{:>width$}", "....:", width = width - 1)) + " "
    }
    fn out_tag(&self, n: usize) -> String {
        self.paint("1;36", &format!("Out[{}]:", n)) + " "
    }
    fn error(&self, msg: &str) -> String {
        self.paint("1;31", msg)
    }
    fn result(&self, v: &Value) -> String {
        let s = write_to_string(v);
        match v {
            // 字符串/字符用黄色，与其它 datum 区分
            Value::Str(_) | Value::Char(_) => self.paint("33", &s),
            _ => s,
        }
    }
}

// ---------------------------------------------------------------------------
// rustyline 补全器

struct SchemeHelper {
    env: Rc<Env>,
}

impl Completer for SchemeHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let start = token_start(line, pos);
        let prefix = &line[start..pos];
        let mut out = Vec::new();
        if !prefix.is_empty() {
            for w in completion_words(&self.env) {
                if w.starts_with(prefix) && w != prefix {
                    out.push(Pair {
                        display: w.clone(),
                        replacement: w,
                    });
                }
            }
        }
        Ok((start, out))
    }
}

impl Hinter for SchemeHelper {
    type Hint = String;
}
impl Highlighter for SchemeHelper {}
impl Validator for SchemeHelper {}
impl Helper for SchemeHelper {}

fn history_file() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME") {
        if !xdg.is_empty() {
            return Some(PathBuf::from(xdg).join("scheme-rs").join("history"));
        }
    }
    std::env::var("HOME")
        .ok()
        .map(|h| PathBuf::from(h).join(".scheme-rs_history"))
}

// ---------------------------------------------------------------------------
// 求值与打印

/// 求值一个完整输入里的全部 datum；返回最后一个非 unspecified 的结果。
/// 出错时打印（红色）并中止本单元剩余 datum，与 Jupyter 单元行为一致。
fn eval_forms(forms: &[Value], env: &Rc<Env>, colors: Colors, n: usize) {
    if forms.iter().any(is_exit_form) {
        println!("bye~");
        std::process::exit(0);
    }
    let mut last: Option<Value> = None;
    for f in forms {
        match eval::eval_program(vec![f.clone()], env) {
            Ok(v) => {
                if !matches!(v, Value::Unspecified) {
                    last = Some(v);
                }
            }
            Err(e) => {
                println!("{}", colors.error(&format!("Error: {}", e)));
                return;
            }
        }
    }
    if let Some(v) = last {
        println!("{}{}", colors.out_tag(n), colors.result(&v));
    }
}

fn print_banner() {
    println!(
        "scheme-rs {} — R5RS Scheme interpreter",
        env!("CARGO_PKG_VERSION")
    );
    println!("exit: (exit) or Ctrl-D; Ctrl-C clears the current input");
}

// ---------------------------------------------------------------------------
// 主入口

pub fn run(env: &Rc<Env>) {
    if std::io::stdin().is_terminal() {
        run_interactive(env);
    } else {
        // 管道/重定向：无颜色、无行编辑，逐行读取
        run_plain(env);
    }
}

fn run_interactive(env: &Rc<Env>) {
    let colors = Colors(true);
    print_banner();
    let helper = SchemeHelper { env: env.clone() };
    let mut rl = match Editor::new() {
        Ok(mut e) => {
            e.set_helper(Some(helper));
            e
        }
        Err(_) => {
            run_plain(env);
            return;
        }
    };
    let hist = history_file();
    if let Some(p) = &hist {
        let _ = rl.load_history(p);
    }

    let mut n = 1usize;
    let mut buffer = String::new();
    let mut cont = false; // 是否处于续行模式
    loop {
        let prompt = if cont {
            colors.cont_prompt(n)
        } else {
            colors.in_prompt(n)
        };
        match rl.readline(&prompt) {
            Ok(line) => {
                if line.trim().is_empty() && !cont {
                    continue;
                }
                let _ = rl.add_history_entry(line.as_str());
                buffer.push_str(&line);
                buffer.push('\n');
                match check_input(&buffer) {
                    InputStatus::Incomplete => cont = true,
                    InputStatus::Error(m) => {
                        println!("{}", colors.error(&format!("Read error: {}", m)));
                        buffer.clear();
                        cont = false;
                    }
                    InputStatus::Complete(forms) => {
                        buffer.clear();
                        cont = false;
                        eval_forms(&forms, env, colors, n);
                        n += 1;
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl-C：丢弃当前输入（含续行缓冲），不退出
                buffer.clear();
                cont = false;
                println!("^C");
                continue;
            }
            Err(ReadlineError::Eof) => break,
            Err(_) => break,
        }
    }
    if let Some(p) = &hist {
        if let Some(parent) = p.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = rl.save_history(p);
    }
}

/// 非 TTY 退化模式：保留 Jupyter 风格提示与续行判断，无颜色无行编辑。
fn run_plain(env: &Rc<Env>) {
    use std::io::Write as _;
    let colors = Colors(false);
    let stdin = std::io::stdin();
    let mut n = 1usize;
    let mut buffer = String::new();
    loop {
        if buffer.is_empty() {
            print!("{}", colors.in_prompt(n));
        } else {
            print!("{}", colors.cont_prompt(n));
        }
        let _ = std::io::stdout().flush();
        let mut line = String::new();
        match stdin.read_line(&mut line) {
            Ok(0) => {
                println!();
                return;
            }
            Ok(_) => {
                buffer.push_str(&line);
                match check_input(&buffer) {
                    InputStatus::Incomplete => {}
                    InputStatus::Error(m) => {
                        println!("{}", colors.error(&format!("Read error: {}", m)));
                        buffer.clear();
                    }
                    InputStatus::Complete(forms) => {
                        buffer.clear();
                        eval_forms(&forms, env, colors, n);
                        n += 1;
                    }
                }
            }
            Err(_) => return,
        }
    }
}

// ---------------------------------------------------------------------------
// 测试

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builtins::standard_env;

    fn complete(src: &str) -> bool {
        matches!(check_input(src), InputStatus::Complete(_))
    }

    #[test]
    fn input_completeness() {
        assert!(complete("(+ 1 2)"));
        assert!(complete("(+ 1 2) (display 3)"));
        assert!(complete("; just a comment\n"));
        assert!(complete(""));
        assert!(matches!(check_input("(+ 1"), InputStatus::Incomplete));
        assert!(matches!(check_input("'"), InputStatus::Incomplete));
        assert!(matches!(check_input("\"abc"), InputStatus::Incomplete));
        assert!(matches!(check_input("#(1 2"), InputStatus::Incomplete));
        assert!(matches!(check_input("(a . )"), InputStatus::Error(_)));
        assert!(matches!(check_input("(a ."), InputStatus::Incomplete));
        // 注释结尾不算不完整
        assert!(complete("(+ 1 2) ; trailing"));
    }

    #[test]
    fn completion_word_list() {
        let env = standard_env();
        let words = completion_words(&env);
        assert!(words.contains(&"car".to_string()));
        assert!(words.contains(&"define".to_string()));
        assert!(words.contains(&"call/cc".to_string()));
        // 用户 define 的符号动态出现
        crate::eval::eval_str("(define my-thing 42)", &env).unwrap();
        let words2 = completion_words(&env);
        assert!(words2.contains(&"my-thing".to_string()));
        // 不含重命名符号（名字带空格）
        assert!(words2.iter().all(|w| !w.contains(' ')));
    }

    #[test]
    fn token_start_positions() {
        assert_eq!(token_start("(car", 4), 1);
        assert_eq!(token_start("(map car", 8), 5);
        assert_eq!(token_start("ca", 2), 0);
        assert_eq!(token_start("(quote ab", 9), 7);
    }
}
