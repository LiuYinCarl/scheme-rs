;;; scheme-rs 扩展库 list 模块：SRFI-1 风格的列表工具
;;;
;;; 用法：(require 'list)
;;; 全部是 R5RS 之外的新名字；不 require 就不会出现在全局环境里，
;;; 不会和用户自己的同名定义混淆。
;;;
;;; 另含 OCaml List 风格函数：filter-map mapi iteri flatten init split
;;; rev-map merge for-all exists count（for-all/exists 语义同 any/every）

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

;;; map 并丢弃 #f 结果
(define (filter-map f xs)
  (if (null? xs)
      '()
      (let ((v (f (car xs))))
        (if v
            (cons v (filter-map f (cdr xs)))
            (filter-map f (cdr xs))))))

;;; 带下标的 map：f 接收下标和元素
(define (mapi f xs)
  (let loop ((i 0) (xs xs))
    (if (null? xs)
        '()
        (cons (f i (car xs)) (loop (+ i 1) (cdr xs))))))

;;; 带下标的遍历（仅副作用），返回 ()
(define (iteri f xs)
  (let loop ((i 0) (xs xs))
    (if (null? xs)
        '()
        (begin (f i (car xs)) (loop (+ i 1) (cdr xs))))))

;;; 拼接列表的列表
(define (flatten xss)
  (if (null? xss)
      '()
      (append (car xss) (flatten (cdr xss)))))

;;; (init n f) => ((f 0) (f 1) ... (f n-1))，n <= 0 时为空表
(define (init n f)
  (let loop ((i 0) (acc '()))
    (if (>= i n)
        (reverse acc)
        (loop (+ i 1) (cons (f i) acc)))))

;;; 二元组列表解 zip：(split '((a 1) (b 2))) => ((a b) (1 2))
(define (split pairs)
  (if (null? pairs)
      (list '() '())
      (let ((r (split (cdr pairs))))
        (list (cons (caar pairs) (car r))
              (cons (cadar pairs) (cadr r))))))

;;; 逆序 map：(rev-map f '(1 2 3)) => ((f 3) (f 2) (f 1))
(define (rev-map f xs)
  (let loop ((xs xs) (acc '()))
    (if (null? xs)
        acc
        (loop (cdr xs) (cons (f (car xs)) acc)))))

;;; 归并两个已按 lt? 排好序的列表（稳定）
(define (merge lt? xs ys)
  (cond ((null? xs) ys)
        ((null? ys) xs)
        ((lt? (car ys) (car xs)) (cons (car ys) (merge lt? xs (cdr ys))))
        (else (cons (car xs) (merge lt? (cdr xs) ys)))))

;;; 全部/任一元素满足 pred（OCaml 命名，语义同 every/any）
(define (for-all pred xs)
  (cond ((null? xs) #t)
        ((pred (car xs)) (for-all pred (cdr xs)))
        (else #f)))

(define (exists pred xs)
  (cond ((null? xs) #f)
        ((pred (car xs)) #t)
        (else (exists pred (cdr xs)))))

;;; 满足 pred 的元素个数
(define (count pred xs)
  (let loop ((xs xs) (n 0))
    (cond ((null? xs) n)
          ((pred (car xs)) (loop (cdr xs) (+ n 1)))
          (else (loop (cdr xs) n)))))
