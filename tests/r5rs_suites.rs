//! Integration tests: run the R5RS suites in tests/scm/ as subprocesses.
//!
//! - tests/scm/r5rs-tests.scm (chibi): no unexpected "[FAIL]" lines.
//! - tests/scm/r5rs_pitfall.scm (SISC): no "Failure:" lines allowed.
//! - tests/scm/r5rs-examples.scm (extracted from the R5RS report):
//!   no "FAIL:" lines allowed.
//!
//! If a specific case ever proves unfixable, whitelist it below (keep it as
//! small as possible).

use std::process::Command;

// Known-failure whitelist.
//
// "(symbol->string 'martin)": unavoidable spec-level conflict. R5RS requires
// case-insensitive identifiers (section 2: "Upper and lower case forms of a
// letter are never distinguished except within character and string
// constants"), and the R5RS report's own example (section 6.3.3) says
// (symbol->string 'Martin) ==> "martin", which the r5rs-examples suite
// checks. The chibi suite assumes case-sensitive symbols and expects
// "Martin" for the very same expression. Both cannot hold at once; we follow
// the R5RS report (the reader folds case; string->symbol still preserves
// case, so pitfall 6.1 passes).
const KNOWN_FAILURES_R5RS_TESTS: &[&str] = &["(symbol->string 'martin)"];
const KNOWN_FAILURES_PITFALL: &[&str] = &[];
const KNOWN_FAILURES_EXAMPLES: &[&str] = &[];

fn run_suite(path: &str) -> (String, String, bool) {
    let exe = env!("CARGO_BIN_EXE_scheme-rs");
    let out = Command::new(exe)
        .arg(path)
        .output()
        .expect("failed to spawn scheme-rs");
    (
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
        out.status.success(),
    )
}

fn check_suite(path: &str, marker: &str, whitelist: &[&str]) {
    let (stdout, stderr, ok) = run_suite(path);
    assert!(ok, "scheme-rs exited with error:\n{}", stderr);
    let failures: Vec<&str> = stdout.lines().filter(|l| l.contains(marker)).collect();
    let unexpected: Vec<&&str> = failures
        .iter()
        .filter(|l| !whitelist.iter().any(|k| l.contains(k)))
        .collect();
    assert!(
        unexpected.is_empty(),
        "unexpected failures in {}:\n{}",
        path,
        unexpected
            .iter()
            .map(|s| (*s).to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        stdout.contains("out of") && stdout.contains("passed"),
        "suite {} did not run to completion:\n{}\n{}",
        path,
        stdout,
        stderr
    );
}

#[test]
fn r5rs_tests_suite() {
    check_suite(
        "tests/scm/r5rs-tests.scm",
        "[FAIL]",
        KNOWN_FAILURES_R5RS_TESTS,
    );
}

#[test]
fn r5rs_pitfall_suite() {
    let (stdout, stderr, ok) = run_suite("tests/scm/r5rs_pitfall.scm");
    assert!(ok, "scheme-rs exited with error:\n{}", stderr);
    let failures: Vec<&str> = stdout.lines().filter(|l| l.contains("Failure:")).collect();
    let unexpected: Vec<&&str> = failures
        .iter()
        .filter(|l| !KNOWN_FAILURES_PITFALL.iter().any(|k| l.contains(k)))
        .collect();
    assert!(
        unexpected.is_empty(),
        "unexpected failures:\n{}",
        unexpected
            .iter()
            .map(|s| (*s).to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        stdout.contains("Passed: 8.3"),
        "pitfall suite did not run to completion:\n{}\n{}",
        stdout,
        stderr
    );
}

#[test]
fn r5rs_examples_suite() {
    check_suite(
        "tests/scm/r5rs-examples.scm",
        "FAIL:",
        KNOWN_FAILURES_EXAMPLES,
    );
}
