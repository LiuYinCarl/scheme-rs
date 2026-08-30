//! Unit tests covering reader/printer, numbers, proper tail calls,
//! continuations, dynamic-wind, and macro hygiene.

use scheme_rs::builtins::standard_env;
use scheme_rs::env::Env;
use scheme_rs::eval::eval_str;
use scheme_rs::printer::write_to_string;
use scheme_rs::value::Value;
use std::rc::Rc;

fn eval_in(env: &Rc<Env>, src: &str) -> Result<Value, String> {
    eval_str(src, env)
}

/// Evaluate one or more programs in a fresh global env; return the printed
/// result of each program's last form.
fn run(srcs: &[&str]) -> Vec<String> {
    let env = standard_env();
    srcs.iter()
        .map(|s| match eval_in(&env, s) {
            Ok(v) => write_to_string(&v),
            Err(e) => format!("ERROR: {}", e),
        })
        .collect()
}

fn one(src: &str) -> String {
    run(&[src]).into_iter().next().unwrap()
}

// ---------------------------------------------------------------------------
// Reader / printer

#[test]
fn reader_printer_roundtrip() {
    assert_eq!(
        one("'(a (b . c) #(1 2) \"s\" #\\a 1/2 -7 3.5)"),
        "(a (b . c) #(1 2) \"s\" #\\a 1/2 -7 3.5)"
    );
    assert_eq!(one("''(quote x)"), "''x");
    assert_eq!(one("'#(1 (2 . 3) ())"), "#(1 (2 . 3) ())");
    assert_eq!(one("#\\space"), "#\\space");
    assert_eq!(one("#t"), "#t");
    assert_eq!(one("(car '(x . y))"), "x");
    assert_eq!(one("(cdr '(x . y))"), "y");
}

#[test]
fn reader_abbreviations_and_comments() {
    assert_eq!(one("; comment\n'(1 ; mid\n 2)"), "(1 2)");
    assert_eq!(one("`(a ,(+ 1 1) ,@(list 3 4))"), "(a 2 3 4)");
    assert_eq!(one("#b101"), "5");
    assert_eq!(one("#x1f"), "31");
    assert_eq!(one("#o17"), "15");
    assert_eq!(one("#e1.5"), "3/2");
    assert_eq!(one("#i1/2"), "0.5");
}

// ---------------------------------------------------------------------------
// Numbers

#[test]
fn numbers_exact() {
    assert_eq!(one("(+ 1/2 1/3)"), "5/6");
    assert_eq!(one("(- 1/2 1)"), "-1/2");
    assert_eq!(one("(* 2 3/4)"), "3/2");
    assert_eq!(one("(/ 3 4)"), "3/4");
    assert_eq!(one("(/ 6 3)"), "2");
    assert_eq!(one("(sqrt 16)"), "4");
    assert_eq!(one("(sqrt 4/9)"), "2/3");
    assert_eq!(one("(sqrt 2)"), "1.4142135623730951");
    assert_eq!(one("(expt 2 10)"), "1024");
    assert_eq!(one("(expt 2 -2)"), "1/4");
    assert_eq!(one("(expt 27 1/3)"), "3.0");
    assert_eq!(one("(gcd 32 -36)"), "4");
    assert_eq!(one("(lcm 32 -36)"), "288");
    assert_eq!(one("(quotient -13 4)"), "-3");
    assert_eq!(one("(modulo -13 4)"), "3");
    assert_eq!(one("(remainder -13 4)"), "-1");
    assert_eq!(one("(modulo 13 -4)"), "-3");
    assert_eq!(one("(floor 7/2)"), "3");
    assert_eq!(one("(ceiling 7/2)"), "4");
    assert_eq!(one("(truncate -7/2)"), "-3");
    assert_eq!(one("(round 7/2)"), "4");
    assert_eq!(one("(round 5/2)"), "2"); // half to even
    assert_eq!(one("(round 3.5)"), "4.0");
    assert_eq!(one("(round 2.5)"), "2.0");
    assert_eq!(one("(numerator 6/4)"), "3");
    assert_eq!(one("(denominator 6/4)"), "2");
    assert_eq!(one("(rationalize 3/10 1/10)"), "1/3");
}

#[test]
fn numbers_exactness_contagion() {
    assert_eq!(one("(+ 1 2.0)"), "3.0");
    assert_eq!(one("(exact->inexact 1/2)"), "0.5");
    assert_eq!(one("(inexact->exact 0.5)"), "1/2");
    assert_eq!(one("(exact? 1/2)"), "#t");
    assert_eq!(one("(inexact? 2.0)"), "#t");
    assert_eq!(one("(max 3 4)"), "4");
    assert_eq!(one("(max 3.9 4)"), "4.0");
    assert_eq!(one("(min 1/2 0.4)"), "0.4");
}

#[test]
fn numbers_radix_strings() {
    assert_eq!(one("(number->string 255 16)"), "\"ff\"");
    assert_eq!(one("(number->string 100)"), "\"100\"");
    assert_eq!(one("(string->number \"100\" 16)"), "256");
    assert_eq!(one("(string->number \"3/4\")"), "3/4");
    assert_eq!(one("(string->number \"1e2\")"), "100.0");
    assert_eq!(one("(string->number \"not-a-number\")"), "#f");
    assert_eq!(one("(= 1/2 0.5)"), "#t");
    assert_eq!(one("(< 1/3 1/2 2/3)"), "#t");
    assert_eq!(one("(integer? 3.0)"), "#t");
    assert_eq!(one("(rational? 3.5)"), "#t");
}

// ---------------------------------------------------------------------------
// Proper tail recursion (must complete in constant stack space)

#[test]
fn proper_tail_recursion() {
    assert_eq!(
        one("(let loop ((n 500000)) (if (= n 0) 'done (loop (- n 1))))"),
        "done"
    );
}

#[test]
fn tail_calls_in_all_positions() {
    // if / cond / and / or / begin / let* in tail position
    assert_eq!(
        one("(define (f n) (cond ((= n 0) 'done) (else (and #t (or #f (f (- n 1))))))) (f 100000)"),
        "done"
    );
    assert_eq!(
        one("(define (g n acc) (let* ((m (- n 1)) (a (+ acc 1))) (if (= n 0) acc (begin (g m a))))) (g 100000 0)"),
        "100000"
    );
}

// ---------------------------------------------------------------------------
// call/cc

#[test]
fn callcc_basic_and_multi_shot() {
    assert_eq!(one("(+ 1 (call/cc (lambda (k) 10)))"), "11");
    assert_eq!(one("(+ 1 (call/cc (lambda (k) (k 10) 20)))"), "11");
    // multi-shot re-entry
    let out = run(&["(define k #f)", "(+ 1 (call/cc (lambda (c) (set! k c) 2)))"]);
    assert_eq!(out[1], "3");
    let env = standard_env();
    eval_in(&env, "(define k #f)").unwrap();
    eval_in(&env, "(+ 1 (call/cc (lambda (c) (set! k c) 2)))").unwrap();
    let v = eval_in(&env, "(k 5)").unwrap();
    assert_eq!(write_to_string(&v), "6");
    let v = eval_in(&env, "(k 100)").unwrap();
    assert_eq!(write_to_string(&v), "101");
}

#[test]
fn callcc_escape_from_recursion() {
    assert_eq!(
        one("(define (product ls) (call/cc (lambda (exit) (let loop ((l ls)) (cond ((null? l) 1) ((= (car l) 0) (exit 0)) (else (* (car l) (loop (cdr l))))))))) (product '(1 2 0 4))"),
        "0"
    );
}

// ---------------------------------------------------------------------------
// dynamic-wind

#[test]
fn dynamic_wind_reentry() {
    let src = "(let ((path '()) (c #f))
      (let ((add (lambda (s) (set! path (cons s path)))))
        (dynamic-wind
            (lambda () (add 'connect))
            (lambda () (add (call/cc (lambda (c0) (set! c c0) 'talk1))))
            (lambda () (add 'disconnect)))
        (if (< (length path) 4) (c 'talk2) (reverse path))))";
    assert_eq!(
        one(src),
        "(connect talk1 disconnect connect talk2 disconnect)"
    );
}

#[test]
fn dynamic_wind_escape() {
    // jumping out of the body must run the after thunk
    assert_eq!(
        one("(let ((path '()))
          (call/cc (lambda (k)
            (dynamic-wind (lambda () (set! path (cons 'in path)))
                          (lambda () (k 'out))
                          (lambda () (set! path (cons 'after path))))))
          (reverse path))"),
        "(in after)"
    );
}

// ---------------------------------------------------------------------------
// syntax-rules and hygiene

#[test]
fn hygiene_free_identifiers() {
    // template's + must refer to the definition-site binding
    assert_eq!(
        one("(let-syntax ((foo (syntax-rules () ((_ expr) (+ expr 1)))))
          (let ((+ *)) (foo 3)))"),
        "4"
    );
}

#[test]
fn hygiene_no_capture() {
    // introduced temporaries must not capture user variables
    assert_eq!(
        one("(define-syntax swap! (syntax-rules () ((swap! a b) (let ((tmp a)) (set! a b) (set! b tmp)))))
          (let ((a 1) (tmp 2)) (swap! a tmp) (list a tmp))"),
        "(2 1)"
    );
}

#[test]
fn syntax_rules_ellipsis() {
    assert_eq!(
        one("(define-syntax my-list (syntax-rules () ((_ x ...) (list x ...)))) (my-list 1 2 3)"),
        "(1 2 3)"
    );
    // nested ellipsis
    assert_eq!(
        one("(define-syntax m (syntax-rules () ((_ (x y ...) ...) (list (list y ...) ...))))\n(m (1 2 3) (4 5))"),
        "((2 3) (5))"
    );
    // (... ...) escape (the R5RS be-like-begin example)
    assert_eq!(
        one("(define-syntax be-like-begin
               (syntax-rules ()
                 ((be-like-begin name)
                  (define-syntax name
                    (syntax-rules () ((name a (... ...)) (begin a (... ...))))))))
            (be-like-begin sequence)
            (sequence 1 2 3)"),
        "3"
    );
}

#[test]
fn syntax_rules_zero_repetition() {
    // R5RS 4.3.2 `when` example: the ellipsis pattern variable matches zero
    // times; it must still be bound (to the empty sequence).
    assert_eq!(
        one("(let-syntax ((when (syntax-rules ()
                                  ((when test stmt1 stmt2 ...)
                                   (if test (begin stmt1 stmt2 ...))))))
              (let ((if #t))
                (when if (set! if 'now))
                if))"),
        "now"
    );
    // zero repetitions with several body forms present too
    assert_eq!(
        one("(let-syntax ((when (syntax-rules ()
                                  ((when test stmt1 stmt2 ...)
                                   (if test (begin stmt1 stmt2 ...))))))
              (when #t 'a 'b 'c))"),
        "c"
    );
    // zero repetitions under a nested ellipsis
    assert_eq!(
        one(
            "(define-syntax m (syntax-rules () ((_ (x ...) ...) (list (list x ...) ...))))
            (m)"
        ),
        "()"
    );
}

#[test]
fn printer_terminates_on_cycles() {
    // a cdr-cycle must print with a cycle marker, not loop forever
    assert_eq!(
        one("(let ((x (list 'a 'b))) (set-cdr! (cdr x) x) x)"),
        "(a b . #<cycle>)"
    );
    // shared (non-cyclic) tails still print normally
    assert_eq!(one("(let ((x (list 1))) (list x x))"), "((1) (1))");
}

#[test]
fn quasiquote_nested() {
    assert_eq!(
        one("`(a `(b ,(+ 1 2) ,(foo ,(+ 1 3) d) e) f)"),
        "(a `(b ,(+ 1 2) ,(foo 4 d) e) f)"
    );
    assert_eq!(
        one("(let ((name1 'x) (name2 'y)) `(a `(b ,,name1 ,',name2 d) e))"),
        "(a `(b ,x ,'y d) e)"
    );
}

// ---------------------------------------------------------------------------
// Equivalence

#[test]
fn equivalence_predicates() {
    assert_eq!(one("(eqv? 2 2.0)"), "#f");
    assert_eq!(one("(eqv? 'a 'a)"), "#t");
    assert_eq!(one("(eq? (list 'a) (list 'a))"), "#f");
    assert_eq!(one("(equal? (make-vector 3 'x) (make-vector 3 'x))"), "#t");
    assert_eq!(one("(equal? \"abc\" \"abc\")"), "#t");
    assert_eq!(one("(eq? #f '())"), "#f");
    // circular structure: equal? must terminate
    assert_eq!(one("(let ((x (list 'a))) (set-cdr! x x) (eq? x x))"), "#t");
}

// ---------------------------------------------------------------------------
// I/O extensions

#[test]
fn string_ports_and_error() {
    assert_eq!(
        one("(call-with-output-string (lambda (p) (write '(1 2) p) (display \"x\" p)))"),
        "\"(1 2)x\""
    );
    assert_eq!(
        one("(let ((p (open-input-string \"(a b) 42\"))) (list (read p) (read p)))"),
        "((a b) 42)"
    );
    let env = standard_env();
    let r = eval_in(&env, "(error 'boom \"bad\" 42)");
    match r {
        Err(e) => assert!(e.contains("boom"), "unexpected error: {}", e),
        Ok(_) => panic!("error did not raise"),
    }
}

#[test]
fn values_and_call_with_values() {
    assert_eq!(one("(call-with-values (lambda () (values 1 2 3)) +)"), "6");
    assert_eq!(
        one("(call-with-values (lambda () 5) (lambda (x) (* x 2)))"),
        "10"
    );
    // R5RS 6.4: escape procedures accept multiple arguments and deliver them
    // as multiple values (the report's own definition of `values`).
    assert_eq!(
        one("(define (values . things)
              (call-with-current-continuation
                (lambda (cont) (apply cont things))))
            (call-with-values (lambda () (values 4 5)) (lambda (a b) b))"),
        "5"
    );
    assert_eq!(one("(call-with-values * -)"), "-1");
    // zero values
    assert_eq!(
        one("(call-with-values (lambda () (values)) (lambda () 'empty))"),
        "empty"
    );
}

// ---------------------------------------------------------------------------
// R5RS 6.2.5: integer division etc. accept inexact integers

#[test]
fn inexact_integer_operations() {
    assert_eq!(one("(remainder -13 -4.0)"), "-1.0");
    assert_eq!(one("(remainder -13.0 -4)"), "-1.0");
    assert_eq!(one("(modulo 13 -4.0)"), "-3.0");
    assert_eq!(one("(quotient -13 4.0)"), "-3.0");
    assert_eq!(one("(gcd 32.0 -36)"), "4.0");
    assert_eq!(one("(lcm 32.0 -36)"), "288.0");
    assert_eq!(one("(gcd 32 -36)"), "4");
    let env = standard_env();
    assert!(eval_in(&env, "(remainder 1.5 2)").is_err());
}

// ---------------------------------------------------------------------------
// R5RS 6.2.4: '#' is an unspecified digit, result inexact

#[test]
fn hash_digit_placeholder() {
    assert_eq!(one("(string->number \"15##\")"), "1500.0");
    assert_eq!(one("15##"), "1500.0");
    assert_eq!(one("#x1##"), "256.0");
    assert_eq!(one("(exact? 15##)"), "#f");
}

// ---------------------------------------------------------------------------
// Transcendental functions (R5RS 6.2.5)

#[test]
fn transcendental_functions() {
    assert_eq!(one("(exp 0)"), "1.0");
    assert_eq!(one("(log 1)"), "0.0");
    assert_eq!(one("(sin 0)"), "0.0");
    assert_eq!(one("(cos 0)"), "1.0");
    assert_eq!(one("(tan 0)"), "0.0");
    assert_eq!(one("(asin 0)"), "0.0");
    assert_eq!(one("(round (* 2 (acos -1)))"), "6.0");
    assert_eq!(one("(atan 0)"), "0.0");
    assert_eq!(one("(> (atan 1 1) 0.78)"), "#t");
}

// ---------------------------------------------------------------------------
// Case-insensitive identifiers (R5RS section 2); string->symbol preserves case

#[test]
fn case_insensitive_symbols() {
    assert_eq!(one("(eq? 'mISSISSIppi 'mississippi)"), "#t");
    assert_eq!(one("(symbol->string 'Martin)"), "\"martin\"");
    assert_eq!(
        one("(symbol->string (string->symbol \"Malvina\"))"),
        "\"Malvina\""
    );
    assert_eq!(
        one("(eq? (string->symbol \"f\") (string->symbol \"F\"))"),
        "#f"
    );
    assert_eq!(one("(eq? (string->symbol \"F\") 'f)"), "#f");
    // keywords are case-insensitive too
    assert_eq!(one("(LAMBDA (x) x)"), "#<procedure>");
    assert_eq!(one("((Lambda (x) (* x x)) 21)"), "441");
}

// ---------------------------------------------------------------------------
// Bug fix regression tests

/// 脚本文件末尾 datum 未闭合必须报错并以非零退出（不允许静默丢弃）。
#[test]
fn script_unclosed_datum_errors() {
    let dir = std::env::temp_dir();
    let bad = dir.join("scheme_rs_bad_unclosed.scm");
    std::fs::write(&bad, "(define x 1\n").unwrap();
    let exe = env!("CARGO_BIN_EXE_scheme-rs");
    let out = std::process::Command::new(exe).arg(&bad).output().unwrap();
    assert!(!out.status.success(), "unclosed script must fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("unexpected end of input"),
        "stderr: {}",
        stderr
    );
    // 对照：完整脚本正常退出
    let good = dir.join("scheme_rs_good.scm");
    std::fs::write(&good, "(define x 1) (display x)\n").unwrap();
    let out2 = std::process::Command::new(exe).arg(&good).output().unwrap();
    assert!(out2.status.success());
}

/// equal? 对环形 pair/vector 必须终止并按同构假设判定；
/// DAG 共享结构不得误判。
#[test]
fn equal_cyclic_structures() {
    // 环形向量（原来会爆栈）
    assert_eq!(
        one("(define v1 (make-vector 1)) (vector-set! v1 0 v1)
             (define v2 (make-vector 1)) (vector-set! v2 0 v2)
             (equal? v1 v2)"),
        "#t"
    );
    // 环形 pair
    assert_eq!(
        one("(let ((x (list 1 2))) (set-cdr! (cdr x) x)
              (let ((y (list 1 2))) (set-cdr! (cdr y) y)
                (equal? x y)))"),
        "#t"
    );
    // 不同构的环
    assert_eq!(
        one("(let ((x (list 1 2))) (set-cdr! (cdr x) x)
              (let ((y (list 1 3))) (set-cdr! (cdr y) y)
                (equal? x y)))"),
        "#f"
    );
    // DAG 共享（非环）：左边两处共享同一对象，右边是两个相等但不同的对象
    assert_eq!(
        one("(let ((s (cons 1 2)))
              (equal? (list s s) (list (cons 1 2) (cons 1 2))))"),
        "#t"
    );
}

/// promise：外层必须缓存（一个 promise 只求值一次）；force 返回值即 proc
/// 的返回值（R5RS make-promise 参考实现语义，不做链式塌缩）；无进展的
/// 自引用 forcing 报错而不是死循环。
#[test]
fn promise_memoization_and_reentry() {
    // 外层 promise 缓存：副作用只跑一次
    assert_eq!(
        one("(define count 0)
             (define p1 (delay (begin (set! count (+ count 1)) (delay 42))))
             (force p1) (force p1)
             count"),
        "1"
    );
    // 参考实现语义：force 不塌缩 promise 链，返回值可以是 promise
    assert_eq!(one("(force (force (delay (delay 7))))"), "7");
    // R5RS 4.2.5 的自引用示例（重入 forcing 但必须能取得进展）仍然成立
    assert_eq!(
        one("(define count 0)
             (define p (delay (begin (set! count (+ count 1))
                                      (if (> count x) count (force p)))))
             (define x 5)
             (list (force p) (begin (set! x 10) (force p)))"),
        "(6 6)"
    );
    // 无进展自引用：报错而非死循环
    let env = standard_env();
    let r = eval_in(&env, "(define p (delay (force p))) (force p)");
    match r {
        Err(e) => assert!(e.contains("re-entrant"), "unexpected error: {}", e),
        Ok(_) => panic!("self-forcing promise did not raise"),
    }
}

/// 模板某一层 ellipsis 的下降只应作用于该模板项实际使用的变量；作用域里
/// 长度不同的其它 Many 绑定必须原样透传（schelog 移植时暴露的误报）。
#[test]
fn syntax_rules_ellipsis_unrelated_lengths() {
    // x 绑 1 个、y 绑 3 个：展开 (y ...) 时不得因 x 长度不足而报错
    assert_eq!(
        one("(define-syntax m
               (syntax-rules ()
                 ((_ (x ...) (y ...)) (list y ... x ...))))
             (m (1) (2 3 4))"),
        "(2 3 4 1)"
    );
    // 嵌套：内层迭代使用的变量与外层不同（schelog %rel 的形态），
    // 同一模板里不同位置的 ellipsis 各自独立迭代
    assert_eq!(
        one("(define-syntax m2
               (syntax-rules ()
                 ((_ (v ...) ((ch ...) sg ...) ...)
                  (list '(v ...) '(ch ...) ...))))
             (m2 (a) ((x 1) (y 2)) ((x 3) (y 4)))"),
        "((a) (x 1) (x 3))"
    );
}
