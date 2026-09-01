;;; format 模块测试
(require 'format)

;;; 无指令：原样返回
(check "" (sprintf ""))
(check "hello" (sprintf "hello"))

;;; ~a：display 形式
(check "x=1" (sprintf "x=~a" 1))
(check "hello world" (sprintf "~a ~a" "hello" 'world))
(check "list (1 2)" (sprintf "list ~a" '(1 2)))
(check "flag #t" (sprintf "flag ~a" #t))

;;; ~s：write 形式
(check "x=\"hi\"" (sprintf "x=~s" "hi"))
(check "sym abc" (sprintf "sym ~s" 'abc))
(check "(1 . 2)" (sprintf "~s" (cons 1 2)))

;;; 混合多个参数
(check "1+2=3" (sprintf "~a+~a=~a" 1 2 3))
(check "a \"a\"" (sprintf "~a ~s" "a" "a"))

;;; ~%：换行
(check "a\nb" (sprintf "a~%b"))
(check "\n" (sprintf "~%"))

;;; ~~：字面波浪号
(check "~a" (sprintf "~~a"))
(check "100~" (sprintf "100~~"))

;;; 组合
(check "~x=5\n" (sprintf "~~x=~a~%" 5))

;;; format：#f 等价 sprintf
(check "v=42" (format #f "v=~a" 42))

;;; format：#t 写到当前输出端口（此实现无 parameterize，只确认正常返回，
;;; 输出本身会直接出现在 stdout）
(check #t (begin (format #t "stdout:~a~%" 7) #t))

;;; format：写入给定输出端口
(check "p:ok" (let ((p (open-output-string)))
                (format p "p:~a" 'ok)
                (get-output-string p)))
