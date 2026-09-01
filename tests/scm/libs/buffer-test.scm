;;; buffer 模块测试
(require 'buffer)

;;; 新缓冲为空
(check "" (buffer-contents (make-buffer)))

;;; display/write/newline 追加
(check "ab" (let ((b (make-buffer)))
              (buffer-display b "a")
              (buffer-display b #\b)
              (buffer-contents b)))

(check "x\"s\"(1 2)" (let ((b (make-buffer)))
                       (buffer-display b 'x)
                       (buffer-write b "s")
                       (buffer-write b '(1 2))
                       (buffer-contents b)))

(check "a\nb\n" (let ((b (make-buffer)))
                  (buffer-display b "a")
                  (buffer-newline b)
                  (buffer-display b "b")
                  (buffer-newline b)
                  (buffer-contents b)))

;;; buffer-contents 不消费内容（本实现 get-output-string 读后仍在）
(check "data" (let ((b (make-buffer)))
                (buffer-display b "data")
                (buffer-contents b)
                (buffer-contents b)))

;;; 继续追加
(check "firstsecond" (let ((b (make-buffer)))
                       (buffer-display b "first")
                       (buffer-contents b)
                       (buffer-display b "second")
                       (buffer-contents b)))

;;; buffer-length：长度正确且不清空缓冲
(check 5 (let ((b (make-buffer)))
           (buffer-display b "hello")
           (buffer-length b)))

(check "hello" (let ((b (make-buffer)))
                 (buffer-display b "hello")
                 (buffer-length b)
                 (buffer-contents b)))
