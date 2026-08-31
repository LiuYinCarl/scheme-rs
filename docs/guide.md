# scheme-rs 使用指南

R5RS Scheme 解释器。启动：`cargo run` 进入 REPL，`cargo run -- file.scm` 执行文件。

REPL 特性：`In [n]:` 编号提示符、多行编辑（括号未闭合回车自动续行，可
跨行修改，历史整体召回）、Tab 补全、语法高亮
（`--no-highlight` 关闭）、实时错误提示（词法错误与顶层未绑定符号以
暗灰色文字显示在光标后，`--no-hint` 关闭）、历史记录持久化、
`(exit)` 或 Ctrl-D 退出、Ctrl-C 丢弃当前输入。

## 特殊形式

`quote 'x` `quasiquote \`x`（`,` `,@`）`lambda` `define` `set!` `if`
`begin` `let` `let*` `letrec`（含 named-let）`cond`（`else` `=>`）
`case`（含 `=>`）`and` `or` `do` `delay`/`force` `define-syntax`
`let-syntax` `letrec-syntax`（完整 syntax-rules，卫生宏）

```scheme
(define (fact n) (if (= n 0) 1 (* n (fact (- n 1)))))
(let loop ((i 0) (acc '())) (if (= i 5) (reverse acc) (loop (+ i 1) (cons i acc))))
(define-syntax swap! (syntax-rules () ((_ a b) (let ((t a)) (set! a b) (set! b t)))))
```

## 数字

精确整数（无界 BigInt）/ 精确分数 / 浮点，自动传染；进制与精确性前缀
`#b #o #d #x #e #i` 可组合。

`+ - * /` `=` `<` `>` `<=` `>=` `quotient remainder modulo` `gcd lcm`
`numerator denominator` `floor ceiling truncate round rationalize`
`expt sqrt abs max min` `exact->inexact inexact->exact`
`number->string string->number`（支持 radix）`zero? positive? negative?`
`odd? even? exact? inexact? number? complex? real? rational? integer?`
`exp log sin cos tan asin acos atan`

```scheme
(/ 6 4)        ; => 3/2      精确分数
(+ 1/3 0.5)    ; => 0.833…   精确遇浮点则浮点
#e1.5          ; => 3/2      #e 强制精确
(sqrt 16)      ; => 4        完全平方得精确整数
(number->string 255 16)       ; => "ff"
```

## 等价谓词

`eq?`（同一对象）`eqv?`（值等价，数字/字符比较值）`equal?`（递归比较，
环形结构安全）

```scheme
(eqv? 1.0 1.0)         ; => #t
(equal? '(1 (2)) '(1 (2)))  ; => #t
```

## 点对与列表

`cons car cdr` `set-car! set-cdr!` `caar`…`cddddr`（全部 28 个）
`null? pair? list?`（环形安全）`list length append reverse`
`list-tail list-ref` `memq memv member` `assq assv assoc`

```scheme
(append '(1 2) '(3))   ; => (1 2 3)
(assq 'b '((a . 1) (b . 2)))  ; => (b . 2)
```

## 符号 / 字符 / 字符串 / 向量

`symbol? symbol->string string->symbol`

`char? char=? char<? …`（含 `char-ci=` 系列）`char-alphabetic?`
`char-numeric? char-whitespace? char-upper-case? char-lower-case?`
`char->integer integer->char char-upcase char-downcase`

`string? make-string string string-length string-ref string-set!`
`string=? string<? …`（含 `-ci` 系列）`substring string-append`
`string->list list->string string-copy string-fill!`

`vector? make-vector vector vector-length vector-ref vector-set!`
`vector->list list->vector vector-fill!`

```scheme
(string-append "hi" "-" "there")  ; => "hi-there"
(vector-ref #(a b c) 1)           ; => b
```

## 控制

`procedure? apply map for-each`（多表、call/cc 安全）
`call/cc call-with-current-continuation`（多射续延）
`values call-with-values` `dynamic-wind` `eval`
`scheme-report-environment null-environment interaction-environment`

```scheme
(map + '(1 2) '(10 20))                 ; => (11 22)
(call/cc (lambda (k) (+ 1 (k 42))))     ; => 42
(call-with-values (lambda () (values 1 2)) +)  ; => 3
```

## I/O

`input-port? output-port? current-input-port current-output-port`
`open-input-file open-output-file close-input-port close-output-port`
`read read-char peek-char eof-object? char-ready?`
`write display newline write-char flush-output` `load`
`call-with-input-file call-with-output-file`
`with-input-from-file with-output-to-file`
`open-input-string open-output-string get-output-string call-with-output-string`
`error`

```scheme
(call-with-output-string (lambda (p) (write '(1 "a") p)))  ; => "(1 \"a\")"
(load "util.scm")   ; REPL 中成功会打印 ; loaded util.scm
```

## 扩展（非 R5RS，详见 extensions.md）

`runtime` `current-milliseconds` `random random-seed`
`cd current-directory` `file-exists? delete-file`
`pretty-print` `trace untrace`

```scheme
(random-seed 42) (random 100)   ; 可复现随机数
(trace 'fact) (fact 3) (untrace 'fact)   ; 打印调用轨迹
```

模块库（需主动加载，见 extensions.md）：

- `(require 'list)` — `iota filter fold fold-right reduce last take drop
  take-while drop-while find any every zip partition delete-duplicates sort`
- `(require 'string)` — `string-reverse string-repeat string-trim
  string-prefix? string-suffix? string-contains? string-split string-join
  string-replace`

```scheme
(require 'list)
(sort '(3 1 2) <)           ; => (1 2 3)
(reduce + 0 (iota 100))     ; => 4950
```

## REPL 专属

- `(time expr)` — 计时并打印 `; time: X.XXX ms`
- `(load "path")` — 成功打印 `; loaded path`
- `(unload "path")` / `(reload "path")` — 回滚/重载一次 load（只回滚命名
  空间绑定，文件副作用不撤销）
- `(view)` — 高亮显示本会话所有求值成功的顶层定义
- `(view 'name)` — 只看某个定义（含重定义历史中的最新版本）
- `(view "path")` — 高亮显示文件内容
- `(exit)` — 退出

## 杂项

`not boolean?`

## 已知限制（有意）

复数省略（负数 `sqrt` 报错）；`char-ready?` 恒 `#t`；报错不含源码位置。
完整清单见 [r5rs-compliance.md](r5rs-compliance.md)。
