;;; scheme-rs 扩展 prelude：SRFI-1 常用子集（纯 R5RS 实现）
;;;
;;; 由 standard_env 在启动时自动加载（include_str! 内嵌，不依赖外部文件）。
;;; 只定义 R5RS 之外的新名字，不影响符合性套件。

;;; (iota count) 或 (iota count start step)：等差列表
(define (iota count . maybe)
  (let ((start (if (null? maybe) 0 (car maybe)))
        (step (if (or (null? maybe) (null? (cdr maybe))) 1 (cadr maybe))))
    (let loop ((i 0) (acc '()))
      (if (>= i count)
          (reverse acc)
          (loop (+ i 1) (cons (+ start (* i step)) acc))))))

;;; 保留满足 pred 的元素
(define (filter pred xs)
  (cond ((null? xs) '())
        ((pred (car xs)) (cons (car xs) (filter pred (cdr xs))))
        (else (filter pred (cdr xs)))))

;;; 左折叠：(f 元素 累积值)
(define (fold f init xs)
  (if (null? xs)
      init
      (fold f (f (car xs) init) (cdr xs))))

;;; 右折叠
(define (fold-right f init xs)
  (if (null? xs)
      init
      (f (car xs) (fold-right f init (cdr xs)))))

;;; 最后一个元素（xs 需为非空 proper list）
(define (last xs)
  (if (null? (cdr xs))
      (car xs)
      (last (cdr xs))))

;;; 前 n 个元素
(define (take xs n)
  (if (or (<= n 0) (null? xs))
      '()
      (cons (car xs) (take (cdr xs) (- n 1)))))

;;; 去掉前 n 个元素后的剩余列表
(define (drop xs n)
  (if (or (<= n 0) (null? xs))
      xs
      (drop (cdr xs) (- n 1))))

;;; 去重（保留首次出现，equal? 语义，member 判断）
(define (delete-duplicates xs)
  (cond ((null? xs) '())
        ((member (car xs) (cdr xs)) (delete-duplicates (cdr xs)))
        (else (cons (car xs) (delete-duplicates (cdr xs))))))
