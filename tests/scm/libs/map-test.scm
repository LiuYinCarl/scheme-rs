;;; map 模块测试：不可变 AVL 有序映射（含旋转、删除与大规模用例）
(require 'map)

;;; 生成 [lo, hi] 的升序整数列表（测试辅助）
(define (range lo hi)
  (if (> lo hi)
      '()
      (cons lo (range (+ lo 1) hi))))

;;; 依次把 ((1 . 10) (2 . 20) ...) 插入空 map
(define (build-map keys)
  (let loop ((ks keys) (m (make-map <)))
    (if (null? ks)
        m
        (loop (cdr ks) (map-add m (car ks) (* 10 (car ks)))))))

;;; --- 空 map ---
(check 0 (map-size (make-map <)))
(check #f (map-find (make-map <) 42))
(check #f (map-member? (make-map <) 42))
(check '() (map->alist (make-map <)))

;;; --- 升序插入 1..7（持续触发左旋）---
(define m-asc (build-map '(1 2 3 4 5 6 7)))
(check '(1 2 3 4 5 6 7) (map-keys m-asc))
(check '(10 20 30 40 50 60 70) (map-values m-asc))
(check 7 (map-size m-asc))

;;; --- 降序插入 7..1（持续触发右旋）---
(define m-desc (build-map '(7 6 5 4 3 2 1)))
(check '(1 2 3 4 5 6 7) (map-keys m-desc))
(check 7 (map-size m-desc))

;;; --- 锯齿序插入（触发左右 / 右左双旋转）---
(define m-zig (build-map '(4 2 6 1 3 5 7)))
(check '(1 2 3 4 5 6 7) (map-keys m-zig))
(check '((1 . 10) (2 . 20) (3 . 30) (4 . 40) (5 . 50) (6 . 60) (7 . 70))
       (map->alist m-zig))

;;; --- 键已存在时覆盖值，不增加大小 ---
(define m-dup (map-add (map-add (make-map <) 1 'a) 1 'b))
(check 1 (map-size m-dup))
(check 'b (map-find m-dup 1))

;;; --- 查找与成员判断 ---
(check 40 (map-find m-asc 4))
(check #f (map-find m-asc 99))
(check #t (map-member? m-asc 4))
(check #f (map-member? m-asc 99))
;;; 值为 #f 时 map-find 返回 #f，需用 map-member? 区分（文档已注明）
(define m-false (map-add (make-map <) 1 #f))
(check #f (map-find m-false 1))
(check #t (map-member? m-false 1))

;;; --- 持久化：旧 map 不受 add/remove 影响 ---
(define m-old (build-map '(1 2 3)))
(define m-new (map-add m-old 4 40))
(check '(1 2 3) (map-keys m-old))
(check '(1 2 3 4) (map-keys m-new))

;;; --- 删除：叶子 / 单子节点 / 双子节点（根）/ 不存在的键 ---
(check '(1 3) (map-keys (map-remove (build-map '(1 2 3)) 2)))
(check '(2 3) (map-keys (map-remove (build-map '(1 2 3)) 1)))
(check '(1 2 3 5 6 7) (map-keys (map-remove m-asc 4)))
(check '(1 2 3 4 5 6 7) (map-keys (map-remove m-asc 99)))
(check 6 (map-size (map-remove m-asc 4)))

;;; --- map-fold：按键升序累积 ---
(check 28 (map-fold (lambda (k v acc) (+ k acc)) 0 m-asc))
(check 280 (map-fold (lambda (k v acc) (+ v acc)) 0 m-asc))
(check '(7 6 5 4 3 2 1)
       (map-fold (lambda (k v acc) (cons k acc)) '() m-asc))

;;; --- alist->map：重复键以后出现的为准 ---
(define m-al (alist->map < '((2 . b) (1 . a) (3 . c) (2 . b2))))
(check '((1 . a) (2 . b2) (3 . c)) (map->alist m-al))
(check 3 (map-size m-al))

;;; --- 大规模：1..150 顺序插入（不平衡 BST 的最坏情形）---
(define m-big (build-map (range 1 150)))
(check 150 (map-size m-big))
(check (range 1 150) (map-keys m-big))
(check 1000 (map-find m-big 100))
(check #f (map-find m-big 151))
;;; AVL 平衡性：150 个节点的树高应远小于线性（avc-h 上界约 11）
(check #t (<= (avl-h (cdr m-big)) 11))
;;; 删除所有 3 的倍数后剩 100 个，且确被删干净
(define m-big2
  (let loop ((i 3) (m m-big))
    (if (> i 150)
        m
        (loop (+ i 3) (map-remove m i)))))
(check 100 (map-size m-big2))
(check #f (map-member? m-big2 99))
(check #t (map-member? m-big2 100))
(check #t (<= (avl-h (cdr m-big2)) 10))
