# 测试体系

scheme-rs 的验证分五层：三套 Scheme 套件、真实程序实战、Rust 单元/
集成测试、覆盖率、criterion 基准。

## 三套 Scheme 套件（tests/scm/）

由 `tests/r5rs_suites.rs` 用 `env!("CARGO_BIN_EXE_scheme-rs")` 起子进程
运行真实二进制并检查输出（此方式在 Linux CI 上同样工作）：

| 套件 | 来源 | 覆盖点 | 结果 |
|---|---|---|---|
| `r5rs-tests.scm` | chibi-scheme 的 R5RS 套件 | 第 4/6 章的常规语义：各特殊形式、quasiquote 嵌套、等价谓词、数字、字符串/向量/列表、dynamic-wind 重入、卫生示例（`else`/`=>`/`unquote`/`...` 被局部绑定）、自定义 ellipsis | **188/189**，1 例白名单 |
| `r5rs_pitfall.scm` | SISC 的 R5RS 边界套件 | letrec+call/cc 重入（1.x）、尾位置应用（2.1）、卫生宏（3.x）、关键字可遮蔽（4.x）、#f/() 区分（5.x）、多射续延交错重入（7.x）、named-let 作用域（8.x） | **22/22** Passed，0 Failure |
| `r5rs-examples.scm` | 从 R5RS 报告 HTML 自动提取的示例（见下节） | 报告第 4/5/6 章几乎全部 `==>` 示例 | **253/253** |

pitfall 末尾还输出 "Map is call/cc safe, but probably not tail recursive
or inefficient."——这是该测试允许两种结果之一的信息行，我们的 map 用
显式续延帧实现，可任意重入（call/cc 安全）但不是尾递归风格。

**白名单机制**：`tests/r5rs_suites.rs` 顶部的 `KNOWN_FAILURES_*` 数组，
按子串匹配失败行。当前唯一条目（chibi 套件）：

```scheme
(test "Martin" (symbol->string 'Martin))
```

原因：R5RS 第 2 章规定标识符不区分大小写（"Upper and lower case forms
of a letter are never distinguished except within character and string
constants"），且报告 6.3.3 的示例就是 `(symbol->string 'Martin)` ⇒
`"martin"`（r5rs-examples 套件验证这一点）；chibi 是大小写敏感的
实现，对**同一表达式**期望 `"Martin"`。两者不可兼得，选择遵循 R5RS
报告（reader 折叠大小写，`string->symbol` 保持原样，故 pitfall 6.1
仍通过）。pitfall 与 examples 的白名单为空。

判定方式：chibi 套件不允许未在白名单内的 `[FAIL]` 行；pitfall 不允许
`Failure:` 行；examples 不允许 `FAIL:` 行；三者都要求出现完成标记
（"out of ... passed" / "Passed: 8.3"）且进程 exit 0。

## 真实程序实战（programs/）

`tests/r5rs_suites.rs` 的 `real_world_programs` 测试逐个运行
`programs/` 下的第三方程序（每个文件末尾有驱动段，输出可校验结果），
来源与许可证见 `programs/README.md`。全部 **13/13 通过**，且整个过程
没有发现解释器 bug（仅有的两处适配是程序用了非 R5RS 特性：`when`
宏 shim、SICP 的 `1+`/`-1+` 改名——后者本就不是合法 R5RS 标识符）。

| 程序 | 验证内容 | 预期 vs 实际 | 本地耗时 |
|---|---|---|---|
| `gabriel/tak.scm` | Takeuchi 函数（普通递归压力） | 7 = 7 | 0.13 s |
| `gabriel/cpstak.scm` | CPS tak（闭包 + 尾调用压力） | 7 = 7 | 0.18 s |
| `gabriel/ack.scm` | Ackermann(3,7) | 1021 = 1021 | 2.36 s |
| `gabriel/diviter.scm` | 10 万元素表折半 | 50000 = 50000 | 0.35 s |
| `gabriel/fibc.scm` | call/cc Peano 算术 fib(20) | 6765 = 6765 | 0.22 s |
| `gabriel/deriv.scm` | 符号求导 ×10000 | 与 Gabriel 标准答案逐字一致 | 0.76 s |
| `gabriel/destruc.scm` | set-car!/set-cdr! 破坏性操作 | 结果结构逐字一致 | 0.52 s |
| `gabriel/nqueens.scm` | 八皇后 | 92 = 92 | 0.12 s |
| `gabriel/puzzle.scm` | Baskett 拼图回溯 + call/cc 逃逸 | 2005 = 2005 | 3.37 s |
| `gabriel/mazefun.scm` | 纯函数迷宫构造（定种子） | 迷宫矩阵逐字一致 | 0.16 s |
| `gabriel/nboyer.scm` | **Boyer-Moore 定理证明基准**（合一、重写、循环结构术语） | **95024 = 95024 rewrites，精确命中文件记录值** | 3.38 s |
| `sicp/mceval.scm` | SICP 4.1 元循环求值器 | fact=3628800、fib=144、高阶 42、map=(1 4 9 16)、let=3、guest 尾递归 done | 2.61 s |
| `sicp/amb.scm` | SICP 4.3 amb 非确定性求值器 | (3 20)、(1 2 3 4)、(1 6)，回溯全部正确 | 0.02 s |

耗时为 2026-08-30 在 Apple M5 上的实测（debug build）；`triangl.scm`
结果正确但单次 ~53 s 超 CI 预算，未收录。

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

- `tests/r5rs_suites.rs`（4 个集成测试）：上述三套件 +
  `real_world_programs`（programs/ 下 13 个真实程序），均为子进程方式。
- `tests/scheme_units.rs`（27 个）：reader/printer 往返、精确算术与
  进制、`#` 占位数字、inexact 整函数、超越函数、5×10⁵ 尾递归与各
  尾位置、call/cc 多射重入、dynamic-wind 逃逸/重入、syntax-rules
  卫生（定义处解析、不捕获、嵌套/零次重复 ellipsis、`(... ...)`）、
  嵌套 quasiquote、等价谓词（含环形结构）、promise 缓存与重入、
  字符串端口、`values`、脚本未闭合报错（子进程）。
- `src/repl.rs` 内 `#[cfg(test)]`（3 个）：datum 完整性判断、补全
  词表生成、补全起点计算。

合计 34 个测试（27 + 3 + 4），CI 在 Ubuntu 与 macOS 双平台运行。

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

## criterion 基准

见 [benchmarks.md](benchmarks.md)（各用例测量内容、最新参考值、
测量环境与复现方法）。CI 的 bench job 只做记录（上传 artifact），
不设回归门禁。

