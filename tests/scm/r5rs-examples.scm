;; Auto-extracted from the R5RS report examples (chapters 4, 5, 6).
(define *tests-run* 0)
(define *tests-passed* 0)
(define *tests-failed* 0)
(define-syntax test
  (syntax-rules ()
    ((test expect expr)
     (begin
       (set! *tests-run* (+ *tests-run* 1))
       (let ((res expr))
         (cond ((equal? res expect)
                (set! *tests-passed* (+ *tests-passed* 1)))
               (else
                (set! *tests-failed* (+ *tests-failed* 1))
                (display "FAIL: ") (write 'expr)
                (display " expected ") (write expect)
                (display " got ") (write res) (newline))))))))
(define (test-end)
  (write *tests-passed*) (display " out of ") (write *tests-run*)
  (display " passed") (newline))


(test '28
(begin (define x 28)

x))

(test 'a
(quote a))

(test '#(a b c)
(quote #(a b c)))

(test '(+ 1 2)
(quote (+ 1 2)))

(test 'a
'a)

(test '#(a b c)
'#(a b c))

(test '()
'())

(test '(+ 1 2)
'(+ 1 2))

(test '(quote a)
'(quote a))

(test '(quote a)
''a)

(test '"abc"
'"abc")

(test '"abc"
"abc")

(test '145932
'145932)

(test '145932
145932)

(test '#t
'#t)

(test '#t
#t)

(test '7
(+ 3 4))

(test '12
((if #f + *) 3 4))

(test '8
((lambda (x) (+ x x)) 4))

(test '3
(begin (define reverse-subtract

  (lambda (x y) (- y x)))

(reverse-subtract 7 10)))

(test '10
(begin (define add4

  (let ((x 4))

    (lambda (y) (+ x y))))

(add4 6)))

(test '(3 4 5 6)
((lambda x x) 3 4 5 6))

(test '(5 6)
((lambda (x y . z) z)

3 4 5 6))

(test 'yes
(if (> 3 2) 'yes 'no))

(test 'no
(if (> 2 3) 'yes 'no))

(test '1
(if (> 3 2)

    (- 3 2)

(+ 3 2)))

(test '3
(begin (define x 2)

(+ x 1)))

(set! x 4)

(test '5
(+ x 1))

(test 'greater
(cond ((> 3 2) 'greater)

((< 3 2) 'less)))

(test 'equal
(cond ((> 3 3) 'greater)

      ((< 3 3) 'less)

(else 'equal)))

(test '2
(cond ((assv 'b '((a 1) (b 2))) => cadr)

(else #f)))

(test 'composite
(case (* 2 3)

  ((2 3 5 7) 'prime)

((1 4 6 8 9) 'composite)))

(case (car '(c d))

  ((a) 'a)

((b) 'b))

(test 'consonant
(case (car '(c d))

  ((a e i o u) 'vowel)

  ((w y) 'semivowel)

(else 'consonant)))

(test '#t
(and (= 2 2) (> 2 1)))

(test '#f
(and (= 2 2) (< 2 1)))

(test '(f g)
(and 1 2 'c '(f g)))

(test '#t
(and))

(test '#t
(or (= 2 2) (> 2 1)))

(test '#t
(or (= 2 2) (< 2 1)))

(test '#f
(or #f #f #f))

(test '(b c)
(or (memq 'b '(a b c))

(/ 3 0)))

(test '6
(let ((x 2) (y 3))

(* x y)))

(test '35
(let ((x 2) (y 3))

  (let ((x 7)

        (z (+ x y)))

(* z x))))

(test '70
(let ((x 2) (y 3))

  (let* ((x 7)

         (z (+ x y)))

(* z x))))

(test '#t
(letrec ((even?

          (lambda (n)

            (if (zero? n)

                #t

                (odd? (- n 1)))))

         (odd?

          (lambda (n)

            (if (zero? n)

                #f

                (even? (- n 1))))))

  (even? 88)))

(test '6
(begin (define x 0)



(begin (set! x 5)

(+ x 1))))

(begin (display "4 plus 1 equals ")

(display (+ 4 1)))

(test '#(0 1 2 3 4)
(do ((vec (make-vector 5))

     (i 0 (+ i 1)))

    ((= i 5) vec)

(vector-set! vec i i)))

(test '25
(let ((x '(1 3 5 7 9)))

  (do ((x x (cdr x))

       (sum 0 (+ sum (car x))))

((null? x) sum))))

(test '((6 1 3) (-5 -2))
(let loop ((numbers '(3 -2 1 6 -5))

           (nonneg '())

           (neg '()))

  (cond ((null? numbers) (list nonneg neg))

        ((>= (car numbers) 0)

         (loop (cdr numbers)

               (cons (car numbers) nonneg)

               neg))

        ((< (car numbers) 0)

         (loop (cdr numbers)

               nonneg

               (cons (car numbers) neg))))))

(test '(list 3 4)
`(list ,(+ 1 2) 4))

(test '(list a (quote a))
(let ((name 'a)) `(list ,name ',name)))

(test '(a 3 4 5 6 b)
`(a ,(+ 1 2) ,@(map abs '(4 -5 6)) b))

(test '((foo 7) . cons)
`(( foo ,(- 10 3)) ,@(cdr '(c)) . ,(car '(cons))))

(test '#(10 5 2 4 3 8)
`#(10 5 ,(sqrt 4) ,@(map sqrt '(16 9)) 8))

(test '(a `(b ,(+ 1 2) ,(foo 4 d) e) f)
`(a `(b ,(+ 1 2) ,(foo ,(+ 1 3) d) e) f))

(test '(a `(b ,x ,'y d) e)
(let ((name1 'x)

      (name2 'y))

  `(a `(b ,,name1 ,',name2 d) e)))

(test '(list 3 4)
(quasiquote (list (unquote (+ 1 2)) 4)))

(test '`(list ,(+ 1 2) 4)
'(quasiquote (list (unquote (+ 1 2)) 4)))

(test 'now
(let-syntax ((when (syntax-rules ()

                     ((when test stmt1 stmt2 ...)

                      (if test

                          (begin stmt1

                                 stmt2 ...))))))

  (let ((if #t))

    (when if (set! if 'now))

if)))

(test 'outer
(let ((x 'outer))

  (let-syntax ((m (syntax-rules () ((m) x))))

    (let ((x 'inner))

(m)))))

(test '7
(letrec-syntax

  ((my-or (syntax-rules ()

            ((my-or) #f)

            ((my-or e) e)

            ((my-or e1 e2 ...)

             (let ((temp e1))

               (if temp

                   temp

                   (my-or e2 ...)))))))

  (let ((x #f)

        (y 7)

        (temp 8)

        (let odd?)

        (if even?))

    (my-or x

           (let temp)

           (if y)

y))))

(test 'ok
(let ((=> #f))

(cond (#t => 'ok))))

(test '6
(begin (define add3

  (lambda (x) (+ x 3)))

(add3 3)))

(test '1
(begin (define first car)

(first '(1 2))))

(test '45
(let ((x 5))

  (define foo (lambda (y) (bar x y)))

  (define bar (lambda (a b) (+ (* a b) a)))

(foo (+ x 3))))

(test '#t
(eqv? 'a 'a))

(test '#f
(eqv? 'a 'b))

(test '#t
(eqv? 2 2))

(test '#t
(eqv? '() '()))

(test '#t
(eqv? 100000000 100000000))

(test '#f
(eqv? (cons 1 2) (cons 1 2)))

(test '#f
(eqv? (lambda () 1)

(lambda () 2)))

(test '#f
(eqv? #f 'nil))

(test '#t
(let ((p (lambda (x) x)))

(eqv? p p)))

(eqv? "" "")

(eqv? '#() '#())

(eqv? (lambda (x) x)

(lambda (x) x))

(eqv? (lambda (x) x)

(lambda (y) y))

(test '#t
(begin (define gen-counter

  (lambda ()

    (let ((n 0))

      (lambda () (set! n (+ n 1)) n))))

(let ((g (gen-counter)))

(eqv? g g))))

(test '#f
(eqv? (gen-counter) (gen-counter)))

(test '#t
(begin (define gen-loser

  (lambda ()

    (let ((n 0))

      (lambda () (set! n (+ n 1)) 27))))

(let ((g (gen-loser)))

(eqv? g g))))

(eqv? (gen-loser) (gen-loser))

(letrec ((f (lambda () (if (eqv? f g) 'both 'f)))

         (g (lambda () (if (eqv? f g) 'both 'g))))

  (eqv? f g))

(test '#f
(letrec ((f (lambda () (if (eqv? f g) 'f 'both)))

         (g (lambda () (if (eqv? f g) 'g 'both))))

  (eqv? f g)))

(eqv? '(a) '(a))

(eqv? "a" "a")

(eqv? '(b) (cdr '(a b)))

(test '#t
(let ((x '(a)))

(eqv? x x)))

(test '#t
(eq? 'a 'a))

(eq? '(a) '(a))

(test '#f
(eq? (list 'a) (list 'a)))

(eq? "a" "a")

(eq? "" "")

(test '#t
(eq? '() '()))

(eq? 2 2)

(eq? #\A #\A)

(test '#t
(eq? car car))

(let ((n (+ 2 3)))

(eq? n n))

(test '#t
(let ((x '(a)))

(eq? x x)))

(test '#t
(let ((x '#()))

(eq? x x)))

(test '#t
(let ((p (lambda (x) x)))

(eq? p p)))

(test '#t
(equal? 'a 'a))

(test '#t
(equal? '(a) '(a)))

(test '#t
(equal? '(a (b) c)

'(a (b) c)))

(test '#t
(equal? "abc" "abc"))

(test '#t
(equal? 2 2))

(test '#t
(equal? (make-vector 5 'a)

(make-vector 5 'a)))

(equal? (lambda (x) x)

(lambda (y) y))

(test '#t
(complex? 3))

(test '#t
(real? 3))

(test '#t
(real? #e1e10))

(test '#t
(rational? 6/10))

(test '#t
(rational? 6/3))

(test '#t
(integer? 3.0))

(test '#t
(integer? 8/4))

(test '4    ; exact
(max 3 4))

(test '4.0  ; inexact
(max 3.9 4))

(test '7
(+ 3 4))

(test '3
(+ 3))

(test '0
(+))

(test '4
(* 4))

(test '1
(*))

(test '-1
(- 3 4))

(test '-6
(- 3 4 5))

(test '-3
(- 3))

(test '3/20
(/ 3 4 5))

(test '1/3
(/ 3))

(test '7
(abs -7))

(test '1
(modulo 13 4))

(test '1
(remainder 13 4))

(test '3
(modulo -13 4))

(test '-1
(remainder -13 4))

(test '-3
(modulo 13 -4))

(test '1
(remainder 13 -4))

(test '-1
(modulo -13 -4))

(test '-1
(remainder -13 -4))

(test '-1.0  ; inexact
(remainder -13 -4.0))

(test '4
(gcd 32 -36))

(test '0
(gcd))

(test '288
(lcm 32 -36))

(test '288.0  ; inexact
(lcm 32.0 -36))

(test '1
(lcm))

(test '3
(numerator (/ 6 4)))

(test '2
(denominator (/ 6 4)))

(test '2.0
(denominator

(exact->inexact (/ 6 4))))

(test '-5.0
(floor -4.3))

(test '-4.0
(ceiling -4.3))

(test '-4.0
(truncate -4.3))

(test '-4.0
(round -4.3))

(test '3.0
(floor 3.5))

(test '4.0
(ceiling 3.5))

(test '3.0
(truncate 3.5))

(test '4.0  ; inexact
(round 3.5))

(test '4    ; exact
(round 7/2))

(test '7
(round 7))

(test '1/3    ; exact
(rationalize

(inexact->exact .3) 1/10))

(test '#i1/3  ; inexact
(rationalize .3 1/10))

(test '100
(string->number "100"))

(test '256
(string->number "100" 16))

(test '100.0
(string->number "1e2"))

(test '1500.0
(string->number "15##"))

(test '#t
#t)

(test '#f
#f)

(test '#f
'#f)

(test '#f
(not #t))

(test '#f
(not 3))

(test '#f
(not (list 3)))

(test '#t
(not #f))

(test '#f
(not '()))

(test '#f
(not (list)))

(test '#f
(not 'nil))

(test '#t
(boolean? #f))

(test '#f
(boolean? 0))

(test '#f
(boolean? '()))

(test '(a b c)
(begin (define x (list 'a 'b 'c))

(define y x)

y))

(test '#t
(list? y))

(set-cdr! x 4)

(test '(a . 4)
x)

(test '#t
(eqv? x y))

(test '(a . 4)
y)

(test '#f
(list? y))

(set-cdr! x x)

(test '#f
(list? x))

(test '#t
(pair? '(a . b)))

(test '#t
(pair? '(a b c)))

(test '#f
(pair? '()))

(test '#f
(pair? '#(a b)))

(test '(a)
(cons 'a '()))

(test '((a) b c d)
(cons '(a) '(b c d)))

(test '("a" b c)
(cons "a" '(b c)))

(test '(a . 3)
(cons 'a 3))

(test '((a b) . c)
(cons '(a b) 'c))

(test 'a
(car '(a b c)))

(test '(a)
(car '((a) b c d)))

(test '1
(car '(1 . 2)))

(test '(b c d)
(cdr '((a) b c d)))

(test '2
(cdr '(1 . 2)))

(begin (define (f) (list 'not-a-constant-list))

(define (g) '(constant-list))

(set-car! (f) 3))

(define caddr (lambda (x) (car (cdr (cdr x)))))

(test '#t
(list? '(a b c)))

(test '#t
(list? '()))

(test '#f
(list? '(a . b)))

(test '#f
(let ((x (list 'a)))

          (set-cdr! x x)

(list? x)))

(test '(a 7 c)
(list 'a (+ 3 4) 'c))

(test '()
(list))

(test '3
(length '(a b c)))

(test '3
(length '(a (b) (c d e))))

(test '0
(length '()))

(test '(x y)
(append '(x) '(y)))

(test '(a b c d)
(append '(a) '(b c d)))

(test '(a (b) (c))
(append '(a (b)) '((c))))

(test '(a b c . d)
(append '(a b) '(c . d)))

(test 'a
(append '() 'a))

(test '(c b a)
(reverse '(a b c)))

(test '((e (f)) d (b c) a)
(reverse '(a (b c) d (e (f)))))

(define list-tail

  (lambda (x k)

    (if (zero? k)

        x

        (list-tail (cdr x) (- k 1)))))

(test 'c
(list-ref '(a b c d) 2))

(test 'c
(list-ref '(a b c d)

          (inexact->exact (round 1.8))))

(test '(a b c)
(memq 'a '(a b c)))

(test '(b c)
(memq 'b '(a b c)))

(test '#f
(memq 'a '(b c d)))

(test '#f
(memq (list 'a) '(b (a) c)))

(test '((a) c)
(member (list 'a)

'(b (a) c)))

(memq 101 '(100 101 102))

(test '(101 102)
(memv 101 '(100 101 102)))

(test '(a 1)
(begin (define e '((a 1) (b 2) (c 3)))

(assq 'a e)))

(test '(b 2)
(assq 'b e))

(test '#f
(assq 'd e))

(test '#f
(assq (list 'a) '(((a)) ((b)) ((c)))))

(test '((a))
(assoc (list 'a) '(((a)) ((b)) ((c)))))

(assq 5 '((2 3) (5 7) (11 13)))

(test '(5 7)
(assv 5 '((2 3) (5 7) (11 13))))

(test '#t
(symbol? 'foo))

(test '#t
(symbol? (car '(a b))))

(test '#f
(symbol? "bar"))

(test '#t
(symbol? 'nil))

(test '#f
(symbol? '()))

(test '#f
(symbol? #f))

(test '"flying-fish"
(symbol->string 'flying-fish))

(test '"martin"
(symbol->string 'Martin))

(test '"Malvina"
(symbol->string

   (string->symbol "Malvina")))

(test '#t
(eq? 'mISSISSIppi 'mississippi))

(test '#f
(eq? 'bitBlt (string->symbol "bitBlt")))

(test '#t
(eq? 'JollyWog

     (string->symbol

       (symbol->string 'JollyWog))))

(test '#t
(string=? "K. Harper, M.D."

          (symbol->string

            (string->symbol "K. Harper, M.D."))))

(begin (define (f) (make-string 3 #\*))

(define (g) "***")

(string-set! (f) 0 #\?))

(test '#(0 (2 2 2 2) "Anna")
'#(0 (2 2 2 2) "Anna"))

(test '#(a b c)
(vector 'a 'b 'c))

(test '8
(vector-ref '#(1 1 2 3 5 8 13 21)

            5))

(test '13
(vector-ref '#(1 1 2 3 5 8 13 21)

            (let ((i (round (* 2 (acos -1)))))

              (if (inexact? i)

                  (inexact->exact i)

                  i))))

(test '#(0 ("Sue" "Sue") "Anna")
(let ((vec (vector 0 '(2 2 2 2) "Anna")))

  (vector-set! vec 1 '("Sue" "Sue"))

  vec))

(test '(dah dah didah)
(vector->list '#(dah dah didah)))

(test '#(dididit dah)
(list->vector '(dididit dah)))

(test '#t
(procedure? car))

(test '#f
(procedure? 'car))

(test '#t
(procedure? (lambda (x) (* x x))))

(test '#f
(procedure? '(lambda (x) (* x x))))

(test '#t
(call-with-current-continuation procedure?))

(test '7
(apply + (list 3 4)))

(test '30
(begin (define compose

  (lambda (f g)

    (lambda args

      (f (apply g args)))))



((compose sqrt *) 12 75)))

(test '(b e h)
(map cadr '((a b) (d e) (g h))))

(test '(1 4 27 256 3125)
(map (lambda (n) (expt n n))

     '(1 2 3 4 5)))

(test '(5 7 9)
(map + '(1 2 3) '(4 5 6)))

(test '#(0 1 4 9 16)
(let ((v (make-vector 5)))

  (for-each (lambda (i)

              (vector-set! v i (* i i)))

            '(0 1 2 3 4))

v))

(test '3
(force (delay (+ 1 2))))

(test '(3 3)
(let ((p (delay (+ 1 2))))

  (list (force p) (force p))))

(test '2
(begin (define a-stream

  (letrec ((next

            (lambda (n)

              (cons n (delay (next (+ n 1)))))))

    (next 0)))

(define head car)

(define tail

  (lambda (stream) (force (cdr stream))))



(head (tail (tail a-stream)))))

(define count 0)

(define p

  (delay (begin (set! count (+ count 1))

                (if (> count x)

                    count

                    (force p)))))

(define x 5)

(test '6
(force p))

(test '6
(begin (set! x 10)

(force p)))

(define force

  (lambda (object)

    (object)))

(define-syntax delay

  (syntax-rules ()

    ((delay expression)

     (make-promise (lambda () expression)))))

(define make-promise

  (lambda (proc)

    (let ((result-ready? #f)

          (result #f))

      (lambda ()

        (if result-ready?

            result

            (let ((x (proc)))

              (if result-ready?

                  result

                  (begin (set! result-ready? #t)

                         (set! result x)

                         result))))))))

(eqv? (delay 1) 1)

(pair? (delay (cons 1 2)))

(test '-3
(call-with-current-continuation

  (lambda (exit)

    (for-each (lambda (x)

                (if (negative? x)

                    (exit x)))

              '(54 0 37 -3 245 19))

#t)))

(test '4
(begin (define list-length

  (lambda (obj)

    (call-with-current-continuation

      (lambda (return)

        (letrec ((r

                  (lambda (obj)

                    (cond ((null? obj) 0)

                          ((pair? obj)

                           (+ (r (cdr obj)) 1))

                          (else (return #f))))))

          (r obj))))))



(list-length '(1 2 3 4))))

(test '#f
(list-length '(a b . c)))

(define (values . things)

  (call-with-current-continuation

    (lambda (cont) (apply cont things))))

(test '5
(call-with-values (lambda () (values 4 5))

                  (lambda (a b) b)))

(test '-1
(call-with-values * -))

(test '(connect talk1 disconnect

connect talk2 disconnect)
(let ((path '())

      (c #f))

  (let ((add (lambda (s)

               (set! path (cons s path)))))

    (dynamic-wind

      (lambda () (add 'connect))

      (lambda ()

        (add (call-with-current-continuation

               (lambda (c0)

                 (set! c c0)

                 'talk1))))

      (lambda () (add 'disconnect)))

    (if (< (length path) 4)

        (c 'talk2)

        (reverse path)))))

(test '21
(eval '(* 7 3) (scheme-report-environment 5)))

(test '20
(let ((f (eval '(lambda (f x) (f x x))

               (null-environment 5))))

  (f + 10)))

(test-end)
