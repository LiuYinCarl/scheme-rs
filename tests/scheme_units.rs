//! Unit tests covering reader/printer, numbers, proper tail calls,
//! continuations, dynamic-wind, and macro hygiene.

use scheme_rs::builtins::standard_env;
use scheme_rs::env::Env;
use scheme_rs::eval::eval_str;
use scheme_rs::printer::{display_to_string, write_to_string};
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

/// 全角括号等多字节字符开头的 token 按 symbol 处理；
/// 数字解析的路径不得按字节切片导致 panic（历史 bug）。
#[test]
fn reader_multibyte_token_no_panic() {
    assert_eq!(one("'（x）"), "（x）");
}

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

#[test]
fn integer_division_arity() {
    // used to panic (index out of bounds) or silently drop extra args
    assert!(one("(quotient 5)").starts_with("ERROR"));
    assert!(one("(remainder 5)").starts_with("ERROR"));
    assert!(one("(modulo 5)").starts_with("ERROR"));
    assert!(one("(modulo 5 3 2)").starts_with("ERROR"));
    assert!(one("(quotient 5 3 2)").starts_with("ERROR"));
}

#[test]
fn radix_bounds_checked() {
    assert_eq!(one("(number->string 255 36)"), "\"73\"");
    // out-of-range radix used to panic inside num-bigint
    assert!(one("(number->string 255 37)").starts_with("ERROR"));
    assert!(one("(number->string 255 1)").starts_with("ERROR"));
    // huge radix used to truncate via `as u32`
    assert!(one("(number->string 255 4294967297)").starts_with("ERROR"));
    // rational printing path uses the radix too
    assert!(one("(number->string 1/2 37)").starts_with("ERROR"));
    assert!(one("(string->number \"ff\" 37)").starts_with("ERROR"));
    assert!(one("(string->number \"10\" 1)").starts_with("ERROR"));
    // extra args used to be ignored
    assert!(one("(number->string 5 10 99)").starts_with("ERROR"));
    assert!(one("(string->number \"5\" 10 99)").starts_with("ERROR"));
}

#[test]
fn placeholder_digits_require_a_real_digit() {
    // 1## == 100.0 stays valid; bare placeholders are not numbers (R5RS 7.1.1)
    assert_eq!(one("1##"), "100.0");
    assert_eq!(one("(string->number \"1##\")"), "100.0");
    assert!(one("#d#").starts_with("ERROR: bad number"));
    assert!(one("#x#").starts_with("ERROR: bad number"));
    assert_eq!(one("(string->number \"#\")"), "#f");
}

#[test]
fn integer_to_char_range_checked() {
    assert_eq!(one("(integer->char 65)"), "#\\A");
    assert_eq!(one("(char->integer #\\A)"), "65");
    // used to truncate to #\x01 via `as u32`
    assert!(one("(integer->char 4294967297)").starts_with("ERROR"));
    assert!(one("(integer->char -1)").starts_with("ERROR"));
    // UTF-16 surrogate half: not a valid char
    assert!(one("(integer->char 55296)").starts_with("ERROR"));
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
fn member_assoc_detect_cycles() {
    // used to loop forever on circular lists
    assert!(one("(let ((x (list 1 2))) (set-cdr! (cdr x) x) (memq 9 x))").starts_with("ERROR"));
    assert!(one("(let ((x (list 1 2))) (set-cdr! (cdr x) x) (member 9 x))").starts_with("ERROR"));
    assert!(one("(let ((x (list (list 'a 1)))) (set-cdr! x x) (assq 'b x))").starts_with("ERROR"));
    assert!(one("(let ((x (list (list 'a 1)))) (set-cdr! x x) (assoc 'b x))").starts_with("ERROR"));
    // a hit found before completing a lap still works
    assert_eq!(
        one("(let ((x (list 1 2))) (set-cdr! (cdr x) x) (car (memv 2 x)))"),
        "2"
    );
    assert_eq!(
        one("(let ((x (list (list 'a 1)))) (set-cdr! x x) (assv 'a x))"),
        "(a 1)"
    );
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
fn output_procedures_check_arity() {
    // extra args used to be silently ignored
    assert!(one("(write)").starts_with("ERROR"));
    assert!(one("(write 1 2 3)").starts_with("ERROR"));
    assert!(one("(display 1 2 3)").starts_with("ERROR"));
    assert!(one("(write-char #\\a 1 2)").starts_with("ERROR"));
    assert!(one("(pretty-print 'a 'b 'c)").starts_with("ERROR"));
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

#[test]
fn odd_even_accept_inexact_integers() {
    // R5RS 6.2.5: integer-valued inexacts count as integers
    assert_eq!(one("(odd? 3.0)"), "#t");
    assert_eq!(one("(even? -4.0)"), "#t");
    assert_eq!(one("(odd? -3)"), "#t");
    assert!(one("(odd? 3.5)").starts_with("ERROR"));
    assert!(one("(even? 'a)").starts_with("ERROR"));
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

/// equal? 沿 cdr 链迭代而不是 Rust 递归：500k 长度的列表比较不再爆
/// Rust 栈（曾 SIGABRT；测试线程栈比主线程更小，更需要这个保证）。
#[test]
fn equal_long_lists_no_stack_overflow() {
    assert_eq!(
        one(
            "(begin (define (mk n) (if (= n 0) '() (cons n (mk (- n 1)))))
                    (equal? (mk 500000) (mk 500000)))"
        ),
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

// ---------------------------------------------------------------------------
// Extensions（非 R5RS，见 docs/extensions.md）

#[test]
fn ext_runtime_and_clock() {
    assert_eq!(one("(>= (runtime) 0)"), "#t");
    assert_eq!(one("(integer? (current-milliseconds))"), "#t");
}

#[test]
fn ext_random() {
    // random-seed 可复现（RNG 状态是 thread_local，跨 standard_env 保持）
    let a = one("(begin (random-seed 42) (random 1000000))");
    let b = one("(begin (random-seed 42) (random 1000000))");
    assert_eq!(a, b);
    assert_eq!(one("(begin (random-seed 1) (< -1 (random 10) 10))"), "#t");
    assert_eq!(one("(begin (random-seed 1) (<= 0 (random) 1))"), "#t");
    assert!(one("(random 0)").starts_with("ERROR"));
    assert!(one("(random 'x)").starts_with("ERROR"));
}

#[test]
fn ext_files() {
    std::fs::create_dir_all("target/tmp").unwrap();
    assert_eq!(one("(file-exists? \"Cargo.toml\")"), "#t");
    assert_eq!(one("(file-exists? \"no-such-file-xyz-123\")"), "#f");
    assert_eq!(one("(string? (current-directory))"), "#t");
    let r = run(&[
        "(with-output-to-file \"target/tmp/ext-delete-me.scm\" (lambda () (display \"x\")))",
        "(file-exists? \"target/tmp/ext-delete-me.scm\")",
        "(delete-file \"target/tmp/ext-delete-me.scm\")",
        "(file-exists? \"target/tmp/ext-delete-me.scm\")",
    ]);
    assert_eq!(r[1], "#t");
    assert_eq!(r[3], "#f");
}

#[test]
fn ext_require_modules() {
    // 不 require 时这些名字不存在（不与用户自定义冲突）
    assert!(one("(filter odd? '(1 2 3))").starts_with("ERROR: unbound variable"));
    assert!(one("(sort '(1) <)").starts_with("ERROR: unbound variable"));

    // list 模块
    let with_list = |expr: &str| one(&format!("(begin (require 'list) {})", expr));
    assert_eq!(with_list("(iota 5)"), "(0 1 2 3 4)");
    assert_eq!(with_list("(iota 3 10 2)"), "(10 12 14)");
    assert_eq!(with_list("(filter odd? '(1 2 3 4 5))"), "(1 3 5)");
    assert_eq!(with_list("(fold + 0 '(1 2 3 4))"), "10");
    assert_eq!(with_list("(fold cons '() '(1 2 3))"), "(3 2 1)");
    assert_eq!(with_list("(fold-right cons '() '(1 2 3))"), "(1 2 3)");
    assert_eq!(with_list("(reduce + 0 '(1 2 3 4))"), "10");
    assert_eq!(with_list("(reduce + 99 '())"), "99");
    assert_eq!(with_list("(last '(a b c))"), "c");
    assert_eq!(with_list("(take '(1 2 3 4) 2)"), "(1 2)");
    assert_eq!(with_list("(drop '(1 2 3 4) 2)"), "(3 4)");
    assert_eq!(with_list("(take-while odd? '(1 3 2 5))"), "(1 3)");
    assert_eq!(with_list("(drop-while odd? '(1 3 2 5))"), "(2 5)");
    assert_eq!(with_list("(find even? '(1 3 4 5))"), "4");
    assert_eq!(with_list("(find even? '(1 3 5))"), "#f");
    assert_eq!(with_list("(any even? '(1 3 4))"), "#t");
    assert_eq!(with_list("(every odd? '(1 3 4))"), "#f");
    assert_eq!(with_list("(zip '(a b) '(1 2))"), "((a 1) (b 2))");
    assert_eq!(with_list("(partition odd? '(1 2 3 4))"), "((1 3) (2 4))");
    assert_eq!(with_list("(delete-duplicates '(1 2 1 3 2))"), "(1 3 2)");
    assert_eq!(with_list("(sort '(3 1 2) <)"), "(1 2 3)");
    assert_eq!(with_list("(sort '(3 1 2) >)"), "(3 2 1)");
    // 稳定性：相等元素保持原顺序（用 car 比较 pair）
    assert_eq!(
        with_list("(sort '((2 . b) (1 . a) (2 . c)) (lambda (x y) (< (car x) (car y))))"),
        "((1 . a) (2 . b) (2 . c))"
    );

    // string 模块
    let with_str = |expr: &str| one(&format!("(begin (require 'string) {})", expr));
    assert_eq!(with_str("(string-reverse \"abc\")"), "\"cba\"");
    assert_eq!(with_str("(string-repeat \"ab\" 3)"), "\"ababab\"");
    assert_eq!(with_str("(string-trim \"  hi \t\")"), "\"hi\"");
    assert_eq!(with_str("(string-prefix? \"he\" \"hello\")"), "#t");
    assert_eq!(with_str("(string-suffix? \"lo\" \"hello\")"), "#t");
    assert_eq!(with_str("(string-contains? \"hello\" \"ll\")"), "2");
    assert_eq!(with_str("(string-contains? \"hello\" \"zz\")"), "#f");
    assert_eq!(
        with_str("(string-split \"a,b,,c\" #\\,)"),
        "(\"a\" \"b\" \"\" \"c\")"
    );
    assert_eq!(with_str("(string-join '(\"a\" \"b\") \"-\")"), "\"a-b\"");
    assert_eq!(
        with_str("(string-replace \"a-b-c\" \"-\" \"+\")"),
        "\"a+b+c\""
    );
    assert_eq!(with_str("(string-replace \"aaaa\" \"aa\" \"b\")"), "\"bb\"");

    // 未知模块报错并列出可用模块
    let err = one("(require 'nosuch)");
    assert!(err.contains("unknown module: nosuch"), "got: {}", err);
    assert!(err.contains("list"), "got: {}", err);
    // 参数必须是符号
    assert!(one("(require \"list\")").starts_with("ERROR:"));
}

#[test]
fn ext_trace_untrace() {
    let r = run(&[
        "(define (ext-f x) (if (= x 0) 0 (+ 1 (ext-f (- x 1)))))",
        "(trace 'ext-f)",
        "(ext-f 3)",
        "(untrace 'ext-f)",
        "(ext-f 1)",
    ]);
    assert_eq!(r[2], "3");
    assert_eq!(r[4], "1");
    // trace 未定义符号报错
    assert!(one("(trace 'no-such-var-xyz)").starts_with("ERROR"));
    // trace 非过程报错
    assert!(one("(trace 42)").starts_with("ERROR"));
    // untrace 无参清空全部
    assert_eq!(one("(begin (trace car) (untrace) 'ok)"), "ok");
}

#[test]
fn ext_pretty_print() {
    // 短列表平铺
    assert_eq!(
        one("(call-with-output-string (lambda (p) (pretty-print '(1 2 3) p)))"),
        "\"(1 2 3)\""
    );
    // 超长结构换行展开
    let env = standard_env();
    let v = eval_in(
        &env,
        "(call-with-output-string (lambda (p) (pretty-print '(define (fib n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))) p)))",
    )
    .unwrap();
    let s = display_to_string(&v);
    assert!(s.contains('\n'), "expected multiline output, got: {}", s);
}

/// let 族绑定列表畸形时，报错要指出是哪个绑定有问题（括号错位的常见症状：
/// body 被吞进绑定列表）。
#[test]
fn bad_binding_error_names_the_binding() {
    let err = one("(letrec ((iter (lambda (n1 p) n1)) (iter n n)) 1)");
    assert!(err.starts_with("ERROR:"));
    assert!(err.contains("(iter n n)"), "got: {}", err);
    assert!(err.contains("expected (name value)"), "got: {}", err);
    // 非列表绑定
    assert!(one("(let (x) 1)").contains("bad binding: x"));
    // 绑定名不是标识符
    assert!(one("(let ((1 2)) 3)").contains("binding name must be identifier: 1"));
}

// ---------------------------------------------------------------------------
// 回归：正确性 bug 修复

#[test]
fn expt_huge_exponent_errors() {
    // 超范围的指数报错，而不是被钳位后溢出成 -1
    assert_eq!(
        one("(expt 2 4294967296)"),
        "ERROR: expt: exponent too large"
    );
    // 正常行为不受影响
    assert_eq!(one("(expt 2 -3)"), "1/8");
    assert_eq!(one("(expt 2 10)"), "1024");
    assert_eq!(one("(expt 2 0)"), "1");
}

#[test]
fn nan_compares_false() {
    // IEEE 语义：NaN 参与的所有比较均为 #f
    assert_eq!(one("(= +nan.0 +nan.0)"), "#f");
    assert_eq!(one("(< +nan.0 1)"), "#f");
    assert_eq!(one("(> +nan.0 +nan.0)"), "#f");
    assert_eq!(one("(<= +nan.0 1)"), "#f");
    assert_eq!(one("(>= 1 +nan.0)"), "#f");
}

#[test]
fn exact_prefix_on_inf_nan_rejected() {
    // R5RS：无穷/NaN 没有精确表示
    assert!(one("#e+inf.0").starts_with("ERROR: bad number"));
    assert!(one("#e-nan.0").starts_with("ERROR: bad number"));
    assert_eq!(one("(string->number \"#e+inf.0\")"), "#f");
    // 不带 #e（或带 #i）行为不变
    assert_eq!(one("+inf.0"), "+inf.0");
    assert_eq!(one("#i+inf.0"), "+inf.0");
    assert_eq!(one("+nan.0"), "+nan.0");
}

#[test]
fn load_unbalanced_parens_errors() {
    // 括号不平衡的文件必须报错，而不是静默丢弃末尾 datum。
    // 用进程号保证文件名唯一（测试并行），写在 target/tmp/ 下。
    std::fs::create_dir_all("target/tmp").unwrap();
    let path = format!("target/tmp/load_unbalanced_{}.scm", std::process::id());
    let src = format!(
        "(begin (with-output-to-file \"{}\" (lambda () (display \"(define x (list 1 2)\"))) (load \"{}\"))",
        path, path
    );
    assert_eq!(one(&src), "ERROR: load: unexpected end of input");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn environment_specifiers_check_arity() {
    assert_eq!(
        one("(null-environment)"),
        "ERROR: null-environment: expected 1 args, got 0"
    );
    assert_eq!(
        one("(scheme-report-environment 5 6)"),
        "ERROR: scheme-report-environment: expected 1 args, got 2"
    );
    // 行为不变：返回完整交互环境（文档已承认的偏差）
    assert_eq!(one("(null-environment 5)"), "#<environment>");
    assert_eq!(
        one("(interaction-environment 1)"),
        "ERROR: interaction-environment: expected 0 args, got 1"
    );
    assert_eq!(one("(interaction-environment)"), "#<environment>");
}

#[test]
fn unspecified_is_self_eq() {
    assert_eq!(one("(let ((x (if #f #f))) (eq? x x))"), "#t");
}

#[test]
fn bar_is_symbol_char_not_delimiter() {
    // R5RS 没有 |...| 语法，| 是普通符号字符
    assert_eq!(one("'|foo|"), "|foo|");
}

#[test]
fn closed_port_read_errors() {
    assert_eq!(
        one("(let ((p (open-input-string \"x\"))) (close-input-port p) (read-char p))"),
        "ERROR: read-char: port is closed"
    );
    assert_eq!(
        one("(let ((p (open-input-string \"x\"))) (close-input-port p) (peek-char p))"),
        "ERROR: peek-char: port is closed"
    );
    assert_eq!(
        one("(let ((p (open-input-string \"x\"))) (close-input-port p) (read p))"),
        "ERROR: read: port is closed"
    );
}

// ---------------------------------------------------------------------------
// 回归：结构性修复

#[test]
fn macro_template_quote_respects_def_env() {
    // 模板里的 (quote x) 是否按 quotation 处理，取决于 quote 在宏定义
    // 环境中的解析：仍是内建关键字 → 数据；被重绑定为变量 → 普通组合。
    let r = run(&[
        "(define-syntax mq (syntax-rules () ((_ x) (quote x))))",
        "(mq a)",
        "(define quote list)",
        "(mq 1)",
    ]);
    assert_eq!(r[1], "a"); // quote 未重绑定：quotation
    assert_eq!(r[3], "(1)"); // quote 重绑定为 list：(list 1)
}

#[test]
fn var_macro_namespaces_last_definer_wins() {
    // 变量/宏两个命名空间同帧互斥，后定义者生效
    let r = run(&[
        "(define-syntax ns-a (syntax-rules () ((_) 'macro)))",
        "(ns-a)",
        "(define ns-a 'var)",
        "ns-a",
        "(ns-a)",
    ]);
    assert_eq!(r[1], "macro");
    assert_eq!(r[3], "var"); // define 清掉了同帧同名宏
    assert!(r[4].starts_with("ERROR: not a procedure")); // ns-a 已是普通变量
    let r = run(&[
        "(define ns-b 'var)",
        "ns-b",
        "(define-syntax ns-b (syntax-rules () ((_) 'macro)))",
        "(ns-b)",
        "ns-b",
    ]);
    assert_eq!(r[1], "var");
    assert_eq!(r[3], "macro"); // define-syntax 清掉了同帧同名变量
    assert!(r[4].starts_with("ERROR: unbound variable")); // 变量绑定已移除
}

#[test]
fn with_ports_dynamic_wind_normal_escape_reentry() {
    // 端口切换挂在 dynamic-wind 上：正常返回/call/cc 逃逸/重入都要以
    // 正确时机恢复端口；after 里 close（含 flush），写文件不丢数据。
    std::fs::create_dir_all("target/tmp").unwrap();
    let out_a = format!("target/tmp/withdw-a-{}.txt", std::process::id());
    let out_b = format!("target/tmp/withdw-b-{}.txt", std::process::id());
    let out_c = format!("target/tmp/withdw-c-{}.txt", std::process::id());
    // 正常返回：内容落盘，当前输出端口恢复
    let r = run(&[
        "(define dw-p (current-output-port))",
        &format!(
            "(with-output-to-file \"{}\" (lambda () (display \"abc\")))",
            out_a
        ),
        "(eq? dw-p (current-output-port))",
        &format!("(call-with-input-file \"{}\" (lambda (p) (read p)))", out_a),
    ]);
    assert_eq!(r[2], "#t");
    assert_eq!(r[3], "abc");
    // call/cc 逃逸：after 仍执行——端口恢复，逃逸前写的内容 flush 落盘
    let r = run(&[
        "(define dw-p (current-output-port))",
        &format!(
            "(call/cc (lambda (c) (with-output-to-file \"{}\" (lambda () (display \"esc\") (c #f)))))",
            out_b
        ),
        "(eq? dw-p (current-output-port))",
        &format!("(call-with-input-file \"{}\" (lambda (p) (read p)))", out_b),
    ]);
    assert_eq!(r[2], "#t");
    assert_eq!(r[3], "esc");
    // 重入被捕获的续延：before/after 再次执行，外层端口保持正确
    let r = run(&[
        "(define dw-p (current-output-port))",
        "(define dw-k #f)",
        &format!(
            "(with-output-to-file \"{}\" (lambda () (call/cc (lambda (c) (set! dw-k c)))))",
            out_c
        ),
        "(eq? dw-p (current-output-port))",
        "(dw-k 'reentered)",
        "(eq? dw-p (current-output-port))",
    ]);
    assert_eq!(r[3], "#t");
    assert_eq!(r[4], "reentered");
    assert_eq!(r[5], "#t");
    // with-input-from-file 逃逸同样恢复
    let r = run(&[
        &format!(
            "(call-with-output-file \"{}\" (lambda (p) (display \"in-data\" p)))",
            out_a
        ),
        "(define dw-ip (current-input-port))",
        &format!(
            "(call/cc (lambda (c) (with-input-from-file \"{}\" (lambda () (c #f)))))",
            out_a
        ),
        "(eq? dw-ip (current-input-port))",
    ]);
    assert_eq!(r[3], "#t");
    let _ = std::fs::remove_file(&out_a);
    let _ = std::fs::remove_file(&out_b);
    let _ = std::fs::remove_file(&out_c);
}
