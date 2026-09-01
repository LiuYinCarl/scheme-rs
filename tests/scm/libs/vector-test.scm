;;; vector 模块测试：OCaml Array 风格的向量工具
(require 'vector)

;;; vector-copy
(check #(1 2 3) (vector-copy '#(1 2 3)))
(check #() (vector-copy '#()))
(check '(1 2 3)
       (let ((v (vector 1 2 3)))
         (let ((c (vector-copy v)))
           (vector-set! c 0 99)
           (vector->list v)))) ; 改拷贝不影响原向量

;;; vector-map
(check #(2 4 6) (vector-map (lambda (x) (* 2 x)) '#(1 2 3)))
(check #() (vector-map (lambda (x) x) '#()))
(check #("a" "b") (vector-map symbol->string '#(a b)))

;;; vector-mapi
(check #(0 2 6) (vector-mapi (lambda (i x) (* i x)) '#(5 2 3)))
(check #((0 a) (1 b)) (vector-mapi list '#(a b)))

;;; vector-for-each / vector-iteri（用累加副作用观察）
(check 6 (let ((s 0))
           (vector-for-each (lambda (x) (set! s (+ s x))) '#(1 2 3))
           s))
(check '(1 2 3) (let ((acc '()))
                   (vector-for-each (lambda (x) (set! acc (cons x acc)))
                                    '#(1 2 3))
                   (reverse acc)))
(check 8 (let ((s 0))
           (vector-iteri (lambda (i x) (set! s (+ s (* i x)))) '#(1 2 3))
           s))

;;; vector-fold-left / vector-fold-right
(check 10 (vector-fold-left + 0 '#(1 2 3 4)))
(check '(((() . 1) . 2) . 3) (vector-fold-left cons '() '#(1 2 3)))
(check 10 (vector-fold-right + 0 '#(1 2 3 4)))
(check '(1 2 3) (vector-fold-right cons '() '#(1 2 3)))
(check 0 (vector-fold-left + 0 '#()))

;;; vector-find
(check 1 (vector-find odd? '#(2 3 4)))
(check #f (vector-find odd? '#(2 4 6)))
(check #f (vector-find odd? '#()))

;;; vector-for-all / vector-exists
(check #t (vector-for-all even? '#(2 4 6)))
(check #f (vector-for-all even? '#(2 3 6)))
(check #t (vector-for-all even? '#()))
(check #t (vector-exists odd? '#(2 4 5)))
(check #f (vector-exists odd? '#(2 4 6)))
(check #f (vector-exists odd? '#()))

;;; vector-append
(check #(1 2 3 4) (vector-append '#(1 2) '#(3 4)))
(check #(1 2 3) (vector-append '#(1) '#(2) '#(3)))
(check #() (vector-append '#() '#()))
(check #(a b c) (vector-append '#(a b c) '#()))

;;; vector-reverse
(check #(3 2 1) (vector-reverse '#(1 2 3)))
(check #() (vector-reverse '#()))
(check #(1) (vector-reverse '#(1)))

;;; vector-count
(check 3 (vector-count odd? '#(1 2 3 4 5)))
(check 0 (vector-count odd? '#(2 4)))
(check 0 (vector-count odd? '#()))

;;; vector-sort
(check #(1 2 3) (vector-sort < '#(3 1 2)))
(check #() (vector-sort < '#()))
(check #(1) (vector-sort < '#(1)))
(check #(1 2 2 3) (vector-sort < '#(2 3 1 2)))
(check #(3 2 1) (vector-sort > '#(1 2 3)))
(check '(3 1 2)
       (let ((v (vector 3 1 2)))
         (vector-sort < v)
         (vector->list v))) ; 排序返回新向量，原向量不变

;;; vector-binary-search
(check 0 (vector-binary-search < '#(1 2 3) 1))
(check 1 (vector-binary-search < '#(1 2 3) 2))
(check 2 (vector-binary-search < '#(1 2 3) 3))
(check #f (vector-binary-search < '#(1 2 3) 4))
(check #f (vector-binary-search < '#() 1))
(check 3 (vector-binary-search < '#(1 3 5 7 9) 7))
(check #f (vector-binary-search < '#(1 3 5 7 9) 6))
(check 2 (vector-binary-search (lambda (a b) (> a b)) '#(9 7 5 3 1) 5))
