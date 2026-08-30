;;; DIVITER -- Benchmark which divides by 2 using lists of n ()'s.


(define (create-n n)
  (do ((n n (- n 1))
       (a '() (cons '() a)))
      ((= n 0) a)))

(define (iterative-div2 l)
  (do ((l l (cddr l))
       (a '() (cons (car l) a)))
      ((null? l) a)))

;;; ===== driver (added by scheme-rs; not part of the original file) =====
(display (length (iterative-div2 (create-n 100000)))) (newline)   ; ==> 50000
