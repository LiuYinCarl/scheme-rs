;;; scheme-rs 扩展库 set 模块：OCaml Set 风格的不可变有序集合
;;;
;;; 用法：(require 'set)（会自动 require 'map）
;;; 元素的顺序由用户提供的比较器 (lt? a b) 决定（严格小于，类似 <）。
;;; 直接复用 map 模块的 AVL 树：集合就是"值恒为 #t 的 map"，
;;; 因此同样满足纯函数式 / 持久化语义，修改操作返回新集合。
;;;
;;; 注意：set-union / set-intersection / set-difference / set-subset?
;;; 假定两个集合使用相同的比较器（行为未做检查）。

(require 'map)

;;; (make-set lt?)：创建空集合，lt? 是元素的严格小于比较器
(define (make-set lt?)
  (make-map lt?))

;;; 取集合内部的比较器（内部辅助）
(define (set-lt? s) (car s))

;;; 加入元素，返回新集合（原集合不变）
(define (set-add s x)
  (map-add s x #t))

;;; 元素是否存在
(define (set-member? s x)
  (map-member? s x))

;;; 删除元素，返回新集合
(define (set-remove s x)
  (map-remove s x))

;;; 元素个数
(define (set-size s)
  (map-size s))

;;; 升序元素列表
(define (set->list s)
  (map-keys s))

;;; 由列表构建集合；重复元素自然去重
(define (list->set lt? xs)
  (let loop ((xs xs) (s (make-set lt?)))
    (if (null? xs)
        s
        (loop (cdr xs) (set-add s (car xs))))))

;;; 按元素升序折叠：(f 元素 累积值)
(define (set-fold f init s)
  (map-fold (lambda (k v acc) (f k acc)) init s))

;;; 并集（假定两集合比较器相同）
(define (set-union a b)
  (set-fold (lambda (x acc) (set-add acc x)) a b))

;;; 交集（结果沿用 a 的比较器）
(define (set-intersection a b)
  (set-fold (lambda (x acc)
              (if (set-member? b x) (set-add acc x) acc))
            (make-set (set-lt? a))
            a))

;;; 差集：a 中不属于 b 的元素
(define (set-difference a b)
  (set-fold (lambda (x acc)
              (if (set-member? b x) acc (set-add acc x)))
            (make-set (set-lt? a))
            a))

;;; a 是否为 b 的子集
(define (set-subset? a b)
  (set-fold (lambda (x acc) (and acc (set-member? b x))) #t a))
