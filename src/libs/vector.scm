;;; scheme-rs 扩展库 vector 模块：OCaml Array 风格的向量工具
;;;
;;; 用法：(require 'vector)
;;; 基于 R5RS 原生 vector；除 vector-for-each/vector-iteri 外的变换类
;;; 函数都返回新向量，不改动入参。vector-sort 经列表做稳定归并排序，
;;; 自包含实现，不依赖其他扩展模块。

;;; (vector-copy v)：浅拷贝（R5RS 没有提供）
(define (vector-copy v)
  (let* ((n (vector-length v))
         (r (make-vector n)))
    (let loop ((i 0))
      (if (< i n)
          (begin (vector-set! r i (vector-ref v i))
                 (loop (+ i 1)))
          r))))

;;; (vector-map f v)：逐元素映射成新向量
(define (vector-map f v)
  (let* ((n (vector-length v))
         (r (make-vector n)))
    (let loop ((i 0))
      (if (< i n)
          (begin (vector-set! r i (f (vector-ref v i)))
                 (loop (+ i 1)))
          r))))

;;; (vector-mapi f v)：f 接收下标和元素，映射成新向量
(define (vector-mapi f v)
  (let* ((n (vector-length v))
         (r (make-vector n)))
    (let loop ((i 0))
      (if (< i n)
          (begin (vector-set! r i (f i (vector-ref v i)))
                 (loop (+ i 1)))
          r))))

;;; (vector-for-each f v)：按序对每个元素求值 (f 元素)，仅看副作用
(define (vector-for-each f v)
  (let ((n (vector-length v)))
    (let loop ((i 0))
      (if (< i n)
          (begin (f (vector-ref v i))
                 (loop (+ i 1)))
          #t))))

;;; (vector-iteri f v)：同 vector-for-each，但 f 接收下标和元素
(define (vector-iteri f v)
  (let ((n (vector-length v)))
    (let loop ((i 0))
      (if (< i n)
          (begin (f i (vector-ref v i))
                 (loop (+ i 1)))
          #t))))

;;; 左折叠：(vector-fold-left f init v) 中 f 接收 (累积值 元素)
(define (vector-fold-left f init v)
  (let ((n (vector-length v)))
    (let loop ((i 0) (acc init))
      (if (< i n)
          (loop (+ i 1) (f acc (vector-ref v i)))
          acc))))

;;; 右折叠：(vector-fold-right f init v) 中 f 接收 (元素 累积值)
(define (vector-fold-right f init v)
  (let loop ((i (- (vector-length v) 1)) (acc init))
    (if (>= i 0)
        (loop (- i 1) (f (vector-ref v i) acc))
        acc)))

;;; 第一个满足 pred 的元素的下标，没有则 #f
(define (vector-find pred v)
  (let ((n (vector-length v)))
    (let loop ((i 0))
      (cond ((>= i n) #f)
            ((pred (vector-ref v i)) i)
            (else (loop (+ i 1)))))))

;;; 全部/任一元素满足 pred
(define (vector-for-all pred v)
  (let ((n (vector-length v)))
    (let loop ((i 0))
      (cond ((>= i n) #t)
            ((pred (vector-ref v i)) (loop (+ i 1)))
            (else #f)))))

(define (vector-exists pred v)
  (let ((n (vector-length v)))
    (let loop ((i 0))
      (cond ((>= i n) #f)
            ((pred (vector-ref v i)) #t)
            (else (loop (+ i 1)))))))

;;; 把若干向量按序拼成一个新向量：(vector-append '#(1) '#(2 3)) => #(1 2 3)
(define (vector-append . vs)
  (let ((total (apply + (map vector-length vs))))
    (let ((r (make-vector total)))
      (let outer ((rest vs) (i 0))
        (if (null? rest)
            r
            (let ((v (car rest)))
              (let inner ((j 0) (i i))
                (if (< j (vector-length v))
                    (begin (vector-set! r i (vector-ref v j))
                           (inner (+ j 1) (+ i 1)))
                    (outer (cdr rest) i)))))))))

;;; 逆序新向量
(define (vector-reverse v)
  (let* ((n (vector-length v))
         (r (make-vector n)))
    (let loop ((i 0))
      (if (< i n)
          (begin (vector-set! r i (vector-ref v (- n 1 i)))
                 (loop (+ i 1)))
          r))))

;;; 满足 pred 的元素个数
(define (vector-count pred v)
  (vector-fold-left (lambda (acc x) (if (pred x) (+ acc 1) acc)) 0 v))

;;; 稳定归并排序，返回新向量（原向量不变）：(vector-sort < '#(3 1 2)) => #(1 2 3)
;;; lt? 是严格小于比较器：(lt? a b) 为真表示 a 排在 b 前
(define (vector-sort lt? v)
  (define (merge a b)
    (cond ((null? a) b)
          ((null? b) a)
          ((lt? (car b) (car a)) (cons (car b) (merge a (cdr b))))
          (else (cons (car a) (merge (cdr a) b)))))
  (define (halve xs acc n)
    (if (<= n 0)
        (cons acc xs)
        (halve (cdr xs) (cons (car xs) acc) (- n 1))))
  (define (sort-list xs)
    (let ((len (length xs)))
      (if (<= len 1)
          xs
          (let* ((h (halve xs '() (quotient len 2)))
                 (left (reverse (car h)))
                 (right (cdr h)))
            (merge (sort-list left) (sort-list right))))))
  (list->vector (sort-list (vector->list v))))

;;; 二分查找：v 需已按 lt? 升序排列；命中返回下标，否则 #f
(define (vector-binary-search lt? v x)
  (let loop ((lo 0) (hi (- (vector-length v) 1)))
    (if (> lo hi)
        #f
        (let* ((mid (quotient (+ lo hi) 2))
               (m (vector-ref v mid)))
          (cond ((lt? m x) (loop (+ mid 1) hi))
                ((lt? x m) (loop lo (- mid 1)))
                (else mid))))))
