;;; scheme-rs 扩展库 string 模块：字符串工具（借自 Python/Ruby 标准库）
;;;
;;; 用法：(require 'string)
;;; 全部是 R5RS 之外的新名字；不 require 就不会出现在全局环境里。

;;; 反转字符串
(define (string-reverse s)
  (list->string (reverse (string->list s))))

;;; 重复 n 次：(string-repeat "ab" 3) => "ababab"
(define (string-repeat s n)
  (if (<= n 0)
      ""
      (string-append s (string-repeat s (- n 1)))))

;;; 去掉首尾空白
(define (string-trim s)
  (let ((cs (string->list s)))
    (list->string
      (let trim-front ((cs cs))
        (if (and (pair? cs) (char-whitespace? (car cs)))
            (trim-front (cdr cs))
            (let trim-back ((cs (reverse cs)))
              (if (and (pair? cs) (char-whitespace? (car cs)))
                  (trim-back (cdr cs))
                  (reverse cs))))))))

;;; 前缀/后缀判断
(define (string-prefix? prefix s)
  (let ((n (string-length prefix)))
    (and (>= (string-length s) n)
         (string=? prefix (substring s 0 n)))))

(define (string-suffix? suffix s)
  (let* ((n (string-length suffix))
         (m (string-length s)))
    (and (>= m n)
         (string=? suffix (substring s (- m n) m)))))

;;; 子串查找：返回首次出现的下标，没有则 #f（SRFI-13 惯例）
(define (string-contains? s sub)
  (let ((n (string-length s))
        (m (string-length sub)))
    (cond ((> m n) #f)
          ((= m 0) 0)
          (else
           (let loop ((i 0))
             (cond ((> (+ i m) n) #f)
                   ((string=? sub (substring s i (+ i m))) i)
                   (else (loop (+ i 1)))))))))

;;; 按分隔字符拆分：(string-split "a,b,c" #\,) => ("a" "b" "c")
(define (string-split s sep)
  (let ((n (string-length s)))
    (let loop ((i 0) (start 0) (acc '()))
      (cond ((= i n)
             (reverse (cons (substring s start n) acc)))
            ((char=? (string-ref s i) sep)
             (loop (+ i 1) (+ i 1) (cons (substring s start i) acc)))
            (else
             (loop (+ i 1) start acc))))))

;;; 用分隔符连接：(string-join '("a" "b") "-") => "a-b"
(define (string-join strs sep)
  (cond ((null? strs) "")
        ((null? (cdr strs)) (car strs))
        (else (string-append (car strs) sep (string-join (cdr strs) sep)))))

;;; 替换所有出现的子串：(string-replace "a-b-c" "-" "+") => "a+b+c"
(define (string-replace s from to)
  (cond ((string=? from "") (error "string-replace: empty pattern"))
        (else
         (let ((i (string-contains? s from)))
           (if (not i)
               s
               (string-append (substring s 0 i)
                              to
                              (string-replace
                                (substring s (+ i (string-length from)) (string-length s))
                                from
                                to)))))))
