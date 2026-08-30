# 性能专题

本文收集 scheme-rs 的性能数据、测量方法与已知取舍。先说结论性的提醒：
**这是一个以正确性与可读性为目标的 tree-walking 教学解释器，不是为
速度设计的**——数字用来说明"代价在哪里"，而不是参与竞速。

## 测量环境

- 机器：Apple M5（arm64），24 GB 内存，macOS 26.6.2
- 工具链：rustc/cargo 1.98（stable），`--release`（criterion bench profile）
- 日期：2026-08-30

## criterion 基准（benches/interpreter.rs）

运行方式（必须带 `--bench interpreter`，否则参数会传给默认 test
harness 而报错）：

```
cargo bench --bench interpreter -- \
  --warm-up-time 1 --measurement-time 3 --sample-size 10
```

每个用例在全新的全局环境里求值一段 Scheme 程序（reader 用例除外）：

| 用例 | 测量内容 | 参考值（mean） |
|---|---|---|
| `fib_recursion_20` | `(fib 20)` 普通递归：帧分配 + 环境创建 + BigInt 加法，约 2.2 万次调用 | **29.3 ms**（≈ 0.75M 调用/s） |
| `tail_loop_100k` | 10 万次 named-let 尾调用：TCO 路径稳态成本；同时证明常数栈（否则会爆栈） | **212.8 ms**（≈ 0.47M 迭代/s） |
| `map_over_1000` | 内建 map 的帧化迭代 + 每元素一次闭包调用 | **4.9 ms** |
| `string_and_number_mix` | 200 次 `number->string`（BigInt→十进制）+ `string-append` | **661 µs** |
| `reader_r5rs_tests_scm` | reader 把整个 chibi 套件（~10 KB、524 行）读成 datum | **231 µs** |

历史对照（首次接入 CI 时的参考值，同机 2026-08-30 早些时候）：
fib 30.1 ms / tail 214 ms / map 4.6 ms / mix 790 µs / reader 246 µs——
与现值一致，性能无回归。

## 实战程序耗时（programs/）

本地实测（`/usr/bin/time`，同一台 M5，debug build 的解释器二进制——
即 `cargo build` 默认产物；release 构建会更快）：

| 程序 | 验证内容 | 结果 | 耗时 |
|---|---|---|---|
| `gabriel/tak.scm` | tak(18,12,6) | 7 | 0.13 s |
| `gabriel/cpstak.scm` | CPS 版 tak | 7 | 0.18 s |
| `gabriel/ack.scm` | ack(3,7) | 1021 | 2.36 s |
| `gabriel/diviter.scm` | 10 万元素表折半 | 50000 | 0.35 s |
| `gabriel/fibc.scm` | call/cc Peano fib(20) | 6765 | 0.22 s |
| `gabriel/deriv.scm` | 符号求导 ×10000 | 与标准答案一致 | 0.76 s |
| `gabriel/destruc.scm` | 破坏性表操作 | 结构一致 | 0.52 s |
| `gabriel/nqueens.scm` | 八皇后计数 | 92 | 0.12 s |
| `gabriel/puzzle.scm` | Baskett 拼图回溯 + call/cc | 2005 | 3.37 s |
| `gabriel/mazefun.scm` | 纯函数迷宫构造 | 矩阵一致 | 0.16 s |
| **`gabriel/nboyer.scm`** | **Boyer-Moore 定理证明基准** | **95024 rewrites（精确命中）** | **3.38 s** |
| `sicp/mceval.scm` | SICP 4.1 元循环求值器（fact/fib/高阶/map/let/guest 尾递归） | 全部正确 | 2.61 s |
| `sicp/amb.scm` | SICP 4.3 amb 非确定性求值器（回溯搜索） | 全部正确 | 0.02 s |

亮点：**nboyer(0) 以 ≈ 28k rewrites/s 的速度精确命中 95024 次重写**。
这个基准做大量合一匹配、符号记录查找与 cons 分配，是解释器在真实
工作负载下最有代表性的数字。

## 代价在哪里（已知取舍）

详见 [r5rs-compliance.md](r5rs-compliance.md) 的性能节，摘要：

- **每次闭包调用都重新扫描 body**（内部 define/宏检查）——正确性
  优先的常数开销，fib/tail 用例里占大头。
- **参数求值帧逐元素克隆 Vec**：persistent 续延语义（call/cc 多射
  重入）优先于分配效率。
- **所有精确整数都是 BigInt**：小整数也走堆分配；没有小整数内联。
- **字符串按字符索引 O(n)**：`string-ref`/`string-set!` 如此。
- 求值器是 tree-walking：没有编译、没有内联缓存、没有 JIT。

## 复现

```
cargo bench --bench interpreter          # criterion 基准（默认时长，较慢）
cargo bench --bench interpreter -- \
  --warm-up-time 1 --measurement-time 3 --sample-size 10   # CI 同款快速档
cargo build && time ./target/debug/scheme-rs programs/gabriel/nboyer.scm
```

CI 的 bench job 只做记录（artifact），不设回归门禁。
