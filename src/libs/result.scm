;;; scheme-rs 扩展库 result 模块：OCaml Result 风格的成功/失败值
;;;
;;; 用法：(require 'result)
;;; (ok v) 构造 (ok v)，(err e) 构造 (err e)，都是带标签的列表，
;;; 直接打印出来也清晰可读。

;;; 构造
(define (ok v) (list 'ok v))
(define (err e) (list 'err e))

;;; 谓词
(define (ok? x) (and (pair? x) (eq? (car x) 'ok)))
(define (err? x) (and (pair? x) (eq? (car x) 'err)))

;;; 对 ok 内的值应用 f；err 原样返回
(define (result-map f res)
  (if (ok? res)
      (ok (f (cadr res)))
      res))

;;; 对 err 内的错误应用 f；ok 原样返回
(define (result-map-err f res)
  (if (err? res)
      (err (f (cadr res)))
      res))

;;; 串联返回 result 的计算（f 需返回 result）
(define (result-bind res f)
  (if (ok? res)
      (f (cadr res))
      res))

;;; 取出 ok 内的值；err 则报错
(define (result-get res)
  (if (ok? res)
      (cadr res)
      (error "result-get: err" (cadr res))))

;;; 取出 err 内的错误；ok 则报错
(define (result-get-err res)
  (if (err? res)
      (cadr res)
      (error "result-get-err: ok" (cadr res))))

;;; 取出 ok 内的值；err 返回默认值
(define (result-get-or res default)
  (if (ok? res)
      (cadr res)
      default))
