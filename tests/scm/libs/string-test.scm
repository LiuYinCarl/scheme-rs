;;; string 模块测试：OCaml String 风格的新函数
(require 'string)

;;; string-upcase / string-downcase
(check "ABC" (string-upcase "abC"))
(check "abc" (string-downcase "AbC"))
(check "" (string-upcase ""))
(check "123" (string-upcase "123"))
(check "a-b" (string-downcase "A-B"))

;;; string-capitalize / string-uncapitalize
(check "Hello" (string-capitalize "hELLO"))
(check "Hello" (string-capitalize "hello"))
(check "" (string-capitalize ""))
(check "A" (string-capitalize "a"))
(check "hello" (string-uncapitalize "Hello"))
(check "hello World" (string-uncapitalize "Hello World"))
(check "" (string-uncapitalize ""))

;;; string-concat（与 string-join 等价，参数顺序相反）
(check "a-b-c" (string-concat "-" '("a" "b" "c")))
(check "" (string-concat "-" '()))
(check "solo" (string-concat "-" '("solo")))
(check "a-b-c" (string-concat "-" '("a" "b" "c")))

;;; string-index
(check 0 (string-index "abc" #\a))
(check 2 (string-index "abc" #\c))
(check #f (string-index "abc" #\z))
(check #f (string-index "" #\a))

;;; string-map
(check "ABC" (string-map char-upcase "abc"))
(check "" (string-map char-upcase ""))
(check "xyz" (string-map (lambda (c) c) "xyz"))

;;; string-iteri：用副作用把 (下标 字符) 收集到端口里
(check "0:a,1:b,2:c,"
       (let ((p (open-output-string)))
         (string-iteri (lambda (i c)
                         (display i p)
                         (display ":" p)
                         (display c p)
                         (display "," p))
                       "abc")
         (get-output-string p)))
(check "" (let ((p (open-output-string)))
            (string-iteri (lambda (i c) (display c p)) "")
            (get-output-string p)))

;;; string-fold
(check "cba" (string-fold (lambda (acc c) (string-append (string c) acc)) "" "abc"))
(check 3 (string-fold (lambda (acc c) (+ acc 1)) 0 "abc"))
(check "" (string-fold (lambda (acc c) (string-append acc (string c))) "" ""))

;;; string-for-all
(check #t (string-for-all char-alphabetic? "abc"))
(check #f (string-for-all char-alphabetic? "ab1"))
(check #t (string-for-all char-alphabetic? ""))

;;; string-exists
(check #t (string-exists char-numeric? "ab1"))
(check #f (string-exists char-numeric? "abc"))
(check #f (string-exists char-numeric? ""))
