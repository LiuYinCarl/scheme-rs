# 测试体系

scheme-rs 的验证分四层：三套 Scheme 套件、Rust 单元/集成测试、
覆盖率、criterion 基准。

## 三套 Scheme 套件（tests/scm/）

由 `tests/r5rs_suites.rs` 用 `env!("CARGO_BIN_EXE_scheme-rs")` 起子进程
运行真实二进制并检查输出（此方式在 Linux CI 上同样工作）：

| 套件 | 来源 | 覆盖点 | 结果 |
|---|---|---|---|
| `r5rs-tests.scm` | chibi-scheme 的 R5RS 套件 | 第 4/6 章的常规语义：各特殊形式、quasiquote 嵌套、等价谓词、数字、字符串/向量/列表、dynamic-wind 重入、卫生示例（`else`/`=>`/`unquote`/`...` 被局部绑定）、自定义 ellipsis | 188/189，1 例白名单 |
| `r5rs_pitfall.scm` | SISC 的 R5RS 边界套件 | letrec+call/cc 重入（1.x）、尾位置应用（2.1）、卫生宏（3.x）、关键字可遮蔽（4.x）、#f/() 区分（5.x）、多射续延交错重入（7.x）、named-let 作用域（8.x） | 22 个 Passed，0 Failure |
| `r5rs-examples.scm` | 从 R5RS 报告 HTML 自动提取的示例（见下节） | 报告第 4/5/6 章几乎全部 `==>` 示例 | 253/253 |

**白名单机制**：`tests/r5rs_suites.rs` 顶部的 `KNOWN_FAILURES_*` 数组，
按子串匹配失败行。当前仅 1 例：chibi 套件的
`(symbol->string 'Martin)`——R5RS 要求大小写折叠（期望 "martin"），
chibi 期望 "Martin"，规范层面不可兼得，选择遵循报告（注释里写了
完整理由）。pitfall 与 examples 的白名单为空。

判定方式：chibi 套件不允许未在白名单内的 `[FAIL]` 行；pitfall 不允许
`Failure:` 行；examples 不允许 `FAIL:` 行；三者都要求出现完成标记
（"out of ... passed" / "Passed: 8.3"）且进程 exit 0。

## 示例提取工具（tools/extract_r5rs_examples.py）

用法：`extract_r5rs_examples.py ch7.html ch8.html ch9.html > out.scm`
（输入是 R5RS HTML 版报告的第 7/8/9 章，即"形式语法之前的全部章节"
里的示例所在的三个文件）。

工作原理：

1. **抓 `<tt>...</tt>` 块**：报告里示例代码都在 `<tt>` 中。`<tt>` 可
   嵌套（块内还可能有 `<tt>#f</tt>`），所以用深度计数而不是正则配对
   （曾经修过的嵌套 bug）。
2. **找 `==>` 箭头**：箭头前是表达式、箭头后是期望结果。粘在闭括号
   后的散文句号（`x))).`）会被剥掉，避免误当点对语法。
3. **过滤规则**（被滤掉时仍会"抢救"块内合法的顶层 `define`，因为
   后续示例可能依赖它们，例如 delay/force 例前的定义）：
   - 结果为 error/unspecified/非单一 datum（如 "a promise"、"a
     procedure"）的示例：error 的丢弃，unspecified 的保留表达式本身
     （副作用可能有顺序意义）；
   - 含元变量（`obj1`、`n1`、`q`、`<variable>` 等形式文法记号）的
     "伪示例"；
   - 复数示例（`3+4i`、`make-rectangular` 等，数字塔有意省略复数）；
   - 已知损坏或非标准片段：丢失 `#\` 前缀的 char 示例、元变量散文
     （`(char<=? (integer->char x) ...)`）、"implicit forcing"（
     `(+ (delay ...) 13)`，R5RS 明确标注为可选扩展）；
   - 期望值跨行的结果会继续吞行直到括号配平成恰好一个 datum。
4. 生成的文件自带 chibi 风格 `test` 宏与计数器，失败打印 `FAIL:`，
   末尾打印 `N out of M passed`。

手工修补记录（提取器无法自动恢复的损伤，已直接在生成文件里修复并
在任务记录中说明）：`make-promise` 定义（R5RS 4.2.5 原文有、提取时
丢失）按原文补回；一处跨行期望值被截断的 dynamic-wind 用例补全。

## Rust 测试结构

- `tests/r5rs_suites.rs`：上述三套件的集成测试（子进程方式）。
- `tests/scheme_units.rs`（27 个）：reader/printer 往返、精确算术与
  进制、`#` 占位数字、inexact 整函数、超越函数、5×10⁵ 尾递归与各
  尾位置、call/cc 多射重入、dynamic-wind 逃逸/重入、syntax-rules
  卫生（定义处解析、不捕获、嵌套/零次重复 ellipsis、`(... ...)`）、
  嵌套 quasiquote、等价谓词（含环形结构）、promise 缓存与重入、
  字符串端口、`values`、脚本未闭合报错（子进程）。
- `src/repl.rs` 内 `#[cfg(test)]`（3 个）：datum 完整性判断、补全
  词表生成、补全起点计算。

## 覆盖率

- 本地实测（cargo-llvm-cov 0.9）：**行覆盖率 75.17%**（3918 行）。
  低分项是 `port.rs`/`repl.rs`（I/O 与交互路径难单测），核心求值与
  数字模块在 84–87%。
- CI（`.github/workflows/ci.yml` 的 coverage job）：
  `cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info`
  + `--summary-only` + `--fail-under-lines 70`（比实测低约 5 个百分点，
  workflow 注释里写了实测值），HTML 报告与 lcov 上传 artifact。
- 本地生成：
  `cargo llvm-cov --all-features --workspace --summary-only`、
  `cargo llvm-cov report --html`（报告在 `target/llvm-cov/html/`）。

## criterion 基准（benches/interpreter.rs）

`cargo bench --bench interpreter`（注意必须带 `--bench interpreter`，
否则参数会传给默认 test harness 而报错）。各用例在全新的全局环境里
求值一段 Scheme 程序（除 reader 用例外）：

| 用例 | 测量内容 | 本地参考值 |
|---|---|---|
| `fib_recursion_20` | 普通递归调用的帧分配/环境创建开销 | ~30 ms |
| `tail_loop_100k` | TCO 路径：10 万次尾调用的稳态成本（同时验证常数栈） | ~214 ms |
| `map_over_1000` | 内建 map 的帧化迭代 + 闭包调用 | ~4.6 ms |
| `string_and_number_mix` | BigInt 算术 + number->string + string-append | ~790 µs |
| `reader_r5rs_tests_scm` | reader 解析整个 chibi 套件（~10KB 源码）的吞吐 | ~246 µs |

CI 的 bench job 只做记录（上传 artifact），不设回归门禁。
