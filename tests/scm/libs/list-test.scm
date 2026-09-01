;;; list 模块 OCaml List 风格新增函数测试
(require 'list)

;;; filter-map
(check '(4 8) (filter-map (lambda (x) (and (even? x) (* x 2))) '(1 2 3 4)))
(check '() (filter-map (lambda (x) #f) '(1 2 3)))
(check '(1 2 3) (filter-map (lambda (x) x) '(1 2 3)))
(check '() (filter-map (lambda (x) x) '()))

;;; mapi
(check '(0 2 6) (mapi (lambda (i x) (* i x)) '(5 2 3)))
(check '((0 a) (1 b) (2 c)) (mapi list '(a b c)))
(check '() (mapi (lambda (i x) x) '()))

;;; iteri（只验证副作用的累积顺序）
(check '((0 a) (1 b) (2 c))
       (let ((seen '()))
         (iteri (lambda (i x) (set! seen (cons (list i x) seen))) '(a b c))
         (reverse seen)))
(check '() (iteri (lambda (i x) x) '()))

;;; flatten
(check '(1 2 3 4 5 6) (flatten '((1 2) (3) () (4 5 6))))
(check '() (flatten '()))
(check '() (flatten '(() ())))

;;; init
(check '(0 1 4 9) (init 4 (lambda (i) (* i i))))
(check '() (init 0 (lambda (i) i)))
(check '(x x x) (init 3 (lambda (i) 'x)))

;;; split
(check '((a b c) (1 2 3)) (split '((a 1) (b 2) (c 3))))
(check '(() ()) (split '()))

;;; rev-map
(check '(3 2 1) (rev-map (lambda (x) x) '(1 2 3)))
(check '(9 4 1) (rev-map (lambda (x) (* x x)) '(1 2 3)))
(check '() (rev-map (lambda (x) x) '()))

;;; merge
(check '(1 2 3 4 5 6) (merge < '(1 3 5) '(2 4 6)))
(check '(1 1 2) (merge < '(1) '(1 2)))
(check '(1 2) (merge < '() '(1 2)))
(check '(1 2) (merge < '(1 2) '()))
(check '() (merge < '() '()))
(check '(3 2 1) (merge > '(3 1) '(2)))

;;; for-all / exists
(check #t (for-all even? '(2 4 6)))
(check #f (for-all even? '(2 3 4)))
(check #t (for-all even? '()))
(check #t (exists odd? '(2 3 4)))
(check #f (exists odd? '(2 4 6)))
(check #f (exists odd? '()))

;;; count
(check 3 (count odd? '(1 2 3 4 5)))
(check 0 (count odd? '(2 4 6)))
(check 0 (count odd? '()))
(check 4 (count (lambda (x) #t) '(1 2 3 4)))
