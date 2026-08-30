//! Interpreter benchmarks. Each case evaluates a small Scheme program via the
//! library API in a fresh global environment.

use criterion::{criterion_group, criterion_main, Criterion};
use scheme_rs::builtins::standard_env;
use scheme_rs::eval::eval_str;
use scheme_rs::reader;

fn run(src: &str) {
    let env = standard_env();
    eval_str(src, &env).expect("bench program failed");
}

fn bench_eval(c: &mut Criterion) {
    c.bench_function("fib_recursion_20", |b| {
        b.iter(|| {
            run("(define (fib n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))
                 (fib 20)")
        })
    });

    c.bench_function("tail_loop_100k", |b| {
        b.iter(|| {
            run("(let loop ((n 100000) (acc 0))
                   (if (= n 0) acc (loop (- n 1) (+ acc 1))))")
        })
    });

    c.bench_function("map_over_1000", |b| {
        b.iter(|| {
            run("(define xs (let loop ((n 1000) (acc '()))
                              (if (= n 0) acc (loop (- n 1) (cons n acc)))))
                 (map (lambda (x) (* x x)) xs)")
        })
    });

    c.bench_function("string_and_number_mix", |b| {
        b.iter(|| {
            run("(let loop ((n 200) (s \"\"))
                   (if (= n 0)
                       (string-length s)
                       (loop (- n 1)
                             (string-append s (number->string (* n n))))))")
        })
    });
}

fn bench_reader(c: &mut Criterion) {
    let src = std::fs::read_to_string("tests/scm/r5rs-tests.scm").unwrap();
    c.bench_function("reader_r5rs_tests_scm", |b| {
        b.iter(|| reader::read_all(&src).unwrap())
    });
}

criterion_group!(benches, bench_eval, bench_reader);
criterion_main!(benches);
