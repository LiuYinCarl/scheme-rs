;;; scheme-rs 扩展库 buffer 模块：可变字符串缓冲（OCaml Buffer 风格）
;;;
;;; 用法：(require 'buffer)
;;; 就是字符串输出端口的薄封装；buffer-contents 行为同 get-output-string。

;;; 新建空缓冲
(define (make-buffer)
  (open-output-string))

;;; 以 display 形式追加
(define (buffer-display buf x)
  (display x buf))

;;; 以 write 形式追加
(define (buffer-write buf x)
  (write x buf))

;;; 追加换行
(define (buffer-newline buf)
  (newline buf))

;;; 取走全部内容（行为同 get-output-string：本实现中读后内容仍在）
(define (buffer-contents buf)
  (get-output-string buf))

;;; 当前内容长度（不清空缓冲）
(define (buffer-length buf)
  (string-length (get-output-string buf)))
