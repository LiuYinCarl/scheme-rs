;;; scheme-rs 扩展库 format 模块：格式化输出（OCaml Printf / SRFI-28/48 子集）
;;;
;;; 用法：(require 'format)
;;; 支持的指令：~a（display 形式）、~s（write 形式）、~%（换行）、~~（字面波浪号）。
;;; 未知指令或参数个数不匹配会 (error ...)。

;;; 把 fmt 和 args 展开后写入 port
(define (format-into port fmt args)
  (let loop ((cs (string->list fmt)) (args args))
    (cond ((null? cs)
           (if (null? args)
               #t
               (error "format: too many arguments" args)))
          ((char=? (car cs) #\~)
           (if (null? (cdr cs))
               (error "format: dangling ~ at end of format string")
               (let ((d (cadr cs)))
                 (cond ((char=? d #\a)
                        (if (null? args)
                            (error "format: not enough arguments for ~a")
                            (begin (display (car args) port)
                                   (loop (cddr cs) (cdr args)))))
                       ((char=? d #\s)
                        (if (null? args)
                            (error "format: not enough arguments for ~s")
                            (begin (write (car args) port)
                                   (loop (cddr cs) (cdr args)))))
                       ((char=? d #\%)
                        (newline port)
                        (loop (cddr cs) args))
                       ((char=? d #\~)
                        (write-char (car cs) port)
                        (loop (cddr cs) args))
                       (else
                        (error "format: unknown directive ~" d))))))
          (else
           (write-char (car cs) port)
           (loop (cdr cs) args)))))

;;; (sprintf fmt . args)：按指令展开，返回字符串
(define (sprintf fmt . args)
  (let ((port (open-output-string)))
    (format-into port fmt args)
    (get-output-string port)))

;;; (format dest fmt . args)：dest 为 #t 写到当前输出端口（返回未指定值），
;;; 为输出端口则写到该端口，为 #f 等价于 sprintf
(define (format dest fmt . args)
  (cond ((eq? dest #f) (apply sprintf fmt args))
        ((eq? dest #t)
         (display (apply sprintf fmt args))
         (if #f #f))
        ((output-port? dest)
         (display (apply sprintf fmt args) dest)
         (if #f #f))
        (else
         (error "format: bad destination" dest))))
