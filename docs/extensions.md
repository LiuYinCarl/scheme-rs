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

## 模块库（src/libs/*.scm）

借自 SRFI-1 / OCaml / Python / Ruby 标准库的常用工具，纯 R5RS 实现，
**运行时从磁盘加载**（不内嵌进二进制，改了 .scm 文件直接生效）。
搜索路径按优先级：可执行文件旁的 `lib/`（release 包布局）、当前目录
的 `lib/`、当前目录的 `src/libs/`（仓库内开发布局）。
**必须主动加载**：`(require '模块名)`，不加载就
不会占用全局环境里的任何名字，不会和你的同名定义冲突。

### `(require 'list)` — 列表工具

`iota` `filter` `fold` `fold-right` `reduce`（空表返回 ridentity）
`last` `take` `drop` `take-while` `drop-while` `find`（无则 #f）
`any` `every` `zip` `partition` `delete-duplicates` `sort`（稳定归并排序，
`(sort xs less?)`）

OCaml `List` 风格：`filter-map` `mapi` `iteri` `flatten` `init`
（`(init n f)` => `((f 0) … (f n-1))`）`split`（unzip）`rev-map`
`merge`（归并两个有序表）`for-all` `exists`（语义同 `every`/`any`）
`count`

```scheme
(require 'list)
(sort '(3 1 2) <)            ; => (1 2 3)
(reduce + 0 (iota 100))      ; => 4950
(partition odd? '(1 2 3 4))  ; => ((1 3) (2 4))
```

### `(require 'string)` — 字符串工具

`string-reverse` `string-repeat` `string-trim` `string-prefix?`
`string-suffix?` `string-contains?`（返回下标或 #f）`string-split`
`string-join` `string-replace`

OCaml `String` 风格：`string-upcase` `string-downcase` `string-capitalize`
`string-uncapitalize` `string-concat`（参数顺序同 OCaml）
`string-index`（返回下标或 #f）`string-map` `string-iteri` `string-fold`
`string-for-all` `string-exists`

```scheme
(require 'string)
(string-split "a,b,c" #\,)       ; => ("a" "b" "c")
(string-replace "a-b" "-" "+")   ; => "a+b"
```

### `(require 'option)` / `(require 'result)` — 可空值与错误值（OCaml 风格）

option：`none` 是符号 `none`，`(some v)` 构造 `(some v)`；
`some?` `none?` `option-map` `option-bind` `option-get`（none 时报错）
`option-get-or` `option-filter` `option-iter` `option->list`。

result：`(ok v)` / `(err e)`；`ok?` `err?` `result-map` `result-map-err`
`result-bind` `result-get` `result-get-err` `result-get-or`。

### `(require 'vector)` — 向量工具（OCaml Array 风格）

`vector-copy` `vector-map` `vector-mapi` `vector-for-each` `vector-iteri`
`vector-fold-left` `vector-fold-right` `vector-find`（返回下标或 #f）
`vector-for-all` `vector-exists` `vector-append`（可变参）`vector-reverse`
`vector-count` `vector-sort`（返回新向量，稳定归并排序）
`vector-binary-search`（有序向量，返回下标或 #f）

### `(require 'stream)` — 惰性流（OCaml Seq 风格，delay/force 实现）

流是一个 promise，求值得 `()` 或 `(头 . 流)`。`stream-null`
`stream-null?` `stream-cons`（宏，延迟尾流）`stream-car` `stream-cdr`
`list->stream` `stream->list`（仅有限流）`stream-take` `stream-drop`
`stream-map` `stream-filter` `stream-append` `stream-iterate`
`stream-unfold` `integers-from` `stream-range`（半开区间）
`stream-ref` `stream-for-each` `stream-fold`

```scheme
(require 'stream)
(stream-take (integers-from 0) 5)                 ; => (0 1 2 3 4)
(stream-take (stream-filter odd? (integers-from 0)) 4)  ; => (1 3 5 7)
```

### `(require 'map)` / `(require 'set)` — 有序映射与集合（OCaml Map/Set 风格）

不可变 AVL 树，键序由用户比较器 `(lt? a b)` 决定；修改操作纯函数式，
返回新结构（持久化）。map：`make-map` `map-add` `map-find`（未命中
返回 #f，有歧义时用 `map-member?` 区分）`map-member?` `map-remove`
`map-size` `map-keys` `map-values` `map->alist`（有序）`alist->map`
`map-fold`。set：`make-set` `set-add` `set-member?` `set-remove`
`set-size` `set->list` `list->set` `set-union` `set-intersection`
`set-difference` `set-fold` `set-subset?`（set 复用 map 的树实现，
二元运算要求两边比较器相同）。

```scheme
(require 'map)
(define m (map-add (map-add (make-map <) 2 'b) 1 'a))
(map->alist m)   ; => ((1 . a) (2 . b))
```

### `(require 'format)` / `(require 'buffer)` — 格式化与字符串缓冲

format（OCaml Printf / SRFI-28/48 子集）：`(sprintf fmt . args)`，指令
`~a`（display）`~s`（write）`~%`（换行）`~~`；`(format dest fmt . args)`
的 dest 为 `#t`（当前输出端口）/ 输出端口 / `#f`（同 sprintf）。

buffer（OCaml Buffer，字符串输出端口的薄封装）：`make-buffer`
`buffer-display` `buffer-write` `buffer-newline` `buffer-contents`
（行为同 `get-output-string`）`buffer-length`。

```scheme
(require 'format)
(sprintf "~a + ~a = ~s" 1 2 3)   ; => "1 + 2 = 3"
```

## REPL 专属（src/repl.rs）

只在交互层识别，不是求值器的特殊形式，脚本里写同名表达式不受影响：

- `(time expr)` — 求值 expr 并打印 `; time: X.XXX ms`
- `(load "path")` 成功时打印 `; loaded path`（嵌套 load 不打印）
- `(view)` — 高亮列出本会话的定义：直接输入的顶层 `define`/`define-syntax`
  和 `load` 文件里的定义带源码（重定义只保留最新版本）；`require` 加载的
  模块函数等未记录源码的绑定以名字列表附注
- `(view 'name)` — 只看某个定义；名字存在但未记录源码时给出提示
- `(view "path")` — 高亮查看文件。非 TTY 模式自动退化为无颜色输出
- `(unload "path")` — 回滚一次顶层 `load`：删除文件新定义的绑定、恢复被
  覆盖的旧绑定（变量与宏两张表都处理）。只回滚命名空间，文件里的副作用
  （`set!` 已有变量、I/O）不撤销；中途失败的 load 也可 unload 掉部分定义
- `(reload "path")` — 先回滚再重新 load（没有加载记录也能直接加载）
