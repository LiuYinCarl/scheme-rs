;;; scheme-rs 扩展库 list 模块：SRFI-1 风格的列表工具
;;;
;;; 用法：(require 'list)
;;; 全部是 R5RS 之外的新名字；不 require 就不会出现在全局环境里，
;;; 不会和用户自己的同名定义混淆。

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

;;; reduce（SRFI-1 语义）：无初始值折叠；空表返回 ridentity
(define (reduce f ridentity xs)
  (if (null? xs)
      ridentity
      (fold f (car xs) (cdr xs))))

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

;;; 最长满足 pred 的前缀
(define (take-while pred xs)
  (if (and (pair? xs) (pred (car xs)))
      (cons (car xs) (take-while pred (cdr xs)))
      '()))

;;; 去掉最长满足 pred 的前缀
(define (drop-while pred xs)
  (if (and (pair? xs) (pred (car xs)))
      (drop-while pred (cdr xs))
      xs))

;;; 第一个满足 pred 的元素，没有则 #f
(define (find pred xs)
  (cond ((null? xs) #f)
        ((pred (car xs)) (car xs))
        (else (find pred (cdr xs)))))

;;; 任一/全部元素满足 pred
(define (any pred xs)
  (cond ((null? xs) #f)
        ((pred (car xs)) #t)
        (else (any pred (cdr xs)))))

(define (every pred xs)
  (cond ((null? xs) #t)
        ((pred (car xs)) (every pred (cdr xs)))
        (else #f)))

;;; 把若干列表按位置配对成列表的列表：(zip '(a b) '(1 2)) => ((a 1) (b 2))
(define (zip . lists)
  (if (any null? lists)
      '()
      (cons (map car lists) (apply zip (map cdr lists)))))

;;; 按 pred 一分为二：(partition odd? '(1 2 3)) => ((1 3) (2))
(define (partition pred xs)
  (cond ((null? xs) (list '() '()))
        ((pred (car xs))
         (let ((r (partition pred (cdr xs))))
           (list (cons (car xs) (car r)) (cadr r))))
        (else
         (let ((r (partition pred (cdr xs))))
           (list (car r) (cons (car xs) (cadr r)))))))

;;; 去重（保留首次出现，member 语义）
(define (delete-duplicates xs)
  (cond ((null? xs) '())
        ((member (car xs) (cdr xs)) (delete-duplicates (cdr xs)))
        (else (cons (car xs) (delete-duplicates (cdr xs))))))

;;; 稳定归并排序：(sort '(3 1 2) <) => (1 2 3)
(define (sort xs less?)
  (define (merge a b)
    (cond ((null? a) b)
          ((null? b) a)
          ((less? (car b) (car a)) (cons (car b) (merge a (cdr b))))
          (else (cons (car a) (merge (cdr a) b)))))
  (define (halve xs acc n)
    (if (<= n 0)
        (cons acc xs)
        (halve (cdr xs) (cons (car xs) acc) (- n 1))))
  (let ((len (length xs)))
    (if (<= len 1)
        xs
        (let* ((h (halve xs '() (quotient len 2)))
               (left (reverse (car h)))
               (right (cdr h)))
          (merge (sort left less?) (sort right less?))))))
