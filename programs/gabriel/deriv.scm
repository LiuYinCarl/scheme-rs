;;; DERIV -- Symbolic derivation.


;;; Returns the wrong answer for quotients.
;;; Fortunately these aren't used in the benchmark.

(define (deriv a)
  (cond ((not (pair? a))
         (if (eq? a 'x) 1 0))
        ((eq? (car a) '+)
         (cons '+
               (map deriv (cdr a))))
        ((eq? (car a) '-)
         (cons '-
               (map deriv (cdr a))))
        ((eq? (car a) '*)
         (list '*
               a
               (cons '+
                     (map (lambda (a) (list '/ (deriv a) a)) (cdr a)))))
        ((eq? (car a) '/)
         (list '-
               (list '/
                     (deriv (cadr a))
                     (caddr a))
               (list '/
                     (cadr a)
                     (list '*
                           (caddr a)
                           (caddr a)
                           (deriv (caddr a))))))
        (else
         (error #f "No derivation method available"))))

;;; ===== driver (added by scheme-rs; not part of the original file) =====
(define input '(+ (* 3 x x) (* a x x) (* b x) 5))
(define (drive n)
  (if (= n 0) 'done (begin (deriv input) (drive (- n 1)))))
(display (deriv input)) (newline)
(drive 10000)
(display 'done) (newline)
