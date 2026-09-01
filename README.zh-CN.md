# scheme-rs

[![CI](https://github.com/LiuYinCarl/scheme-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/LiuYinCarl/scheme-rs/actions/workflows/ci.yml)

[English README](README.md)

一个用 Rust 编写的 R5RS Scheme 解释器：基于显式 persistent 续延栈的
trampoline 求值器（tree-walking），实现了正确的尾递归、一等多射续延
（multi-shot continuation）和正确的 `dynamic-wind` 重入——求值过程不
依赖 Rust 原生递归。

## 演示

REPL 基础（语法高亮、named `let`、精确有理数）：

![repl demo](docs/screenshots/repl.gif)

一等 `call/cc`（先逃逸，再重入保存的续延）：

![call/cc demo](docs/screenshots/callcc.gif)

通过 `require` 加载标准库 + 对库函数使用 `trace`：

![stdlib demo](docs/screenshots/stdlib.gif)

`define-syntax` 卫生宏：

![macro demo](docs/screenshots/macro.gif)

多行编辑、bignum 运算与 `(time)`：

![multiline demo](docs/screenshots/multiline.gif)

演示由脚本化 REPL 会话录制（`scripts/record_demos.sh`，
asciinema + agg；见 `scripts/demos/*.demo`）。如何重新录制或新增演示：
[docs/demos.md](docs/demos.md)。

## 测试与性能（摘要）

| 验证项 | 结果 |
|---|---|
| chibi R5RS 套件（`tests/scm/r5rs-tests.scm`） | **188/189**（1 例白名单：大小写冲突，见[说明](docs/testing.md)） |
| SISC R5RS pitfalls（`tests/scm/r5rs_pitfall.scm`） | **22/22**（letrec+call/cc、多射续延、卫生宏、TCO 等最刁钻用例） |
| R5RS 报告示例提取套件（`tests/scm/r5rs-examples.scm`） | **253/253** |
| 真实程序（`tests/scm/programs/`：11 个 Gabriel 基准 + SICP mceval/amb/regmach + Schelog） | **15/15 全过**，含 nboyer 精确命中 **95024 rewrites**、SICP 第 5 章编译器、Prolog 嵌入 |
| Rust 单元 + 集成测试 | **84 个**（55 scheme_units + 20 r5rs_suites + 9 crate 内单元；统一入口 `scripts/test.sh`） |
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

## 用法

```
cargo build
cargo run -- path/to/file.scm   # 运行文件
cargo run                        # REPL（语法高亮默认开启，--no-highlight 关闭）
cargo test                       # 单元 + 集成测试（必须全绿）
```

## 文档

中文设计文档（面向想学习解释器设计的读者）：

- [docs/guide.md](docs/guide.md) — 使用指南：全部可用函数与示例
- [docs/architecture.md](docs/architecture.md) — 总体架构：trampoline
  求值器、persistent 续延栈、call/cc、dynamic-wind、location 环境
- [docs/syntax-rules.md](docs/syntax-rules.md) — 宏系统与重命名式卫生
- [docs/numeric-tower.md](docs/numeric-tower.md) — 数字塔与精确性规则
- [docs/r5rs-compliance.md](docs/r5rs-compliance.md) — R5RS 符合性清单与有意偏差
- [docs/extensions.md](docs/extensions.md) — R5RS 之外的扩展（random/runtime/trace/pretty-print 等）与纯 Scheme 标准库模块（list/string/option/result/vector/stream/map/set/format/buffer）
- [docs/demos.md](docs/demos.md) — README 演示 gif 的录制方法
- [docs/testing.md](docs/testing.md) — 测试体系、覆盖率与全部结果
- [docs/benchmarks.md](docs/benchmarks.md) — 性能专题：criterion 与实战程序耗时

## 开发

```
cargo fmt --check                            # 格式检查（CI 门禁）
cargo clippy --all-targets -- -D warnings    # lint（CI 门禁）
cargo test                                   # 单元 + 集成测试
cargo llvm-cov --all-features --workspace --summary-only   # 行覆盖率
cargo llvm-cov report --all-features --workspace --html    # HTML 报告
cargo bench --bench interpreter              # criterion 基准
```

REPL 是 Jupyter 风格的（`src/repl.rs`）：`In [n]:` / `Out[n]:` 编号
提示符、多行编辑（validator 会把括号未配平的输入保留在同一个可编辑
缓冲区中——光标可以跨行移动，历史记录会一次性召回整条多行输入）、
ANSI 颜色（非 TTY 时自动关闭）、语法高亮
（`--no-highlight` 关闭）、光标后以暗灰色实时显示 read 错误提示
（`--no-hint` 关闭）、基于当前全局环境与特殊形式的 Tab 补全、
持久化历史（`$XDG_DATA_HOME/scheme-rs/history` 或
`~/.scheme-rs_history`）、Ctrl-C 丢弃当前输入、`(exit)` 或 Ctrl-D 退出。

## 架构

| 模块 | 内容 |
|---|---|
| `src/value.rs` | `Value` 表示、符号 intern、gensym/rename 表（卫生）、`eq?`/`eqv?`/`equal?`（环形安全） |
| `src/reader.rs` | 完整 datum reader：注释、`#t #f #\c "s" ' ` , ,@`、vector、dotted pair、进制/精确性前缀（`#b #o #d #x #e #i`，可组合） |
| `src/printer.rs` | `write`/`display`、quote 缩写、环形检测 |
| `src/number.rs` | 数字塔：精确整数（BigInt）、精确有理数（BigRational，始终约分）、inexact 实数（f64）；contagion 遵循 R5RS |
| `src/env.rs` | 环境将符号映射到 *location*（`Rc<RefCell<Value>>`）、宏命名空间、rename 感知解析、`free-identifier=?` |
| `src/eval.rs` | trampoline：`State::{Eval, Return, Apply}` + persistent 续延帧；特殊形式；派生形式脱糖；quasiquote；internal define 处理（letrec 语义，批量赋值） |
| `src/syntax_rules.rs` | 模式匹配（literal、`_`、`.`、嵌套 vector/list、嵌套 ellipsis、`(... ...)` 转义、自定义 ellipsis 标识符）、模板展开、重命名式卫生 |
| `src/builtins.rs` | 全部库过程 |
| `src/port.rs` | stdin/stdout、文件端口、字符串端口 |
| `src/repl.rs` | Jupyter 风格 REPL（rustyline）：编号提示符、续行、补全、历史 |
| `src/main.rs` | CLI 分派（文件执行 vs REPL） |

### 关键设计点

- **正确的尾递归。** Scheme 层面的控制流求值从不经过 Rust 栈递归。
  机器状态是一条显式续延栈（`Option<Rc<ContFrame>>`，persistent
  链表）。尾调用复用当前续延而不是压入新帧。
  `(let loop ((n 500000)) (if (= n 0) 'done (loop (- n 1))))` 在常数
  栈空间内运行（有单元测试覆盖）。
- **一等续延。** 捕获 `call/cc` 只是对续延栈指针加 dynamic-wind 链做
  O(1) 快照。由于栈是 persistent 的，续延天然是 multi-shot 的
  （SISC pitfalls 7.1–7.4 全过）。逃逸过程接受任意数量的参数，并以
  multiple values 交付（R5RS 6.4），因此报告中用 `call/cc` + `apply`
  定义 `values` 的写法可以直接工作。
- **dynamic-wind。** 每个续延记录自己的 wind 链。调用续延时，按指针
  同一性计算当前与目标 wind 链的公共前缀，并在恢复之前按顺序运行
  所需的 `after`/`before` thunk。
- **环境持有 location 而不是值。** `set!` 修改共享的
  `Rc<RefCell<Value>>` 单元，因此重入 `letrec` 初始化式的续延能观察到
  之后的赋值（pitfalls 1.1/1.2）。`letrec` 按"先求值全部初始化式、再
  统一赋值"的语义编译（赋值使用新鲜临时变量），这正是 pitfalls
  1.1/1.2 所要求的。
- **卫生。** `syntax-rules` 模板把引入的标识符重命名为新鲜不可读符号，
  记录（原始名字，定义环境）；找不到局部绑定的引用回落到定义环境中的
  原始名字。`quote` 模板内的标识符是数据，不会被重命名。辅助语法
  （`else`、`=>`、`unquote`、`unquote-splicing`、ellipsis）的识别方式
  是沿 rename 回溯到原始标识符，并检查它在用处未被重新绑定，因此
  `(let ((=> #f)) (cond (#t => 'ok)))` 和
  `(let ((unquote 1)) \`(,foo))` 的行为符合 R5RS。
- **map** 基于显式续延栈实现，因此是 call/cc 安全的（pitfall 套件会
  打印 "Map is call/cc safe ..."）。

## 覆盖范围

- 语法：`quote quasiquote unquote unquote-splicing lambda if define set!
  cond case and or let let* letrec named-let begin do delay force
  define-syntax let-syntax letrec-syntax`（含 internal define、
  curried `define`、带 `=>` 的 `cond`/`case`、`case` 扩展）。
- 完整 `syntax-rules`：literal、`_`、点分模式、vector 模式、嵌套
  ellipsis、`(... ...)` 转义、自定义 ellipsis 标识符。
- 数字塔：integer / rational / real，带精确性 contagion；
  `quotient remainder modulo gcd lcm numerator denominator floor ceiling
  truncate round rationalize expt sqrt abs max min exact->inexact
  inexact->exact number->string string->number`（支持各进制；整数运算
  按 R5RS 6.2.5 接受 inexact 整数；`#` 数字占位符按 R5RS 6.2.4），
  另有 `exp log sin cos tan asin acos atan`。
- 序对/列表（全部 28 个 `cxr` 组合子、检测环的 `list?`、多列表
  `map`/`for-each`、`apply`）、符号、字符（含 `char-ci` 一族）、字符串
  （含 `string-ci` 一族、`string-fill!`、`string-copy`）、vector。
- 控制：`procedure? call/cc values call-with-values dynamic-wind eval
  scheme-report-environment null-environment interaction-environment`。
- I/O：完整 R5RS 端口集，外加扩展 `open-input-string
  open-output-string get-output-string call-with-output-string flush-output
  error`（chibi 测试框架使用），以及 `load`。

## 有意偏差与省略

- **复数被省略。** `complex?` 对已支持的实数/有理数回答 `#t`；
  对负数开 `sqrt` 会报错。
- `scheme-report-environment` / `null-environment` 返回完整的交互环境，
  而不是受限的报告专用环境。
- `char-ready?` 总是返回 `#t`。

## 大小写敏感性

按 R5RS 第 2 章（"Upper and lower case forms of a letter are never
distinguished except within character and string constants"），**reader
将标识符折叠为小写**；`string->symbol` 保留大小写，所以
`(eq? (string->symbol "f") (string->symbol "F"))` ⇒ `#f`（pitfall 6.1）。
这使得 R5RS 报告自己的示例 `(symbol->string 'Martin)` ⇒ `"martin"`
在 r5rs-examples 套件中通过。同一表达式在 chibi 套件中却期望
`"Martin"`（chibi 是大小写敏感的）——这是一个不可兼得的冲突；chibi
的这一例是已知失败白名单中唯一的条目（见 `tests/r5rs_suites.rs`）。

## 测试

- `tests/r5rs_suites.rs` 进程内运行三套内置套件：
  - `tests/scm/r5rs-tests.scm`（chibi R5RS 套件）：**188/189 通过**；
    剩余一例即上文的大小写冲突（已入白名单）。
  - `tests/scm/r5rs_pitfall.scm`（SISC pitfalls）：**全部通过**
    （1.1–8.3，外加 "Map is call/cc safe"）。
  - `tests/scm/r5rs-examples.scm`（从 R5RS 第 4/5/6 章提取的示例）：
    **253/253 通过**。少量提取产物（引用了不存在的元变量或变量的伪
    示例、被搅进代码里的可选扩展散文）已从生成文件中移除；提取器
    丢失报告上下文的两处（R5RS 4.2.5 的 `make-promise` 定义、一个
    多行期望值的一半）已按原文补回；具体形式见任务日志。
- `tests/scheme_units.rs`：reader/printer 往返、精确算术、进制转换与
  `#` 数字占位符、inexact 整数运算、超越函数、50 万层深尾递归、所有
  位置的尾调用、multi-shot `call/cc`、`dynamic-wind` 逃逸/重入、宏卫生
  （定义处解析、无捕获、嵌套 ellipsis、`(... ...)` 转义）、嵌套
  quasiquote、等价谓词、大小写折叠、字符串端口、`values`。
- `tests/scm/libs/*-test.scm`：`src/libs/` 中每个标准库模块（
  list/string/option/result/vector/stream/map/set/format/buffer）对应
  一个 Scheme 测试文件，由 `tests/r5rs_suites.rs` 中的
  `libs::scheme_libs` 测试驱动，使用 `check` 断言框架。
