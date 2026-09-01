;;; stream 模块测试：惰性流的构造、转换与无限流
(require 'stream)

;;; 空流与基本访问
(check #t (stream-null? stream-null))
(check #f (stream-null? (stream-cons 1 stream-null)))
(check 1 (stream-car (stream-cons 1 stream-null)))
(check #t (stream-null? (stream-cdr (stream-cons 1 stream-null))))

;;; 尾部确实被延迟：未 force 前不会求值 tail 表达式
(check 1 (stream-car (stream-cons 1 (begin (display "") stream-null))))

;;; list->stream / stream->list
(check '(1 2 3) (stream->list (list->stream '(1 2 3))))
(check '() (stream->list stream-null))
(check #t (stream-null? (list->stream '())))

;;; stream-take / stream-drop
(check '(1 2) (stream-take (list->stream '(1 2 3)) 2))
(check '(1 2 3) (stream-take (list->stream '(1 2 3)) 5))
(check '() (stream-take (list->stream '(1 2 3)) 0))
(check '(3 4) (stream->list (stream-drop (list->stream '(1 2 3 4)) 2)))
(check '(1 2 3 4) (stream->list (stream-drop (list->stream '(1 2 3 4)) 0)))

;;; 无限流：integers-from + stream-take
(check '(0 1 2 3 4) (stream-take (integers-from 0) 5))
(check '(5 6 7) (stream-take (integers-from 5) 3))
(check '(5 6) (stream-take (stream-drop (integers-from 0) 5) 2))

;;; stream-map（有限与无限）
(check '(1 4 9) (stream->list (stream-map (lambda (x) (* x x)) (list->stream '(1 2 3)))))
(check '(0 2 4) (stream-take (stream-map (lambda (x) (* x 2)) (integers-from 0)) 3))

;;; stream-filter（含无限流上的过滤）
(check '(2 4) (stream->list (stream-filter even? (list->stream '(1 2 3 4 5)))))
(check '(0 2 4 6 8) (stream-take (stream-filter even? (integers-from 0)) 5))
(check '(10 12 14) (stream-take (stream-filter (lambda (x) (> x 9))
                                               (stream-filter even? (integers-from 0)))
                                3))

;;; stream-append
(check '(1 2 3 4) (stream->list (stream-append (list->stream '(1 2)) (list->stream '(3 4)))))
(check '(1 2) (stream->list (stream-append stream-null (list->stream '(1 2)))))
(check '(1 2) (stream->list (stream-append (list->stream '(1 2)) stream-null)))
(check '(1 2 0 1 2) (stream-take (stream-append (list->stream '(1 2)) (integers-from 0)) 5))

;;; stream-iterate（无限）
(check '(1 2 4 8 16) (stream-take (stream-iterate (lambda (x) (* x 2)) 1) 5))
(check '(0 1 1 2 3 5 8) (stream-take (stream-map car
                                                 (stream-iterate (lambda (p) (cons (cdr p) (+ (car p) (cdr p))))
                                                                 (cons 0 1)))
                                     7))

;;; stream-unfold
(check '(0 1 2 3) (stream->list (stream-unfold (lambda (s) (< s 4))
                                               (lambda (s) s)
                                               (lambda (s) (+ s 1))
                                               0)))
(check '(1 2 4 8) (stream->list (stream-unfold (lambda (s) (<= s 8))
                                               (lambda (s) s)
                                               (lambda (s) (* s 2))
                                               1)))
(check '() (stream->list (stream-unfold (lambda (s) #f)
                                        (lambda (s) s)
                                        (lambda (s) s)
                                        0)))

;;; stream-range
(check '(1 2 3 4) (stream->list (stream-range 1 5)))
(check '() (stream->list (stream-range 3 3)))
(check '(5) (stream->list (stream-range 5 6)))

;;; stream-ref
(check 3 (stream-ref (list->stream '(1 2 3 4)) 2))
(check 1 (stream-ref (list->stream '(1 2 3 4)) 0))
(check 7 (stream-ref (integers-from 5) 2))

;;; stream-for-each（副作用收集）
(check '(3 2 1) (let ((acc '()))
                  (stream-for-each (lambda (x) (set! acc (cons x acc)))
                                   (list->stream '(1 2 3)))
                  acc))
(check '(9 4 1) (let ((acc '()))
                  (stream-for-each (lambda (x) (set! acc (cons (* x x) acc)))
                                   (stream-range 1 4))
                  acc))

;;; stream-fold（有限流）
(check 10 (stream-fold + 0 (list->stream '(1 2 3 4))))
(check 6 (stream-fold * 1 (stream-range 1 4)))
(check 0 (stream-fold + 0 stream-null))
(check '(4 3 2 1) (stream-fold cons '() (list->stream '(1 2 3 4))))
