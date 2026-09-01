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

## R5RS 之外的扩展

`runtime current-milliseconds random random-seed cd current-directory
file-exists? delete-file pretty-print trace untrace require` 与
`src/libs/` 模块库（list/string，需 `(require '名字)` 主动加载）
全部为新增名字，见 [extensions.md](extensions.md)。

## 有意的偏差与省略

| 项 | 说明 |
|---|---|
| **复数** | 有意省略（见 numeric-tower.md）。`complex?` 对实数答 `#t`，负数 `sqrt` 报错。 |
| **大小写** | reader 按 R5RS 第 2 章把标识符折叠为小写；`string->symbol` 保持原样。这符合报告，但与 chibi 套件冲突（该套件假设大小写敏感），`(symbol->string 'Martin)` 一例列入白名单——同一表达式两个套件期望相反，不可兼得。 |
| **指数标记** | 浮点指数只支持 `e/E`；R5RS 词法里的 `s f d l` 精度标记未支持。 |
| **`scheme-report-environment`/`null-environment`** | 返回完整交互环境，而非裁剪过的"只含报告绑定"环境。 |
| **`char-ready?`** | 恒 `#t`。 |
| **`with-input-from-file`/`with-output-to-file` 的重入** | 端口在动态逃逸/重入时通过 dynamic-wind 钩子正确恢复；但在其中捕获的续延被重入时，PortLeave 已把端口关闭，重入的动态范围里再写该端口会失败（R5RS 对此未作规定，本实现选择在逃逸时关闭）。 |
| **嵌套 `run()` 的续延截断** | quasiquote 的 unquote 求值与 `eval` 内建过程是递归调用求值器实现的；在其中捕获的续延不包含外层机器栈（R5RS 对 `eval` 内捕获续延本就接近未规定；quasiquote 内进行续延跳转的代码极为罕见）。 |
| **`case` 脱糖依赖 `memv`** | `case` 脱糖成 `(if (memv k '(datums...)) ...)`，若用户局部重定义了 `memv`，`case` 会用到它（理论上应引用定义处绑定；实际影响几乎为零）。 |
| **重复模式变量不查错** | 同一 pattern 里同一模式变量出现两次时后绑定覆盖先绑定（R5RS 说 an error，我们不做检查）。 |
| **同层单 ellipsis** | pattern 同一层出现两个 ellipsis（`x ... y ...`）只处理第一个。 |
| **宏生成宏的深度** | rename 锚定单层 def_env；极端合成宏没有完整语法对象系统的保证（已知局限，见 syntax-rules.md）。 |
| **报错信息** | 不含源码位置；`error` 过程打印参数后以非零退出/可捕获错误呈现。 |
| **gensym 可伪造** | 卫生靠"gensym 名字带空格、reader 读不出来"假设；但 `(string->symbol " if.3")` 走同一 intern 表可拿到同一个符号，理论上有意构造的用户代码可击穿卫生边界。 |
| **`GLOBAL_ENV` 是 ambient authority** | `load`、`(trace 'sym)`、environment specifier 读的都是"最后一次 `standard_env()` 创建的环境"。单 REPL/单测试下与实际运行环境一致；同线程创建第二个 env（嵌入宿主）时会静默错位。 |
| **thread_local 不随 `standard_env` 重置** | RNG 状态、trace 表、计时起点是线程本地全局量；测试/嵌入场景多次建 env 时它们保持延续，不复位。 |
| **RENAMES 表只增不减** | 每次宏展开登记一条 fresh→(orig, Rc<Env>)，长期 REPL 会话内存单调增长并 pin 住环境链。 |

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
