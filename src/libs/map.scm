;;; scheme-rs 扩展库 map 模块：OCaml Map 风格的不可变有序映射
;;;
;;; 用法：(require 'map)
;;; 键的顺序由用户提供的比较器 (lt? a b) 决定（严格小于，类似 <）。
;;; 底层是 AVL 自平衡二叉搜索树；所有修改操作都是纯函数式的，
;;; 返回新 map，旧 map 保持不变（持久化数据结构）。
;;;
;;; 注意：(map-find m k) 在键不存在时返回 #f，因此无法区分
;;; "键不存在" 和 "键存在但值就是 #f"；需要先区分时请用 map-member?。

;;; ---------------- AVL 树内部实现 ----------------
;;; 空树用 #f 表示；节点是向量 #(key value left right height size)。
;;; 以下 avl-* 名字是内部实现细节，set 模块会直接复用它们。

(define (avl-key t) (vector-ref t 0))
(define (avl-val t) (vector-ref t 1))
(define (avl-left t) (vector-ref t 2))
(define (avl-right t) (vector-ref t 3))

(define (avl-h t) (if (not t) 0 (vector-ref t 4)))
(define (avl-size t) (if (not t) 0 (vector-ref t 5)))

;;; 由子树组装节点，自动重算高度与大小
(define (avl-make-node k v l r)
  (vector k v l r
          (+ 1 (max (avl-h l) (avl-h r)))
          (+ 1 (avl-size l) (avl-size r))))

;;; 平衡因子：左子树高 - 右子树高
(define (avl-bf t)
  (- (avl-h (avl-left t)) (avl-h (avl-right t))))

(define (avl-rotate-left t)
  (let ((r (avl-right t)))
    (avl-make-node (avl-key r) (avl-val r)
                   (avl-make-node (avl-key t) (avl-val t)
                                  (avl-left t) (avl-left r))
                   (avl-right r))))

(define (avl-rotate-right t)
  (let ((l (avl-left t)))
    (avl-make-node (avl-key l) (avl-val l)
                   (avl-left l)
                   (avl-make-node (avl-key t) (avl-val t)
                                  (avl-right l) (avl-right t)))))

;;; 失衡时旋转恢复（|bf| > 1）；必要时先转子树（左右 / 右左 情形）
(define (avl-balance t)
  (let ((bf (avl-bf t)))
    (cond ((> bf 1)
           (if (< (avl-bf (avl-left t)) 0)
               (avl-rotate-right
                 (avl-make-node (avl-key t) (avl-val t)
                                (avl-rotate-left (avl-left t))
                                (avl-right t)))
               (avl-rotate-right t)))
          ((< bf -1)
           (if (> (avl-bf (avl-right t)) 0)
               (avl-rotate-left
                 (avl-make-node (avl-key t) (avl-val t)
                                (avl-left t)
                                (avl-rotate-right (avl-right t))))
               (avl-rotate-left t)))
          (else t))))

(define (avl-insert lt? t k v)
  (cond ((not t) (avl-make-node k v #f #f))
        ((lt? k (avl-key t))
         (avl-balance (avl-make-node (avl-key t) (avl-val t)
                                     (avl-insert lt? (avl-left t) k v)
                                     (avl-right t))))
        ((lt? (avl-key t) k)
         (avl-balance (avl-make-node (avl-key t) (avl-val t)
                                     (avl-left t)
                                     (avl-insert lt? (avl-right t) k v))))
        (else
         ;; 键已存在：替换值
         (avl-make-node k v (avl-left t) (avl-right t)))))

;;; 摘除子树最小节点，返回 (最小节点 . 新子树)
(define (avl-remove-min t)
  (if (not (avl-left t))
      (cons t (avl-right t))
      (let ((r (avl-remove-min (avl-left t))))
        (cons (car r)
              (avl-balance (avl-make-node (avl-key t) (avl-val t)
                                          (cdr r) (avl-right t)))))))

(define (avl-remove lt? t k)
  (cond ((not t) #f)
        ((lt? k (avl-key t))
         (avl-balance (avl-make-node (avl-key t) (avl-val t)
                                     (avl-remove lt? (avl-left t) k)
                                     (avl-right t))))
        ((lt? (avl-key t) k)
         (avl-balance (avl-make-node (avl-key t) (avl-val t)
                                     (avl-left t)
                                     (avl-remove lt? (avl-right t) k))))
        (else
         (cond ((not (avl-left t)) (avl-right t))
               ((not (avl-right t)) (avl-left t))
               (else
                ;; 双子：用右子树的最小节点顶替
                (let ((r (avl-remove-min (avl-right t))))
                  (avl-balance (avl-make-node (avl-key (car r))
                                              (avl-val (car r))
                                              (avl-left t)
                                              (cdr r)))))))))

(define (avl-lookup lt? t k)
  (cond ((not t) #f)
        ((lt? k (avl-key t)) (avl-lookup lt? (avl-left t) k))
        ((lt? (avl-key t) k) (avl-lookup lt? (avl-right t) k))
        (else t)))

;;; 按键升序中序折叠：(f 键 值 累积值)
(define (avl-fold f acc t)
  (if (not t)
      acc
      (avl-fold f
                (f (avl-key t) (avl-val t) (avl-fold f acc (avl-left t)))
                (avl-right t))))

;;; ---------------- 公开接口 ----------------
;;; map 表示为 (比较器 . 树)，比较器随 map 一路传递。

;;; (make-map lt?)：创建空 map，lt? 是键的严格小于比较器
(define (make-map lt?)
  (cons lt? #f))

;;; 插入或覆盖键值对，返回新 map（原 map 不变）
(define (map-add m k v)
  (cons (car m) (avl-insert (car m) (cdr m) k v)))

;;; 查找键对应的值；键不存在返回 #f（无法与值为 #f 区分，见文件头说明）
(define (map-find m k)
  (let ((node (avl-lookup (car m) (cdr m) k)))
    (if node (avl-val node) #f)))

;;; 键是否存在
(define (map-member? m k)
  (if (avl-lookup (car m) (cdr m) k) #t #f))

;;; 删除键，返回新 map；键不存在时返回等值的新 map
(define (map-remove m k)
  (cons (car m) (avl-remove (car m) (cdr m) k)))

;;; 键值对个数
(define (map-size m)
  (avl-size (cdr m)))

;;; 按键升序折叠：(f 键 值 累积值)
(define (map-fold f init m)
  (avl-fold f init (cdr m)))

;;; 升序键值对列表：((k1 . v1) (k2 . v2) ...)
(define (map->alist m)
  (reverse (map-fold (lambda (k v acc) (cons (cons k v) acc)) '() m)))

;;; 升序键列表
(define (map-keys m)
  (map car (map->alist m)))

;;; 按键升序排列的值列表
(define (map-values m)
  (map cdr (map->alist m)))

;;; 由 assoc 列表构建 map；重复的键以后出现的为准
(define (alist->map lt? xs)
  (let loop ((xs xs) (m (make-map lt?)))
    (if (null? xs)
        m
        (loop (cdr xs) (map-add m (caar xs) (cdar xs))))))
