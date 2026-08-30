# 总体架构：一台显式续延机

本文面向想学习解释器设计的读者，讲清 scheme-rs 的"为什么"。配套阅读：
`src/eval.rs` 顶部的模块注释。

## 模块划分与一次求值的旅程

```
源码文本
   │  src/reader.rs        read_datum / read_all_strict (reader.rs:79,282)
   ▼
Vec<Value>（datum：Pair/Symbol/Int/...，src/value.rs）
   │  src/eval.rs          eval_program (eval.rs:1433)
   │     ├─ 顶层 begin 拼接、宏展开（syntax_rules.rs）
   │     └─ run(State::Eval(form, env)) (eval.rs:250)
   ▼
trampoline 求值器 ──► Value
   │  src/printer.rs       write_to_string (printer.rs:39)
   ▼
输出文本
```

各模块职责：

| 模块 | 职责 |
|---|---|
| `value.rs` | Value 表示、符号 intern、gensym/rename 表（卫生）、`eq?/eqv?/equal?` |
| `reader.rs` | 字符流 → datum（全部 R5RS 词法） |
| `printer.rs` | datum → 文本（write/display，环形检测） |
| `env.rs` | 环境帧：符号 → location；宏命名空间；rename 感知解析 |
| `eval.rs` | trampoline 求值器、特殊形式、派生形式脱糖、quasiquote、body 扫描 |
| `syntax_rules.rs` | 宏的模式匹配与模板展开、卫生重命名 |
| `number.rs` | 数字塔（精确整数/有理数 + inexact f64） |
| `builtins.rs` | 全部内建过程，按名字分派 (builtins.rs:314) |
| `port.rs` | 端口抽象（stdin/文件/字符串端口） |
| `repl.rs` / `main.rs` | Jupyter 风格 REPL / CLI 入口 |

## trampoline：为什么不用 Rust 递归求值

R5RS 有两个硬性要求，决定了求值器的形态：

1. **正确的尾递归**：尾位置的过程调用必须在常数栈空间完成。
   如果 `eval` 用 Rust 函数递归实现，Scheme 每调用一次就消耗一层
   Rust 栈，`(let loop ((n 500000)) ...)` 这种深度直接爆栈。
2. **一等续延（call/cc）**：捕获续延必须便宜，且捕获后不可被后续
   求值破坏（要能多次重入）。

因此求值被压平成一个循环（`run`，src/eval.rs:250），机器状态只有三样：
当前 `State`、续延栈 `cont`、dynamic-wind 链 `winds`。

### State 状态机（src/eval.rs:207）

```rust
pub enum State {
    Eval(Value, Rc<Env>),   // 待求值的表达式 + 词法环境
    Return(Value),          // 值已算出，交付给栈顶续延帧
    Apply(Value, Vec<Value>),// 运算符与操作数都已就绪，执行应用
}
```

- **Eval**：`eval_step` 分派——符号查环境；自求值 datum 直接 `Return`；
  组合式先压 `OpDone` 帧再求值运算符（之后逐个压 `ArgDone` 帧求值操作数）。
- **Return**：弹出栈顶帧，把值交给 `resume`（src/eval.rs:330）——
  每种帧（If/Begin/ArgDone/Map/...）在这里决定下一步。
- **Apply**：进入它时，驱动参数求值的帧**已经被弹出**了，所以闭包体的
  求值直接发生在调用者的续延之下——这是尾调用不耗栈的关键
  （src/eval.rs:595 的 `Value::Closure` 分支）。

### 续延帧链表为什么是 persistent 的

续延栈是 `Option<Rc<ContFrame>>` 单链表（src/eval.rs:117 定义全部帧种类）。
resume 一个帧时**从不原地修改它**：需要更新状态时（比如 ArgDone 的
`collected` 多收了一个值），克隆数据、构造新帧压回栈上。

这个"不可变共享"性质换来两样东西：

- **捕获续延是 O(1)**：`ContObj { cont, winds }`（src/eval.rs:62）只是两个
  Rc 克隆。快照之后，无论求值怎么推进，旧栈永远不变。
- **multi-shot 天然成立**：同一个续延可以被调用任意多次、任意交错，
  因为恢复续延 = 把机器的 `cont` 指针换成快照里的那根，谁也不欠谁
  （pitfall 7.1–7.3 的交错重入就是这样过的）。

### 尾调用如何落在常数栈内

两个配合点：

1. `seq`（src/eval.rs:314）求值序列时，**最后一个形式不压 Begin 帧**，
   直接以当前续延 `State::Eval`——帧被"替换"而不是"叠加"。
   `if`/`cond`/`and`/`or`/`begin`/闭包体的尾位置同理（resume 里尾位置
   分支都是直接返回 `State::Eval`，不再压帧）。
2. 如上所述，`Apply` 闭包时不新增帧。

于是 `(let loop ((n 500000)) (if (= n 0) 'done (loop (- n 1))))` 每一轮
只是堆上分配几个短命的帧对象，Rust 栈始终是平的
（单元测试 `proper_tail_recursion` 验证）。

## call/cc：快照与换栈

`call/cc`（builtins.rs 分派）把当前 `m.cont` 与 `m.winds` 包成
`Value::Continuation(ContObj)` 传给用户过程——Rc 克隆，O(1)。

调用续延（src/eval.rs:595 的 `Value::Continuation` 分支）分两步：

1. 用 `wind_diff` 算出当前 wind 链到目标 wind 链的"离开/进入"序列，
   按序执行 after/before thunk（见下节）。
2. `m.cont = k.cont; m.winds = k.winds;` 整根栈换掉，`State::Return(v)`
   把值投递进快照里的世界。

注意调用续延**抛弃**了调用点的续延——这正是 escape procedure 的语义，
也是 `(+ 2 5 (k 3))` 得 3 而不是 8 的原因。

R5RS 6.4 还允许续延接受多个参数（交付为多值），所以
`(define (values . xs) (call/cc (lambda (k) (apply k xs))))` 这个报告自带
的 `values` 定义可以直接运行。

## dynamic-wind 与 common-tail 算法

每个 dynamic-wind 节点是 `{before, after}`，wind 链同样是 persistent
链表且每个节点记录深度（src/eval.rs 的 `WindNode`）。

`wind_diff`（src/eval.rs:95）的核心观察：两条 wind 链都从同一根
（`None`）长出来，必有**共享的尾部**（指针相等）。于是：

```
while a ≠ b（指针比较）:
    if depth(a) ≥ depth(b): afters.push(a.head);  a = a.parent
    else:                   befores.push(b.head); b = b.parent
befores.reverse()
```

- cur 侧弹出的按"由内到外"收集 after（先离开最内层）；
- tgt 侧弹出的反转后按"由外到内"收集 before（先进最外层）。

### 重入示例逐步推演（chibi 套件第 492 行用例）

```scheme
(define path '())
(define (add s) (set! path (cons s path)))
(dynamic-wind
  (lambda () (add 'connect))          ; before
  (lambda () (add (call/cc (lambda (c0) (set! c c0) 'talk1))))  ; body
  (lambda () (add 'disconnect)))      ; after
(if (< (length path) 4) (c 'talk2) (reverse path))
```

1. 进入 dynamic-wind：before 执行，path=(connect)；wind 链=[W]。
2. body 里 call/cc 捕获 c0：快照的 winds=[W]。add('talk1)。
3. body 返回，after 执行：path=(disconnect talk1 connect)；wind 链=[]。
4. `(c 'talk2)`：cur=[]，tgt=[W]。wind_diff：afters 空，befores=[W.before]。
   先跑 before → path 多了 connect；然后 `m.winds=[W]`，值 'talk2 投递回
   body 里的 add 调用 → add('talk2)。
5. body 再次返回 → DynWindBody 帧（在捕获的栈里）触发 after →
   path=(disconnect talk2 connect disconnect talk1 connect)。
6. `(reverse path)` ⇒ `(connect talk1 disconnect connect talk2 disconnect)`。

要点：before/after thunk 求值时，动态环境必须处于"已离开/未进入"的
中间状态，所以恢复续延时每个 thunk 都配上它该看到的 wind 链
（apply 的 Continuation 分支里的 `steps: Vec<(Value, WindList)>`）。

## 环境为什么存 location

`Env` 每帧是 `HashMap<Sym, Rc<RefCell<Value>>>`（src/env.rs:98 的
`lookup_var`）——存的是**盒子**而不是值。这是 letrec + call/cc 正确性
的关键。看 pitfall 1.1 的骨架：

```scheme
(letrec ((x (call/cc (lambda (c) (set! cont c) 0)))
         (y (call/cc (lambda (c) (set! cont c) 0))))
  (if cont
      (let ((c cont)) (set! cont #f) (set! x 1) (set! y 1) (c 0))
      (+ x y)))
```

初始化 x 时捕获的续延 c 包含"把初值写回 x、再算 y 的初始化、再跑
body"。之后 `(set! x 1)` 改了 x，再 `(c 0)` 重入：写回动作必须通过
**同一个 location** 生效，body 里的 `(+ x y)` 也必须读到后写的值。
如果环境存的是值（或续延帧里存的是值拷贝），重入者看到的就是旧世界。
存储模型上这与 R5RS 3.4 节"变量 denotes 一个 location"的语义一致。

配套的 letrec 编译方式（src/eval.rs:1137 `desugar_letrec`）也服务于
同一语义：所有 init 先求值到**临时变量**，再统一 `set!` 回各变量——
即"先求值完全部 init，再统一赋值"，这正是 R5RS 4.2.2 的措辞顺序，
也是 pitfall 1.1/1.2 通过的必要条件（顺序 set! 展开会给错答案）。
内部 define 走同一套语义（src/eval.rs:753 `kick_body` 的批量赋值）。

`resolve`（src/env.rs:134）在变量/宏两个命名空间逐帧查找，都没命中
时查 rename 表回退到宏定义处环境——这是宏卫生的解析一侧，详见
[ syntax-rules.md ](syntax-rules.md)。
