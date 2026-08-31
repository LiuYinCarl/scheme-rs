//! 命令行入口：`scheme-rs file.scm` 执行文件（出错即非零退出），
//! 无参数进入 REPL（见 repl.rs）。

use std::process::ExitCode;

use scheme_rs::builtins;
use scheme_rs::eval;
use scheme_rs::reader;
use scheme_rs::repl;

fn main() -> ExitCode {
    let mut highlight = true;
    let mut hint = true;
    let mut file: Option<String> = None;
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--no-highlight" => highlight = false,
            "--no-hint" => hint = false,
            "-h" | "--help" => {
                println!("Usage: scheme-rs [--no-highlight] [--no-hint] [file.scm]");
                println!("  --no-highlight  关闭 REPL 语法高亮（默认开启）");
                println!("  --no-hint       关闭 REPL 实时错误提示（默认开启）");
                println!("  file.scm        执行文件；缺省进入 REPL");
                return ExitCode::SUCCESS;
            }
            _ if file.is_none() => file = Some(a),
            other => {
                eprintln!("scheme-rs: unknown argument: {}", other);
                return ExitCode::from(2);
            }
        }
    }
    let env = builtins::standard_env();
    match file {
        Some(f) => run_file(&f, &env),
        None => {
            repl::run(&env, highlight, hint);
            ExitCode::SUCCESS
        }
    }
}

fn run_file(path: &str, env: &std::rc::Rc<scheme_rs::env::Env>) -> ExitCode {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("scheme-rs: cannot read {}: {}", path, e);
            return ExitCode::from(2);
        }
    };
    // 用严格版读取：文件末尾 datum 未闭合必须报错（read_all 会把末尾
    // 未写完的 datum 静默丢弃，导致残缺脚本"成功"退出）。
    let forms = match reader::read_all_strict(&content) {
        Ok(f) => f,
        Err(reader::ReadError::Eof) => {
            eprintln!("scheme-rs: unexpected end of input in {}", path);
            return ExitCode::from(2);
        }
        Err(reader::ReadError::Msg(m)) => {
            eprintln!("scheme-rs: read error in {}: {}", path, m);
            return ExitCode::from(2);
        }
    };
    match eval::eval_program(forms, env) {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}
