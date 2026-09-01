//! Integration tests.
//!
//! - `suites`: the three R5RS suites in tests/scm/, evaluated **in process**
//!   via the library API (no subprocess). chibi/examples are checked through
//!   their own counter variables; pitfall is checked by capturing the
//!   interpreter's current output port into a string port.
//! - `programs`: real-world programs in tests/scm/programs/, read from disk
//!   (the .scm files are the single source of truth and already contain
//!   their own driver sections). Each program is its own test so
//!   `cargo test` runs them in parallel; output is captured via a string
//!   port and asserted line by line.
//! - `cli`: a single subprocess smoke test covering the CLI path.

use std::rc::Rc;

use scheme_rs::builtins::standard_env;
use scheme_rs::env::Env;
use scheme_rs::eval::eval_str;
use scheme_rs::port::{self, Port};
use scheme_rs::printer::write_to_string;

fn manifest_path(rel: &str) -> String {
    format!("{}/{}", env!("CARGO_MANIFEST_DIR"), rel)
}

/// Evaluate `src` in `env` with the current output port redirected to a
/// string port; returns (result, captured-stdout).
fn eval_captured(env: &Rc<Env>, src: &str) -> (Result<scheme_rs::value::Value, String>, String) {
    let out = Port::open_output_string();
    let saved = port::current_output();
    port::set_current_output(out.clone());
    let r = eval_str(src, env);
    port::set_current_output(saved);
    (r, out.get_output_string().unwrap())
}

fn eval_file_captured(
    env: &Rc<Env>,
    rel: &str,
) -> (Result<scheme_rs::value::Value, String>, String) {
    let src = std::fs::read_to_string(manifest_path(rel))
        .unwrap_or_else(|e| panic!("cannot read {}: {}", rel, e));
    eval_captured(env, &src)
}

mod suites {
    use super::*;

    // The only whitelisted case; see docs/testing.md for the full story
    // (R5RS folds case, chibi expects case-sensitive "Martin" for the very
    // same expression -- unsatisfiable both ways, we follow the report).
    const KNOWN_FAILURES_CHIBI: &[&str] = &["(symbol->string 'martin)"];

    #[test]
    fn r5rs_tests_chibi() {
        let env = standard_env();
        let (r, out) = eval_file_captured(&env, "tests/scm/r5rs-tests.scm");
        r.unwrap();
        let unexpected: Vec<&str> = out
            .lines()
            .filter(|l| l.contains("[FAIL]"))
            .filter(|l| !KNOWN_FAILURES_CHIBI.iter().any(|k| l.contains(k)))
            .collect();
        assert!(
            unexpected.is_empty(),
            "unexpected failures:\n{}",
            unexpected.join("\n")
        );
        // counters: 189 run, 188 passed (the whitelisted case being the only failure)
        let v = eval_str("(list *tests-run* *tests-passed*)", &env).unwrap();
        assert_eq!(write_to_string(&v), "(189 188)");
    }

    #[test]
    fn r5rs_pitfall() {
        let env = standard_env();
        let (r, out) = eval_file_captured(&env, "tests/scm/r5rs_pitfall.scm");
        r.unwrap();
        assert!(
            !out.contains("Failure:"),
            "pitfall failures:\n{}",
            out.lines()
                .filter(|l| l.contains("Failure:"))
                .collect::<Vec<_>>()
                .join("\n")
        );
        assert!(
            out.contains("Passed: 8.3"),
            "pitfall did not run to completion:\n{}",
            out
        );
    }

    #[test]
    fn r5rs_examples() {
        let env = standard_env();
        let (r, _out) = eval_file_captured(&env, "tests/scm/r5rs-examples.scm");
        r.unwrap();
        let v = eval_str("*tests-failed*", &env).unwrap();
        assert_eq!(write_to_string(&v), "0");
        let v = eval_str("*tests-run*", &env).unwrap();
        assert_eq!(write_to_string(&v), "253");
    }
}

mod programs {
    use super::*;

    /// Run one program file (with its own driver section) and assert the
    /// captured output lines.
    fn check_program(rel: &str, expected_lines: &[&str]) {
        let env = standard_env();
        let (r, out) = eval_file_captured(&env, rel);
        r.unwrap_or_else(|e| panic!("{}: {}", rel, e));
        let lines: Vec<&str> = out.lines().collect();
        for (i, expected) in expected_lines.iter().enumerate() {
            assert_eq!(
                lines.get(i),
                Some(expected),
                "{}: output line {} mismatch:\nexpected: {}\ngot:\n{}",
                rel,
                i,
                expected,
                out
            );
        }
    }

    #[test]
    fn tak() {
        check_program("tests/scm/programs/gabriel/tak.scm", &["7"]);
    }
    #[test]
    fn cpstak() {
        check_program("tests/scm/programs/gabriel/cpstak.scm", &["7"]);
    }
    #[test]
    fn ack() {
        check_program("tests/scm/programs/gabriel/ack.scm", &["1021"]);
    }
    #[test]
    fn diviter() {
        check_program("tests/scm/programs/gabriel/diviter.scm", &["50000"]);
    }
    #[test]
    fn fibc() {
        check_program("tests/scm/programs/gabriel/fibc.scm", &["6765"]);
    }
    #[test]
    fn deriv() {
        check_program(
            "tests/scm/programs/gabriel/deriv.scm",
            &[
                "(+ (* (* 3 x x) (+ (/ 0 3) (/ 1 x) (/ 1 x))) (* (* a x x) (+ (/ 0 a) (/ 1 x) (/ 1 x))) (* (* b x) (+ (/ 0 b) (/ 1 x))) 0)",
                "done",
            ],
        );
    }
    #[test]
    fn destruc() {
        check_program(
            "tests/scm/programs/gabriel/destruc.scm",
            &["((1 1 2) (1 1 1) (1 1 1 2) (1 1 1 1) (1 1 1 1 2) (1 1 1 1 2) (1 1 1 1 2) (1 1 1 1 2) (1 1 1 1 2) (1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 2 2 2 2 2 3))"],
        );
    }
    #[test]
    fn nqueens() {
        check_program("tests/scm/programs/gabriel/nqueens.scm", &["92"]);
    }
    #[test]
    fn puzzle() {
        check_program("tests/scm/programs/gabriel/puzzle.scm", &["2005"]);
    }
    #[test]
    fn mazefun() {
        check_program(
            "tests/scm/programs/gabriel/mazefun.scm",
            &["((_ * _ _ _ _ _ _ _ _ _) (_ * * * * * * * _ * *) (_ _ _ * _ _ _ * _ _ _) (_ * _ * _ * _ * _ * _) (_ * _ _ _ * _ * _ * _) (* * _ * * * * * _ * _) (_ * _ _ _ _ _ _ _ * _) (_ * _ * _ * * * * * *) (_ _ _ * _ _ _ _ _ _ _) (_ * * * * * * * _ * *) (_ * _ _ _ _ _ _ _ _ _))"],
        );
    }
    #[test]
    fn nboyer() {
        check_program("tests/scm/programs/gabriel/nboyer.scm", &["95024"]);
    }
    #[test]
    fn mceval() {
        check_program(
            "tests/scm/programs/sicp/mceval.scm",
            &["3628800", "144", "42", "(1 4 9 16)", "3", "done"],
        );
    }
    #[test]
    fn amb() {
        check_program(
            "tests/scm/programs/sicp/amb.scm",
            &["(3 20)", "(1 2 3 4)", "(1 6)"],
        );
    }
    #[test]
    fn regmach() {
        check_program("tests/scm/programs/sicp/regmach.scm", &["120", "55"]);
    }
    #[test]
    fn schelog() {
        check_program(
            "tests/scm/programs/logic/schelog.scm",
            &[
                "((who joe))",
                "((who alice))",
                "#f",
                "((x a))",
                "((x b))",
                "((x c))",
                "((xs ()) (ys (1 2 3)))",
                "((xs (1)) (ys (2 3)))",
                "((x 42))",
            ],
        );
    }
}

/// Pure-Scheme stdlib modules (src/libs/*.scm): every tests/scm/libs/*-test.scm
/// is evaluated in a fresh env with the `check` harness prepended; any FAIL
/// line or evaluation error fails the test.
mod libs {
    use super::*;

    const HARNESS: &str = r#"
(define (check e a)
  (if (equal? e a)
      #t
      (begin (display "FAIL: expected ") (write e)
             (display " got ") (write a) (newline))))
"#;

    #[test]
    fn scheme_libs() {
        let dir = manifest_path("tests/scm/libs");
        let mut entries: Vec<_> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("cannot read {}: {}", dir, e))
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("scm"))
            .collect();
        entries.sort();
        assert!(!entries.is_empty(), "no scheme lib tests found in {}", dir);
        for path in entries {
            let body = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e));
            let src = format!("{}\n{}", HARNESS, body);
            let env = standard_env();
            let (r, out) = eval_captured(&env, &src);
            assert!(r.is_ok(), "{}: eval error: {:?}", path.display(), r.err());
            assert!(
                !out.contains("FAIL"),
                "{}: failures:\n{}",
                path.display(),
                out.lines()
                    .filter(|l| l.contains("FAIL"))
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
    }
}

mod cli {
    /// Subprocess smoke test: the CLI file-execution path must keep working.
    #[test]
    fn smoke_run_file() {
        let exe = env!("CARGO_BIN_EXE_scheme-rs");
        let out = std::process::Command::new(exe)
            .arg("tests/scm/programs/gabriel/tak.scm")
            .output()
            .expect("failed to spawn scheme-rs");
        assert!(
            out.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "7");
    }
}
