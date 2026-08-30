# tests/scm/programs/ — 真实 R5RS 程序实战验证

本目录收集真实存在的 Scheme 程序，用本仓库的解释器原样运行，验证实战
兼容性。每个文件末尾追加了 `;;; ===== driver ... =====` 标记的驱动段
（显示可校验的结果），该段不属于原程序。

`tests/r5rs_suites.rs` 的 `programs::*` 集成测试逐个运行这些
文件（进程内求值，string port 捕获输出）并断言关键输出。

## Gabriel 基准程序（tests/scm/programs/gabriel/）

来源：<https://github.com/ecraven/r7rs-benchmarks>（`src/` 目录的单
文件；源自 Dick Gabriel 的经典著作 *Performance and Evaluation of Lisp
Systems*，传统上为公有领域，nboyer.scm 文件头明确标注
"Status: Public Domain"）。

**适配方式**：只剥掉了文件头的 R7RS `(import ...)` 和末尾读取参数、
计时的 `(run-benchmark ...)` 包装，核心定义逐字保留；参数改小以在
几秒内跑完；个别文件用的非 R5RS 形式（`when`）在驱动段以宏 shim 适配
（未改解释器）。

| 文件 | 内容 | 驱动输出 | 预期 |
|---|---|---|---|
| `tak.scm` | Takeuchi 函数（普通递归调用压力） | `(tak 18 12 6)` | `7` |
| `cpstak.scm` | CPS 版 tak（闭包分配 + 尾调用压力） | `(cpstak 18 12 6)` | `7` |
| `ack.scm` | Ackermann 函数 | `(ack 3 7)` | `1021` |
| `diviter.scm` | 用表模拟除法（do 循环、cdr 链遍历） | `(length (iterative-div2 (create-n 100000)))` | `50000` |
| `fibc.scm` | 用 call/cc 做 Peano 算术的 fib | `(fibc 20 (lambda (n) n))` | `6765` |
| `deriv.scm` | 符号求导 ×10000（map、引用数据结构） | 求导结果 + `done` | 见测试断言 |
| `destruc.scm` | 破坏性表操作（set-car!/set-cdr!；含 `when` → shim） | `(destructive 600 50)` | 见测试断言 |
| `nqueens.scm` | 八皇后计数（含 `when` → shim） | `(nqueens 8)` | `92` |
| `puzzle.scm` | Baskett 拼图回溯搜索（向量 + call/cc 逃逸） | `(start 511)` | `2005` |
| `mazefun.scm` | 纯函数式迷宫构造（Marc Feeley；定种子故结果确定） | `(make-maze 11 11)` | 见测试断言 |
| `nboyer.scm` | **Boyer-Moore 风格定理证明基准**（最重的真实程序：合一、重写系统、property-list-free 符号记录、循环结构术语） | `(test-boyer alist term 0)` | `95024`（与文件中记录的 rewrite 数精确一致） |

`triangl.scm` 也在该套件中，但 `test(22,1)` 在本解释器上需 ~53s，
超出 CI 预算，故未收录。

## 大型程序（logic/ 与 sicp/ 新增）

| 文件 | 行数 | 内容 | 来源与许可证 | 适配 |
|---|---|---|---|---|
| `logic/schelog.scm` | 740 | **Schelog**：Dorai Sitaram 的 Prolog-in-Scheme 逻辑编程嵌入（合一、失败续延、cut、%bag-of；syntax-rules + call/cc 重度使用） | <https://github.com/ds26gte/schelog>（公有领域） | 原实现宏用非 R5RS 的 `datum->syntax` 过程式转换器。已在文件内移植为 syntax-rules（文件中有完整移植说明）：`%is` 改为运行时求值（`schelog:arith`）；anaphoric 的 cut `!` 改为显式传名（`%cut-delimiter ! goal`） |
| `sicp/regmach.scm` | 886 | **SICP 第 5 章**：寄存器机器模拟器（5.2）+ Scheme→寄存器机器编译器（5.5），含跑编译代码所需的第 4 章语法/环境子集 | SICP 文本（CC BY-SA 4.0），按书整理为单文件 | 无（纯 R5RS） |

驱动验证：
- schelog：家谱数据库 + grandparent 规则、%which/%more 回溯、schelog 自带的 %member/%append、%is 算术 ⇒ `((who joe))`、`((who alice))`、`#f`、`((x a/b/c))`、`((xs ...) (ys ...))` 两段、`((x 42))`
- regmach：5.1 手写 factorial 机器 ⇒ `120`；5.5 编译器编译 fib(10) 并在模拟器上执行 ⇒ `55`

## SICP（tests/scm/programs/sicp/）

来源：*Structure and Interpretation of Computer Programs*（Abelson &
Sussman），全书以 [CC BY-SA 4.0](https://creativecommons.org/licenses/by-sa/4.0/)
发布；此处按其文本整理为单文件 R5RS 版本。

| 文件 | 内容 | 驱动输出 |
|---|---|---|
| `mceval.scm` | SICP 4.1 元循环求值器：用 Scheme 写的 Scheme 解释器（环境模型、闭包、cond→if、let→lambda 脱糖） | `(fact 10)` ⇒ `3628800`、`(fib 12)` ⇒ `144`、高阶函数 `42`、`map` ⇒ `(1 4 9 16)`、`let` ⇒ `3`、guest 尾递归 `loop 5000` ⇒ `done`（guest 的尾递归由 host 的正确尾递归提供） |
| `amb.scm` | SICP 4.3 amb 非确定性求值器：CPS 化 analyze + 成功/失败续延回溯 | `prime-sum-pair` ⇒ `(3 20)`、穷举 `(amb 1 2 3 4)` ⇒ `(1 2 3 4)`、约束搜索 ⇒ `(1 6)` |

适配说明：SICP/MIT-Scheme 的 `1+`/`-1+` 不是合法 R5RS 标识符
（标识符不能以数字开头），mceval.scm 中改名 `inc`/`dec`。
