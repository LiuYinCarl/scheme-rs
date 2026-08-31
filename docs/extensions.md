# 扩展功能（非 R5RS）

本文列出 scheme-rs 在 R5RS 之外提供的实用扩展。这些名字全部是新增绑定，
不改变任何 R5RS 规定行为，符合性套件结果不受影响
（见 [r5rs-compliance.md](r5rs-compliance.md)）。

## 内建过程（src/builtins.rs）

| 过程 | 说明 |
|---|---|
| `(runtime)` | 解释器启动以来的毫秒数（精确整数）。SICP timed-prime-test 同款 |
| `(current-milliseconds)` | Unix epoch 以来的毫秒数（精确整数） |
| `(random)` | [0, 1) 区间的浮点随机数 |
| `(random n)` | [0, n) 区间的精确整数随机数，n 为正整数 |
| `(random-seed n)` | 设定随机种子（可复现序列）。PRNG 为 xorshift64*，无外部依赖 |
| `(cd path)` | 切换进程工作目录；之后 `load` 等的相对路径按新目录解析 |
| `(current-directory)` | 返回当前工作目录字符串 |
| `(file-exists? path)` | 路径是否存在 |
| `(delete-file path)` | 删除文件，失败报错 |
| `(pretty-print obj [port])` | 带换行缩进地打印嵌套结构（超 60 列自动展开），惯例同 `write` |
| `(trace 'f)` / `(trace f)` | 跟踪过程调用：入口打印 `(f 参数...)`，返回打印结果，`\|` 缩进表示嵌套深度。支持闭包与内建过程，符号参数在全局环境解析 |
| `(untrace 'f)` / `(untrace)` | 取消单个跟踪 / 清空全部跟踪 |

trace 输出示例：

```scheme
> (trace 'fib)
> (fib 3)
(fib 3)
| (fib 2)
| | (fib 1)
| | 1
| | (fib 0)
| | 0
| 1
| (fib 1)
| 1
2
```

## Prelude（src/prelude.scm）

SRFI-1 常用子集，纯 R5RS 实现，`include_str!` 内嵌、`standard_env`
启动时自动加载，REPL 与脚本模式都可用：

- `(iota count [start step])` — 等差列表
- `(filter pred xs)` — 保留满足条件的元素
- `(fold f init xs)` / `(fold-right f init xs)` — 左/右折叠
- `(last xs)` — 最后一个元素
- `(take xs n)` / `(drop xs n)` — 取/去前 n 个元素
- `(delete-duplicates xs)` — 去重（`member` 语义，保留首次出现）

## REPL 专属（src/repl.rs）

只在交互层识别，不是求值器的特殊形式，脚本里写同名表达式不受影响：

- `(time expr)` — 求值 expr 并打印 `; time: X.XXX ms`
- `(load "path")` 成功时打印 `; loaded path`（嵌套 load 不打印）
