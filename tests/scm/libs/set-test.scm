;;; set 模块测试：基于 map AVL 树的不可变有序集合
(require 'set)

;;; 生成 [lo, hi] 的升序整数列表（测试辅助）
(define (range lo hi)
  (if (> lo hi)
      '()
      (cons lo (range (+ lo 1) hi))))

;;; --- 空集合 ---
(check 0 (set-size (make-set <)))
(check #f (set-member? (make-set <) 1))
(check '() (set->list (make-set <)))

;;; --- 升序 / 降序 / 乱序插入都得到有序集合（旋转场景）---
(define s-asc (list->set < '(1 2 3 4 5 6 7)))
(define s-desc (list->set < '(7 6 5 4 3 2 1)))
(define s-zig (list->set < '(4 2 6 1 3 5 7)))
(check '(1 2 3 4 5 6 7) (set->list s-asc))
(check '(1 2 3 4 5 6 7) (set->list s-desc))
(check '(1 2 3 4 5 6 7) (set->list s-zig))
(check 7 (set-size s-zig))

;;; --- 去重与成员判断 ---
(define s-dup (list->set < '(3 1 4 1 5 9 2 6 5 3)))
(check '(1 2 3 4 5 6 9) (set->list s-dup))
(check 7 (set-size s-dup))
(check #t (set-member? s-dup 9))
(check #f (set-member? s-dup 8))

;;; --- 持久化：旧集合不变 ---
(define s-old (list->set < '(1 2 3)))
(define s-new (set-add s-old 4))
(check '(1 2 3) (set->list s-old))
(check '(1 2 3 4) (set->list s-new))

;;; --- 删除 ---
(check '(1 3) (set->list (set-remove (list->set < '(1 2 3)) 2)))
(check '(1 2 3 5 6 7) (set->list (set-remove s-asc 4)))
(check '(1 2 3 4 5 6 7) (set->list (set-remove s-asc 99)))
(check 6 (set-size (set-remove s-asc 4)))

;;; --- set-fold：按元素升序累积 ---
(check 28 (set-fold + 0 s-asc))
(check '(7 6 5 4 3 2 1) (set-fold (lambda (x acc) (cons x acc)) '() s-asc))

;;; --- 集合运算 ---
(define s-a (list->set < '(1 2 3 4 5)))
(define s-b (list->set < '(4 5 6 7)))
(check '(1 2 3 4 5 6 7) (set->list (set-union s-a s-b)))
(check '(4 5) (set->list (set-intersection s-a s-b)))
(check '(1 2 3) (set->list (set-difference s-a s-b)))
(check '(6 7) (set->list (set-difference s-b s-a)))
(check '() (set->list (set-intersection s-a (list->set < '(8 9)))))

;;; --- 子集判断 ---
(check #t (set-subset? (list->set < '(1 3 5)) s-a))
(check #f (set-subset? (list->set < '(1 3 6)) s-a))
(check #t (set-subset? (make-set <) s-a))
(check #t (set-subset? s-a s-a))

;;; --- 非数值比较器：字符串集合 ---
(define s-str (list->set string<? '("pear" "apple" "orange" "apple")))
(check '("apple" "orange" "pear") (set->list s-str))
(check 3 (set-size s-str))
(check #t (set-member? s-str "apple"))

;;; --- 大规模：1..150 顺序插入 ---
(define s-big (list->set < (range 1 150)))
(check 150 (set-size s-big))
(check (range 1 150) (set->list s-big))
;;; 删除所有偶数后剩 75 个奇数
(define s-odd
  (let loop ((i 2) (s s-big))
    (if (> i 150)
        s
        (loop (+ i 2) (set-remove s i)))))
(check 75 (set-size s-odd))
(check 149 (set-fold (lambda (x acc) (if (odd? x) x acc)) 0 s-big))
(check '(1 3 5) (set->list (set-intersection (list->set < '(1 2 3 4 5)) s-odd)))
(check #f (set-member? s-odd 100))
(check #t (set-member? s-odd 99))
;;; 大规模集合运算
(check (range 1 200)
       (set->list (set-union s-big (list->set < (range 101 200)))))
(check (range 51 150)
       (set->list (set-intersection s-big (list->set < (range 51 200)))))
(check (range 1 50)
       (set->list (set-difference s-big (list->set < (range 51 200)))))
(check #t (set-subset? (list->set < (range 51 100)) s-big))
