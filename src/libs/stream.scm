;;; scheme-rs 扩展库 stream 模块：OCaml Seq 风格的惰性流
;;;
;;; 用法：(require 'stream)
;;; 流是一个 promise，force 后得到 ()（空流）或 (head . stream) 序对。
;;; 基于 R5RS 的 delay/force 实现；除 stream-null? / stream-car / stream-cdr
;;; 等必须观察元素的接口外，其余操作都不会提前求值尾部，因此可以表示
;;; 无限流（如 integers-from）。注意 stream->list / stream-fold 只适用于
;;; 有限流，对无限流调用会发散（永不返回）。

;;; 空流
(define stream-null (delay '()))

;;; 判空
(define (stream-null? s)
  (null? (force s)))

;;; 构造流：尾部的求值被 delay 推迟到 stream-cdr 之后
(define-syntax stream-cons
  (syntax-rules ()
    ((stream-cons head tail) (delay (cons head tail)))))

;;; 首元素（s 需非空）
(define (stream-car s)
  (car (force s)))

;;; 剩余流（s 需非空）
(define (stream-cdr s)
  (cdr (force s)))

;;; 列表转流
(define (list->stream xs)
  (if (null? xs)
      stream-null
      (stream-cons (car xs) (list->stream (cdr xs)))))

;;; 流转列表；仅适用于有限流，对无限流会发散
(define (stream->list s)
  (if (stream-null? s)
      '()
      (cons (stream-car s) (stream->list (stream-cdr s)))))

;;; 取前 n 个元素组成列表（n 超过长度则取到流尾）
(define (stream-take s n)
  (if (or (<= n 0) (stream-null? s))
      '()
      (cons (stream-car s) (stream-take (stream-cdr s) (- n 1)))))

;;; 去掉前 n 个元素后的剩余流
(define (stream-drop s n)
  (if (or (<= n 0) (stream-null? s))
      s
      (stream-drop (stream-cdr s) (- n 1))))

;;; 映射
(define (stream-map f s)
  (if (stream-null? s)
      stream-null
      (stream-cons (f (stream-car s)) (stream-map f (stream-cdr s)))))

;;; 过滤（惰性：遇到下一个满足 pred 的元素才继续扫描）
(define (stream-filter pred s)
  (cond ((stream-null? s) stream-null)
        ((pred (stream-car s))
         (stream-cons (stream-car s) (stream-filter pred (stream-cdr s))))
        (else (stream-filter pred (stream-cdr s)))))

;;; 拼接：先流尽 s1 再接 s2（s1 为无限流时 s2 不可达）
(define (stream-append s1 s2)
  (if (stream-null? s1)
      s2
      (stream-cons (stream-car s1) (stream-append (stream-cdr s1) s2))))

;;; 无限流：x, f(x), f(f(x)), ...
(define (stream-iterate f x)
  (stream-cons x (stream-iterate f (f x))))

;;; 展开：当 (p seed) 成立时发出 (f seed)，并令 seed = (g seed)
(define (stream-unfold p f g seed)
  (if (p seed)
      (stream-cons (f seed) (stream-unfold p f g (g seed)))
      stream-null))

;;; 无限流：n, n+1, n+2, ...
(define (integers-from n)
  (stream-cons n (integers-from (+ n 1))))

;;; 有限流：lo, lo+1, ..., hi-1（左闭右开）
(define (stream-range lo hi)
  (if (>= lo hi)
      stream-null
      (stream-cons lo (stream-range (+ lo 1) hi))))

;;; 第 n 个元素（从 0 计）
(define (stream-ref s n)
  (if (<= n 0)
      (stream-car s)
      (stream-ref (stream-cdr s) (- n 1))))

;;; 遍历（仅副作用；对无限流不返回）
(define (stream-for-each f s)
  (if (stream-null? s)
      (if #f #f)
      (begin
        (f (stream-car s))
        (stream-for-each f (stream-cdr s)))))

;;; 左折叠：(f 元素 累积值)；仅适用于有限流
(define (stream-fold f init s)
  (if (stream-null? s)
      init
      (stream-fold f (f (stream-car s) init) (stream-cdr s))))
