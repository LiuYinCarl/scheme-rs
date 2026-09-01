;;; scheme-rs 扩展库 option 模块：OCaml Option 风格的可选值
;;;
;;; 用法：(require 'option)
;;; none 直接用符号 none 表示；(some v) 构造 (some v) 这样的列表，
;;; 带标签所以直接打印出来也清晰可读。

;;; 构造
(define (some v) (list 'some v))

;;; 谓词
(define (some? x) (and (pair? x) (eq? (car x) 'some)))
(define (none? x) (eq? x 'none))

;;; 对 some 内的值应用 f；none 原样返回
(define (option-map f opt)
  (if (some? opt)
      (some (f (cadr opt)))
      'none))

;;; 串联返回 option 的计算（f 需返回 option）
(define (option-bind opt f)
  (if (some? opt)
      (f (cadr opt))
      'none))

;;; 取出 some 内的值；none 则报错
(define (option-get opt)
  (if (some? opt)
      (cadr opt)
      (error "option-get: none")))

;;; 取出 some 内的值；none 返回默认值
(define (option-get-or opt default)
  (if (some? opt)
      (cadr opt)
      default))

;;; 值满足 pred 才保留，否则变 none
(define (option-filter pred opt)
  (if (and (some? opt) (pred (cadr opt)))
      opt
      'none))

;;; some 时对值执行副作用 f；返回未指定值
(define (option-iter f opt)
  (if (some? opt)
      (f (cadr opt))
      #f))

;;; 转列表：(some v) => (v)，none => ()
(define (option->list opt)
  (if (some? opt)
      (list (cadr opt))
      '()))
