//! Jupyter 风格的交互式 REPL。
//!
//! 特性：`In [n]:` / `Out[n]:` 计数提示符、多行编辑（Validator 判定输入
//! 完整性，括号未闭合时回车在同一缓冲内续行，可跨行修改，历史按整个输入
//! 召回）、ANSI 颜色（非 TTY 自动关闭）、语法高亮（注释/字符串/数字/特殊
//! 形式/已绑定符号，默认开启，`--no-highlight` 关闭）、rustyline 提供的
//! Tab 补全（内建过程 + 全局环境中用户 define 的符号 + 特殊形式，动态
//! 读取）、历史记录持久化、Ctrl-C 丢弃当前输入不退出、Ctrl-D / `(exit)`
//! 退出。
//!
//! 文件执行模式不走这里（见 main.rs）。

use std::borrow::Cow;
use std::io::IsTerminal;
use std::path::PathBuf;
use std::rc::Rc;

use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{Context, Editor, Helper};

use crate::env::{is_keyword, lookup_var, Env};
use crate::eval;
use crate::number;
use crate::printer::write_to_string;
use crate::reader::{self, ReadError};
use crate::value::{intern, list_to_vec, sym_str, Sym, Value};

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

/// rustyline Validator 的判定逻辑：只有完整输入才放行；括号未闭合时回车
/// 在同一缓冲内续行（多行编辑，可跨行修改，历史按整个输入召回）；
/// 词法错误提示但不放行，可直接继续编辑修正（Ctrl-C 丢弃）。
fn validate_input(src: &str) -> ValidationResult {
    if src.trim().is_empty() {
        return ValidationResult::Valid(None);
    }
    match reader::read_all_strict(src) {
        Ok(_) => ValidationResult::Valid(None),
        Err(ReadError::Eof) => ValidationResult::Incomplete,
        Err(ReadError::Msg(m)) => ValidationResult::Invalid(Some(format!("read error: {}", m))),
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

/// 识别顶层 `(load "path")` 形式，返回其中的路径字符串。
fn load_path_of(v: &Value) -> Option<String> {
    let (items, tail) = list_to_vec(v)?;
    if items.len() == 2 && tail.is_nil() {
        if let (Value::Symbol(s), Value::Str(path)) = (&items[0], &items[1]) {
            if sym_str(*s) == "load" {
                return Some(path.borrow().clone());
            }
        }
    }
    None
}

/// 识别顶层 `(time expr)` 形式，返回待计时的内部表达式。
fn time_form_of(v: &Value) -> Option<Value> {
    let (items, tail) = list_to_vec(v)?;
    if items.len() == 2 && tail.is_nil() {
        if let Value::Symbol(s) = &items[0] {
            if sym_str(*s) == "time" {
                return Some(items[1].clone());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// (view ...)：REPL 层的代码查看指令（高亮打印，不求值）

/// view 的三种目标：全部定义 / 文件 / 单个定义。
pub enum ViewReq {
    All,
    File(String),
    Name(Sym),
}

/// 识别 `(view)` / `(view "path")` / `(view 'name)`。
fn view_form_of(v: &Value) -> Option<ViewReq> {
    let (items, tail) = list_to_vec(v)?;
    if !tail.is_nil() {
        return None;
    }
    match &items.first() {
        Some(Value::Symbol(s)) if sym_str(*s) == "view" => {}
        _ => return None,
    }
    match items.len() {
        1 => Some(ViewReq::All),
        2 => match &items[1] {
            Value::Str(path) => Some(ViewReq::File(path.borrow().clone())),
            // (view 'name)
            Value::Pair(_) => {
                let (q, qtail) = list_to_vec(&items[1])?;
                if q.len() == 2 && qtail.is_nil() {
                    if let (Value::Symbol(qm), Value::Symbol(name)) = (&q[0], &q[1]) {
                        if sym_str(*qm) == "quote" {
                            return Some(ViewReq::Name(*name));
                        }
                    }
                }
                None
            }
            _ => None,
        },
        _ => None,
    }
}

/// 提取顶层 define/define-syntax 定义的名字。
fn defined_name(f: &Value) -> Option<Sym> {
    let (items, tail) = list_to_vec(f)?;
    if items.len() < 2 || !tail.is_nil() {
        return None;
    }
    match &items[0] {
        Value::Symbol(s) if matches!(sym_str(*s).as_str(), "define" | "define-syntax") => {}
        _ => return None,
    }
    match &items[1] {
        // (define name ...)
        Value::Symbol(s) => Some(*s),
        // (define (name args...) ...)
        Value::Pair(p) => match &p.borrow().0 {
            Value::Symbol(s) => Some(*s),
            _ => None,
        },
        _ => None,
    }
}

/// 高亮打印一段源码；非 TTY 原样输出。
fn print_highlighted(src: &str, env: &Rc<Env>, colors: Colors) {
    for line in src.lines() {
        if colors.0 {
            println!("{}", highlight_line(line, env));
        } else {
            println!("{}", line);
        }
    }
}

/// 执行 (view ...)。defs 是本会话求值成功的顶层定义（名字, pretty-print 文本）。
fn handle_view(req: ViewReq, defs: &[(Sym, String)], env: &Rc<Env>, colors: Colors) {
    match req {
        ViewReq::File(path) => match std::fs::read_to_string(&path) {
            Ok(content) => print_highlighted(content.trim_end(), env, colors),
            Err(e) => println!("{}", colors.error(&format!("Error: view: {}: {}", path, e))),
        },
        ViewReq::All => {
            if defs.is_empty() {
                println!("; no definitions yet");
            }
            for (_, src) in defs {
                print_highlighted(src, env, colors);
            }
        }
        ViewReq::Name(name) => {
            let mut found = false;
            for (n2, src) in defs {
                if *n2 == name {
                    print_highlighted(src, env, colors);
                    found = true;
                }
            }
            if !found {
                println!("; no definition for {}", sym_str(name));
            }
        }
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
// 语法高亮（仅交互模式；可用 --no-highlight 关闭）
//
// 逐字符扫描输入行并插入 ANSI 颜色。词法刻意与 reader 解耦：高亮只是
// 视觉提示，即使个别 token 判错也不影响求值。

const HL_RESET: &str = "\x1b[0m";

fn hl_paint(out: &mut String, code: &str, s: &str) {
    out.push_str("\x1b[");
    out.push_str(code);
    out.push('m');
    out.push_str(s);
    out.push_str(HL_RESET);
}

/// 高亮一行源码。配色：注释/括号 灰、字符串 绿、数字/布尔/字符 黄、
/// 特殊形式 品红、已绑定符号 青、quote 系列 品红。
fn highlight_line(line: &str, env: &Rc<Env>) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ';' => {
                hl_paint(&mut out, "90", &chars[i..].iter().collect::<String>());
                break;
            }
            '"' => {
                let start = i;
                i += 1;
                while i < chars.len() && chars[i] != '"' {
                    if chars[i] == '\\' {
                        i += 1; // 跳过转义字符
                    }
                    i += 1;
                }
                if i < chars.len() {
                    i += 1; // 收尾引号
                }
                hl_paint(&mut out, "32", &chars[start..i].iter().collect::<String>());
            }
            '(' | ')' => {
                hl_paint(&mut out, "90", &c.to_string());
                i += 1;
            }
            '\'' | '`' | ',' => {
                hl_paint(&mut out, "35", &c.to_string());
                i += 1;
            }
            c if c.is_whitespace() => {
                out.push(c);
                i += 1;
            }
            _ => {
                let start = i;
                while i < chars.len()
                    && !chars[i].is_whitespace()
                    && !matches!(chars[i], '(' | ')' | ';' | '"' | '\'' | '`' | ',')
                {
                    i += 1;
                }
                let tok: String = chars[start..i].iter().collect();
                highlight_token(&mut out, &tok, env);
            }
        }
    }
    out
}

fn highlight_token(out: &mut String, tok: &str, env: &Rc<Env>) {
    if number::parse_number(tok).is_some() || tok.starts_with("#t") || tok.starts_with("#f") {
        hl_paint(out, "33", tok); // 数字、布尔
        return;
    }
    if tok.starts_with("#\\") {
        hl_paint(out, "33", tok); // 字符字面量
        return;
    }
    // reader 会把标识符折叠为小写，查环境前保持一致
    let sym = intern(&tok.to_lowercase());
    if is_keyword(sym) || env.macros.borrow().contains_key(&sym) {
        hl_paint(out, "35", tok); // 特殊形式/宏
    } else if lookup_var(env, sym).is_some() {
        hl_paint(out, "36", tok); // 已绑定符号（内建过程/用户定义）
    } else {
        out.push_str(tok);
    }
}

// ---------------------------------------------------------------------------
// rustyline 补全器

struct SchemeHelper {
    env: Rc<Env>,
    highlight: bool,
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
impl Highlighter for SchemeHelper {
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        if self.highlight {
            Cow::Owned(highlight_line(line, &self.env))
        } else {
            Cow::Borrowed(line)
        }
    }
    fn highlight_char(&self, _line: &str, _pos: usize, _forced: bool) -> bool {
        self.highlight
    }
}
impl Validator for SchemeHelper {
    fn validate(&self, ctx: &mut ValidationContext) -> rustyline::Result<ValidationResult> {
        Ok(validate_input(ctx.input()))
    }
}
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

/// 兜底：解释器内部的任何 panic 都降级为普通错误，REPL 不退出。
/// panic hook（见 run）会在 stderr 留下位置信息，便于报告 bug。
fn catch_internal<T>(f: impl FnOnce() -> T) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).map_err(|p| {
        let msg = if let Some(s) = p.downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = p.downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic".into()
        };
        format!("internal error (this is a bug, please report): {}", msg)
    })
}

/// 求值一个完整输入里的全部 datum；返回最后一个非 unspecified 的结果。
/// 出错时打印（红色）并中止本单元剩余 datum，与 Jupyter 单元行为一致。
/// defs 累积本会话求值成功的顶层 define/define-syntax，供 (view) 使用。
fn eval_forms(
    forms: &[Value],
    env: &Rc<Env>,
    colors: Colors,
    n: usize,
    defs: &mut Vec<(Sym, String)>,
) {
    if forms.iter().any(is_exit_form) {
        println!("bye~");
        std::process::exit(0);
    }
    let mut last: Option<Value> = None;
    for f in forms {
        // REPL 层的 (view ...)：高亮查看代码，不求值
        if let Some(req) = view_form_of(f) {
            handle_view(req, defs, env, colors);
            continue;
        }
        // REPL 层的 (time expr)：只包裹顶层表达式计时，不是求值器的特殊形式
        let (target, timing) = match time_form_of(f) {
            Some(inner) => (inner, true),
            None => (f.clone(), false),
        };
        let t0 = std::time::Instant::now();
        match catch_internal(|| eval::eval_program(vec![target], env)) {
            Ok(Ok(v)) => {
                if timing {
                    println!("; time: {:.3} ms", t0.elapsed().as_secs_f64() * 1000.0);
                }
                // 顶层 load 成功时打印确认；脚本内嵌套 load 不受影响
                if let Some(path) = load_path_of(f) {
                    println!("; loaded {}", path);
                }
                // 记录顶层定义（重定义同名则更新），供 (view) 查看
                if let Some(name) = defined_name(f) {
                    let src = crate::printer::pretty_to_string(f);
                    defs.retain(|(n2, _)| *n2 != name);
                    defs.push((name, src));
                }
                if !matches!(v, Value::Unspecified) {
                    last = Some(v);
                }
            }
            Ok(Err(e)) | Err(e) => {
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

pub fn run(env: &Rc<Env>, highlight: bool) {
    // panic 会被 catch_internal 降级为普通错误；hook 只留一行位置信息，
    // 替换默认的 "thread 'main' panicked" 长输出（看起来像崩溃）
    std::panic::set_hook(Box::new(|info| {
        if let Some(loc) = info.location() {
            eprintln!(
                "note: internal panic at {} (this is a bug, please report)",
                loc
            );
        }
    }));
    if std::io::stdin().is_terminal() {
        run_interactive(env, highlight);
    } else {
        // 管道/重定向：无颜色、无行编辑，逐行读取
        run_plain(env);
    }
}

fn run_interactive(env: &Rc<Env>, highlight: bool) {
    let colors = Colors(true);
    print_banner();
    let helper = SchemeHelper {
        env: env.clone(),
        highlight,
    };
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
    let mut defs: Vec<(Sym, String)> = Vec::new(); // 本会话的定义，供 (view)
                                                   // Validator 保证 readline 返回时输入已是完整 datum（括号未闭合时回车
                                                   // 在同一缓冲内续行，多行整体编辑、整体进历史）
    loop {
        let prompt = colors.in_prompt(n);
        match rl.readline(&prompt) {
            Ok(line) => {
                if line.trim().is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(line.as_str());
                let status = match catch_internal(|| check_input(&line)) {
                    Ok(s) => s,
                    Err(e) => InputStatus::Error(e),
                };
                match status {
                    InputStatus::Complete(forms) => {
                        eval_forms(&forms, env, colors, n, &mut defs);
                        n += 1;
                    }
                    // Validator 已拦截这两类，理论上到不了；兜底报告即可
                    InputStatus::Incomplete => {
                        println!("{}", colors.error("Read error: unexpected eof"));
                    }
                    InputStatus::Error(m) => {
                        println!("{}", colors.error(&format!("Read error: {}", m)));
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl-C：丢弃当前输入，不退出
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
    let mut defs: Vec<(Sym, String)> = Vec::new(); // 本会话的定义，供 (view)
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
                let status = match catch_internal(|| check_input(&buffer)) {
                    Ok(s) => s,
                    Err(e) => InputStatus::Error(e),
                };
                match status {
                    InputStatus::Incomplete => {}
                    InputStatus::Error(m) => {
                        println!("{}", colors.error(&format!("Read error: {}", m)));
                        buffer.clear();
                    }
                    InputStatus::Complete(forms) => {
                        buffer.clear();
                        eval_forms(&forms, env, colors, n, &mut defs);
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

    fn strip_ansi(s: &str) -> String {
        let mut out = String::new();
        let mut it = s.chars();
        while let Some(c) = it.next() {
            if c == '\x1b' {
                for c2 in it.by_ref() {
                    if c2 == 'm' {
                        break;
                    }
                }
            } else {
                out.push(c);
            }
        }
        out
    }

    #[test]
    fn highlight_line_colors() {
        let env = standard_env();
        crate::eval::eval_str("(define my-var 1)", &env).unwrap();
        let s = highlight_line("(define my-var \"s\" 42 #t #\\a) ; tail", &env);
        assert!(s.contains("\x1b[90m(\x1b[0m")); // 括号灰
        assert!(s.contains("\x1b[35mdefine\x1b[0m")); // 特殊形式品红
        assert!(s.contains("\x1b[36mmy-var\x1b[0m")); // 已绑定符号青
        assert!(s.contains("\x1b[32m\"s\"\x1b[0m")); // 字符串绿
        assert!(s.contains("\x1b[33m42\x1b[0m")); // 数字黄
        assert!(s.contains("\x1b[33m#t\x1b[0m")); // 布尔黄
        assert!(s.contains("\x1b[33m#\\a\x1b[0m")); // 字符黄
        assert!(s.contains("\x1b[90m; tail\x1b[0m")); // 注释灰
    }

    #[test]
    fn validator_multiline_editing() {
        // 完整输入放行
        assert!(matches!(
            validate_input("(+ 1 2)"),
            ValidationResult::Valid(None)
        ));
        assert!(matches!(
            validate_input("  "),
            ValidationResult::Valid(None)
        ));
        assert!(matches!(
            validate_input("(define (f x)\n  (f x))"),
            ValidationResult::Valid(None)
        ));
        // 未闭合 → 回车续行（同一缓冲多行编辑）
        assert!(matches!(
            validate_input("(+ 1"),
            ValidationResult::Incomplete
        ));
        assert!(matches!(
            validate_input("\"abc"),
            ValidationResult::Incomplete
        ));
        // 词法错误 → 提示并可继续编辑修正
        assert!(matches!(
            validate_input("(a . )"),
            ValidationResult::Invalid(_)
        ));
    }

    #[test]
    fn view_and_define_parsing() {
        let read = |s: &str| match check_input(s) {
            InputStatus::Complete(f) => f,
            _ => panic!("read failed: {}", s),
        };
        // view_form_of
        assert!(matches!(
            view_form_of(&read("(view)")[0]),
            Some(ViewReq::All)
        ));
        assert!(
            matches!(view_form_of(&read("(view \"a.scm\")")[0]), Some(ViewReq::File(p)) if p == "a.scm")
        );
        assert!(
            matches!(view_form_of(&read("(view 'fact)")[0]), Some(ViewReq::Name(n)) if sym_str(n) == "fact")
        );
        assert!(view_form_of(&read("(view 1 2)")[0]).is_none());
        assert!(view_form_of(&read("(vi-ew)")[0]).is_none());
        // defined_name
        assert!(matches!(defined_name(&read("(define x 1)")[0]), Some(n) if sym_str(n) == "x"));
        assert!(matches!(defined_name(&read("(define (f a) a)")[0]), Some(n) if sym_str(n) == "f"));
        assert!(
            matches!(defined_name(&read("(define-syntax m (syntax-rules () ((_ x) x)))")[0]), Some(n) if sym_str(n) == "m")
        );
        assert!(defined_name(&read("(+ 1 2)")[0]).is_none());
        assert!(defined_name(&read("(define)")[0]).is_none());
    }

    #[test]
    fn highlight_line_is_lossless() {
        // 去掉 ANSI 序列后必须还原原文（高亮不破坏内容）
        let env = standard_env();
        for line in [
            "(+ 1 2) (car '(a . b))",
            "(define (f x) ; mid\n  (f (- x 1)))",
            "\"unclosed string",
            "（全角 eqv? 1.0)",
            "",
        ] {
            assert_eq!(strip_ansi(&highlight_line(line, &env)), line);
        }
    }
}
