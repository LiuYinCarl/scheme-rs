# scheme-rs

[![CI](https://github.com/LiuYinCarl/scheme-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/LiuYinCarl/scheme-rs/actions/workflows/ci.yml)

An R5RS Scheme interpreter written in Rust: a tree-walking evaluator built on
an explicit, persistent continuation stack (a trampoline), giving proper tail
recursion, first-class multi-shot continuations, and correct `dynamic-wind`
re-entry — without using native Rust recursion for evaluation.

## 测试与性能（摘要）

| 验证项 | 结果 |
|---|---|
| chibi R5RS 套件（`tests/scm/r5rs-tests.scm`） | **188/189**（1 例白名单：大小写冲突，见[说明](docs/testing.md)） |
| SISC R5RS pitfalls（`tests/scm/r5rs_pitfall.scm`） | **22/22**（letrec+call/cc、多射续延、卫生宏、TCO 等最刁钻用例） |
| R5RS 报告示例提取套件（`tests/scm/r5rs-examples.scm`） | **253/253** |
| 真实程序（`tests/scm/programs/`：11 个 Gabriel 基准 + SICP mceval/amb/regmach + Schelog） | **15/15 全过**，含 nboyer 精确命中 **95024 rewrites**、SICP 第 5 章编译器、Prolog 嵌入 |
| Rust 单元 + 集成测试 | **56 个**（34 单元 + 3 REPL + 19 集成；统一入口 `scripts/test.sh`） |
| 行覆盖率（cargo-llvm-cov） | **75.17%**（CI 门禁 70） |
| CI | fmt / clippy / test（**Ubuntu + macOS**）/ coverage / bench 全绿 |

性能参考（criterion，2026-08-30 实测，Apple M5 / arm64 / 24GB / macOS 26.6，
`cargo bench --bench interpreter`）：

| 用例 | 耗时 | 说明 |
|---|---|---|
| `fib_recursion_20` | 29.3 ms | 普通递归调用 |
| `tail_loop_100k` | 212.8 ms | 10 万次尾调用（常数栈，验证 TCO 路径） |
| `map_over_1000` | 4.9 ms | 内建 map + 闭包调用 |
| `string_and_number_mix` | 661 µs | BigInt 运算 + 字符串拼接 |
| `reader_r5rs_tests_scm` | 231 µs | reader 解析 ~10KB 源码 |
| **nboyer(0)**（实战程序，非 criterion） | **3.4 s / 95024 rewrites** | ≈ 28k rewrites/s |

详情：[docs/testing.md](docs/testing.md)（测试体系与全部结果）、
[docs/benchmarks.md](docs/benchmarks.md)（性能专题与复现方法）。

## Usage

```
cargo build
cargo run -- path/to/file.scm   # run a file
cargo run                        # REPL（语法高亮默认开启，--no-highlight 关闭）
cargo test                       # unit + integration tests (must be green)
```

## Documentation

中文设计文档（面向想学习解释器设计的读者）：

- [docs/guide.md](docs/guide.md) — 使用指南：全部可用函数与示例
- [docs/architecture.md](docs/architecture.md) — 总体架构：trampoline
  求值器、persistent 续延栈、call/cc、dynamic-wind、location 环境
- [docs/syntax-rules.md](docs/syntax-rules.md) — 宏系统与重命名式卫生
- [docs/numeric-tower.md](docs/numeric-tower.md) — 数字塔与精确性规则
- [docs/r5rs-compliance.md](docs/r5rs-compliance.md) — R5RS 符合性清单与有意偏差
- [docs/extensions.md](docs/extensions.md) — R5RS 之外的扩展（random/runtime/trace/pretty-print/prelude 等）
- [docs/testing.md](docs/testing.md) — 测试体系、覆盖率与全部结果
- [docs/benchmarks.md](docs/benchmarks.md) — 性能专题：criterion 与实战程序耗时

## Development

```
cargo fmt --check                            # formatting (CI gates on this)
cargo clippy --all-targets -- -D warnings    # lints (CI gates on this)
cargo test                                   # unit + integration tests
cargo llvm-cov --all-features --workspace --summary-only   # line coverage
cargo llvm-cov report --all-features --workspace --html    # HTML report
cargo bench --bench interpreter              # criterion benchmarks
```

The REPL is Jupyter-styled (`src/repl.rs`): `In [n]:` / `Out[n]:` numbered
prompts, continuation lines (`....:`) while a datum is unbalanced, ANSI
colors (auto-disabled when not a TTY), syntax highlighting
(`--no-highlight` to disable), Tab completion from the live global
environment plus special forms, persistent history
(`$XDG_DATA_HOME/scheme-rs/history` or `~/.scheme-rs_history`), Ctrl-C to
discard the current input, `(exit)` or Ctrl-D to quit.

## Architecture

| Module | Contents |
|---|---|
| `src/value.rs` | `Value` representation, symbol interning, gensym/rename table (hygiene), `eq?`/`eqv?`/`equal?` (cycle-safe) |
| `src/reader.rs` | Full datum reader: comments, `#t #f #\c "s" ' ` , ,@`, vectors, dotted pairs, radix/exactness prefixes (`#b #o #d #x #e #i`, composable) |
| `src/printer.rs` | `write`/`display`, quote abbreviations, cycle detection |
| `src/number.rs` | Numeric tower: exact integers (BigInt), exact rationals (BigRational, always normalized), inexact reals (f64); contagion per R5RS |
| `src/env.rs` | Environments mapping symbols to *locations* (`Rc<RefCell<Value>>`), macro namespace, rename-aware resolution, `free-identifier=?` |
| `src/eval.rs` | The trampoline: `State::{Eval, Return, Apply}` + persistent continuation frames; special forms; derived-form desugaring; quasiquote; internal-define handling (letrec semantics with batch assignment) |
| `src/syntax_rules.rs` | Pattern matching (literals, `_`, `.`, nested vectors/lists, nested ellipses, `(... ...)` escape, custom ellipsis identifier), template expansion, hygiene by renaming |
| `src/builtins.rs` | All library procedures |
| `src/port.rs` | stdin/stdout, file ports, string ports |
| `src/repl.rs` | Jupyter-style REPL (rustyline): numbered prompts, continuation lines, completion, history |
| `src/main.rs` | CLI dispatch (file execution vs REPL) |

### Key design points

- **Proper tail recursion.** Evaluation never recurses through Rust's stack for
  Scheme-level control flow. The machine state is an explicit continuation
  stack (`Option<Rc<ContFrame>>`, a persistent linked list). Tail calls reuse
  the current continuation instead of pushing a frame.
  `(let loop ((n 500000)) (if (= n 0) 'done (loop (- n 1))))` runs in constant
  stack space (covered by a unit test).
- **First-class continuations.** Capturing `call/cc` is an O(1) snapshot of the
  continuation-stack pointer plus the dynamic-wind list. Since the stacks are
  persistent, continuations are multi-shot by construction (SISC pitfalls
  7.1–7.4 pass). Escape procedures accept any number of arguments and deliver
  them as multiple values (R5RS 6.4), so the report's own definition of
  `values` via `call/cc` + `apply` works.
- **dynamic-wind.** Each continuation records its wind list. Invoking a
  continuation computes the common prefix of the current and target wind
  lists (by pointer identity) and runs the required `after`/`before` thunks in
  order before resuming.
- **Environments hold locations, not values.** `set!` mutates a shared
  `Rc<RefCell<Value>>` cell, so continuations re-entering a `letrec`
  initializer observe later assignments (pitfalls 1.1/1.2). `letrec` is
  compiled with the "evaluate all inits, then assign all" semantics
  (assignments use fresh temporaries), which pitfalls 1.1/1.2 require.
- **Hygiene.** `syntax-rules` templates rename introduced identifiers to fresh
  unreadable symbols recording (original name, definition environment);
  references that don't find a local binding fall back to the original name in
  the definition environment. Identifiers inside `quote` templates are data
  and are not renamed. Auxiliary syntax (`else`, `=>`, `unquote`,
  `unquote-splicing`, the ellipsis) is recognized by following renames back to
  the original identifier and checking it is not rebound at the use site, so
  e.g. `(let ((=> #f)) (cond (#t => 'ok)))` and
  `(let ((unquote 1)) \`(,foo))` behave per R5RS.
- **map** is implemented over the explicit continuation stack, so it is
  call/cc safe (the pitfall suite prints "Map is call/cc safe ...").

## Coverage

- Syntax: `quote quasiquote unquote unquote-splicing lambda if define set!
  cond case and or let let* letrec named-let begin do delay force
  define-syntax let-syntax letrec-syntax` (including internal defines,
  curried `define`, `cond`/`case` with `=>`, `case` extension).
- Full `syntax-rules`: literals, `_`, dotted patterns, vector patterns,
  nested ellipses, `(... ...)` escape, custom ellipsis identifier.
- Numeric tower: integer / rational / real with exactness contagion;
  `quotient remainder modulo gcd lcm numerator denominator floor ceiling
  truncate round rationalize expt sqrt abs max min exact->inexact
  inexact->exact number->string string->number` (radices supported; integer
  operations accept inexact integers per R5RS 6.2.5; `#` digit placeholders
  per R5RS 6.2.4), plus `exp log sin cos tan asin acos atan`.
- Pairs/lists (all 28 `cxr` combinators, cycle-detecting `list?`, multi-list
  `map`/`for-each`, `apply`), symbols, characters (incl. `char-ci` family),
  strings (incl. `string-ci` family, `string-fill!`, `string-copy`), vectors.
- Control: `procedure? call/cc values call-with-values dynamic-wind eval
  scheme-report-environment null-environment interaction-environment`.
- I/O: full R5RS port set plus extensions `open-input-string
  open-output-string get-output-string call-with-output-string flush-output
  error` (used by the chibi test harness), and `load`.

## Intentional deviations / omissions

- **Complex numbers are omitted.** `complex?` answers `#t` for the supported
  reals/rationals; `sqrt` of a negative number raises an error.
- `scheme-report-environment` / `null-environment` return the full interaction
  environment rather than a restricted report-only environment.
- `char-ready?` always returns `#t`.
- `with-input-from-file` / `with-output-to-file` restore ports on normal
  return only (not on continuation jumps).

## Case sensitivity

Per R5RS section 2 ("Upper and lower case forms of a letter are never
distinguished except within character and string constants"), the **reader
folds identifiers to lower case**; `string->symbol` preserves case, so
`(eq? (string->symbol "f") (string->symbol "F"))` ⇒ `#f` (pitfall 6.1). This
makes the R5RS report's own example `(symbol->string 'Martin)` ⇒ `"martin"`
pass in the r5rs-examples suite. The same expression appears in the chibi
suite expecting `"Martin"` (chibi is case-sensitive) — an unsatisfiable
conflict; that single chibi case is the only entry in the known-failure
whitelist (see `tests/r5rs_suites.rs`).

## Tests

- `tests/r5rs_suites.rs` runs the three bundled suites as subprocesses:
  - `tests/scm/r5rs-tests.scm` (chibi R5RS suite): **188/189 pass**; the one
    remaining case is the case-sensitivity conflict above (whitelisted).
  - `tests/scm/r5rs_pitfall.scm` (SISC pitfalls): **all pass** (1.1–8.3,
    plus "Map is call/cc safe").
  - `tests/scm/r5rs-examples.scm` (examples extracted from R5RS chapters
    4/5/6): **253/253 pass**. A handful of extraction artifacts
    (pseudo-examples referencing metavariables or variables that do not
    exist, optional-extension prose mangled into code) were removed from the
    generated file, and two spots where the extractor dropped context from
    the report (the `make-promise` definition of R5RS 4.2.5, half of a
    multi-line expected value) were restored verbatim; see the task log for
    the exact forms.
- `tests/scheme_units.rs`: reader/printer roundtrips, exact arithmetic,
  radix conversions and `#` digit placeholders, inexact integer operations,
  transcendental functions, 500k-deep tail recursion, tail calls in all
  positions, multi-shot `call/cc`, `dynamic-wind` escape/re-entry, macro
  hygiene (definition-site resolution, no capture, nested ellipsis,
  `(... ...)` escape), nested quasiquote, equivalence predicates, case
  folding, string ports, `values`.
