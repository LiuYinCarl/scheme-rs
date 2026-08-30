# R5RS 符合性清单

本文列出 scheme-rs 对 R5RS 的覆盖情况、有意偏差与已知取舍。验证依据
是三套套件（见 [ testing.md ](testing.md)）：chibi R5RS 套件
188/189（1 例白名单）、SISC pitfall 全过、R5RS 报告示例 253/253。

## 完整支持的部分

- **第 4 章表达式**：全部原始与派生形式——`quote quasiquote（深度正确
  的嵌套）lambda（定参/rest/点对 rest）if（2/3 参）define（柯里糖衣、
  内部 define）set! cond（else/=>）case（含 R7RS 风格 =>）and or let
  let* letrec named-let begin do delay force define-syntax let-syntax
  letrec-syntax`。
- **4.3 syntax-rules**：字面量按绑定比较、`_`、点对、向量模式、嵌套
  ellipsis、`(... ...)` 转义、自定义省略号、重命名式卫生（R5RS 4.3
  示例与 pitfall 第 3 节全过）。
- **3.5/4.2.2 尾递归**：显式续延栈保证正确尾递归（5×10⁵ 深度单测）。
- **6.4 控制**：`call/cc`（multi-shot）、`values/call-with-values`
  （含多值续延）、`dynamic-wind`（重入正确）、`eval` 与三个环境说明符。
- **6.2 数字**：整数/有理数/实数，传染规则、进制与精确性前缀、`#`
  占位数字、`rationalize`、`sqrt` 精确平方根、超越函数
  （`exp log sin cos tan asin acos atan`）。
- **6.1 等价谓词**：`eq? eqv? equal?`（`equal?` 对环形 pair/vector
  按同构假设判定且保证终止）。
- **6.3 其余类型**：pair/list（全部 28 个 cxr、环形检测 `list?`、
  多表 map/for-each、apply）、symbol、char（含 ci 系列）、string
  （含 ci 系列、string-fill!、string-copy）、vector 全部过程。
- **6.6 I/O**：R5RS 全部端口过程，外加 chibi 测试 harness 需要的
  `open-input-string open-output-string get-output-string
  call-with-output-string flush-output error`。

## 有意的偏差与省略

| 项 | 说明 |
|---|---|
| **复数** | 有意省略（见 numeric-tower.md）。`complex?` 对实数答 `#t`，负数 `sqrt` 报错。 |
| **大小写** | reader 按 R5RS 第 2 章把标识符折叠为小写；`string->symbol` 保持原样。这符合报告，但与 chibi 套件冲突（该套件假设大小写敏感），`(symbol->string 'Martin)` 一例列入白名单——同一表达式两个套件期望相反，不可兼得。 |
| **指数标记** | 浮点指数只支持 `e/E`；R5RS 词法里的 `s f d l` 精度标记未支持。 |
| **`scheme-report-environment`/`null-environment`** | 返回完整交互环境，而非裁剪过的"只含报告绑定"环境。 |
| **`char-ready?`** | 恒 `#t`。 |
| **`with-input-from-file`/`with-output-to-file`** | 只在过程正常返回时恢复端口；被 call/cc 跳出时不恢复（R5RS 未明确规定，属于已知简化）。 |
| **嵌套 `run()` 的续延截断** | quasiquote 的 unquote 求值与 `eval` 内建过程是递归调用求值器实现的；在其中捕获的续延不包含外层机器栈（R5RS 对 `eval` 内捕获续延本就接近未规定；quasiquote 内进行续延跳转的代码极为罕见）。 |
| **`case` 脱糖依赖 `memv`** | `case` 脱糖成 `(if (memv k '(datums...)) ...)`，若用户局部重定义了 `memv`，`case` 会用到它（理论上应引用定义处绑定；实际影响几乎为零）。 |
| **重复模式变量不查错** | 同一 pattern 里同一模式变量出现两次时后绑定覆盖先绑定（R5RS 说 an error，我们不做检查）。 |
| **同层单 ellipsis** | pattern 同一层出现两个 ellipsis（`x ... y ...`）只处理第一个。 |
| **宏生成宏的深度** | rename 锚定单层 def_env；极端合成宏没有完整语法对象系统的保证（已知局限，见 syntax-rules.md）。 |
| **报错信息** | 不含源码位置；`error` 过程打印参数后以非零退出/可捕获错误呈现。 |

## 性能层面的已知取舍

- **每次闭包调用都重新扫描 body**（`prepare_body`，查内部 define 与
  宏展开）——每次调用有常数开销，换来实现简单与语义正确（内部
  define 的 letrec 语义）。可做展开缓存，暂未做。
- **参数求值帧逐元素克隆 Vec**（ArgDone），persistent 语义优先于
  分配效率；`map` 同理（call/cc 安全优先，pitfall 末尾的
  "call/cc safe, but probably not tail recursive" 信息即由此而来）。
- **BigInt 算术**：所有精确整数都是无界 BigInt，小整数也走堆分配
  ——正确性优先，未做小整数内联优化。
- **字符串按 UTF-8 Rust String 存储**，`string-ref`/`string-set!` 按
  字符索引是 O(n)。
- 当前基准见 [benchmarks.md](benchmarks.md)（fib(20) ≈ 29 ms、10 万尾
  递归 ≈ 213 ms），性能不是本项目目标，正确性与可读性优先。
