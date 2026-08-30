;;; mceval.scm -- The metacircular evaluator from SICP section 4.1
;;; ("The Metacircular Evaluator"), Structure and Interpretation of
;;; Computer Programs, Abelson & Sussman. SICP is licensed CC BY-SA 4.0.
;;;
;;; R5RS-clean single-file version. The guest language supports:
;;; self-evaluating atoms, variables, quote, set!, define, if, lambda,
;;; begin, cond (desugared to if), let (desugared to lambda), and
;;; procedure application. Environments are lists of frames; a frame is
;;; a pair of a variable list and a value list.

(define (mce:tagged-list? exp tag)
  (if (pair? exp) (eq? (car exp) tag) #f))

(define (mce:self-evaluating? exp)
  (cond ((number? exp) #t)
        ((string? exp) #t)
        ((boolean? exp) #t)
        (else #f)))

(define (mce:eval exp env)
  (cond ((mce:self-evaluating? exp) exp)
        ((symbol? exp) (mce:lookup-variable-value exp env))
        ((mce:tagged-list? exp 'quote) (cadr exp))
        ((mce:tagged-list? exp 'set!)
         (mce:eval-assignment exp env))
        ((mce:tagged-list? exp 'define)
         (mce:eval-definition exp env))
        ((mce:tagged-list? exp 'if)
         (mce:eval-if exp env))
        ((mce:tagged-list? exp 'lambda)
         (mce:make-procedure (cadr exp) (cddr exp) env))
        ((mce:tagged-list? exp 'begin)
         (mce:eval-sequence (cdr exp) env))
        ((mce:tagged-list? exp 'cond)
         (mce:eval (mce:cond->if (cdr exp)) env))
        ((mce:tagged-list? exp 'let)
         (mce:eval (mce:let->combination exp) env))
        ((pair? exp)
         (mce:apply (mce:eval (car exp) env)
                    (mce:list-of-values (cdr exp) env)))
        (else
         (error "mceval: unknown expression type" exp))))

(define (mce:apply procedure arguments)
  (cond ((mce:primitive-procedure? procedure)
         (mce:apply-primitive-procedure procedure arguments))
        ((mce:compound-procedure? procedure)
         (mce:eval-sequence
          (mce:procedure-body procedure)
          (mce:extend-environment
           (mce:procedure-parameters procedure)
           arguments
           (mce:procedure-environment procedure))))
        (else
         (error "mceval: unknown procedure type" procedure))))

(define (mce:list-of-values exps env)
  (if (null? exps)
      '()
      (cons (mce:eval (car exps) env)
            (mce:list-of-values (cdr exps) env))))

(define (mce:eval-if exp env)
  (if (mce:true? (mce:eval (cadr exp) env))
      (mce:eval (caddr exp) env)
      (mce:eval (cadddr exp) env)))

(define (mce:eval-sequence exps env)
  (cond ((null? (cdr exps)) (mce:eval (car exps) env))
        (else (mce:eval (car exps) env)
              (mce:eval-sequence (cdr exps) env))))

(define (mce:eval-assignment exp env)
  (mce:set-variable-value! (cadr exp)
                           (mce:eval (caddr exp) env)
                           env)
  'ok)

(define (mce:eval-definition exp env)
  (mce:define-variable! (mce:definition-variable exp)
                        (mce:eval (mce:definition-value exp) env)
                        env)
  'ok)

(define (mce:definition-variable exp)
  (if (symbol? (cadr exp))
      (cadr exp)
      (caadr exp)))

(define (mce:definition-value exp)
  (if (symbol? (cadr exp))
      (caddr exp)
      (cons 'lambda (cons (cdadr exp) (cddr exp)))))

(define (mce:true? x) (not (eq? x #f)))

;; cond -> if
(define (mce:cond->if clauses)
  (if (null? clauses)
      #f
      (let ((first (car clauses)))
        (if (eq? (car first) 'else)
            (cons 'begin (cdr first))
            (list 'if
                  (car first)
                  (cons 'begin (cdr first))
                  (mce:cond->if (cdr clauses)))))))

;; let -> lambda
(define (mce:let->combination exp)
  (let ((bindings (cadr exp))
        (body (cddr exp)))
    (cons (cons 'lambda
                (cons (map car bindings) body))
          (map cadr bindings))))

;; procedures
(define (mce:make-procedure parameters body env)
  (list 'procedure parameters body env))
(define (mce:compound-procedure? p)
  (mce:tagged-list? p 'procedure))
(define (mce:procedure-parameters p) (cadr p))
(define (mce:procedure-body p) (caddr p))
(define (mce:procedure-environment p) (cadddr p))

;; environments: a list of frames; frame = (vars . vals)
(define (mce:enclosing-environment env) (cdr env))
(define (mce:first-frame env) (car env))
(define mce:the-empty-environment '())

(define (mce:make-frame variables values)
  (cons variables values))
(define (mce:frame-variables frame) (car frame))
(define (mce:frame-values frame) (cdr frame))
(define (mce:add-binding-to-frame! var val frame)
  (set-car! frame (cons var (car frame)))
  (set-cdr! frame (cons val (cdr frame))))

(define (mce:extend-environment vars vals base-env)
  (if (= (length vars) (length vals))
      (cons (mce:make-frame vars vals) base-env)
      (error "mceval: argument count mismatch" (cons vars vals))))

(define (mce:lookup-variable-value var env)
  (define (env-loop env)
    (define (scan vars vals)
      (cond ((null? vars)
             (env-loop (mce:enclosing-environment env)))
            ((eq? var (car vars)) (car vals))
            (else (scan (cdr vars) (cdr vals)))))
    (if (eq? env mce:the-empty-environment)
        (error "mceval: unbound variable" var)
        (let ((frame (mce:first-frame env)))
          (scan (mce:frame-variables frame)
                (mce:frame-values frame)))))
  (env-loop env))

(define (mce:set-variable-value! var val env)
  (define (env-loop env)
    (define (scan vars vals)
      (cond ((null? vars)
             (env-loop (mce:enclosing-environment env)))
            ((eq? var (car vars)) (set-car! vals val))
            (else (scan (cdr vars) (cdr vals)))))
    (if (eq? env mce:the-empty-environment)
        (error "mceval: unbound variable -- set!" var)
        (let ((frame (mce:first-frame env)))
          (scan (mce:frame-variables frame)
                (mce:frame-values frame)))))
  (env-loop env))

(define (mce:define-variable! var val env)
  (let ((frame (mce:first-frame env)))
    (define (scan vars vals)
      (cond ((null? vars)
             (mce:add-binding-to-frame! var val frame))
            ((eq? var (car vars)) (set-car! vals val))
            (else (scan (cdr vars) (cdr vals)))))
    (scan (mce:frame-variables frame)
          (mce:frame-values frame))))

;; primitive procedures: (primitive <host-procedure>)
(define (mce:primitive-procedure? proc)
  (mce:tagged-list? proc 'primitive))
(define (mce:primitive-implementation proc) (cadr proc))
(define (mce:apply-primitive-procedure proc args)
  (apply (mce:primitive-implementation proc) args))

(define mce:primitive-procedures
  (list (list 'car car)
        (list 'cdr cdr)
        (list 'cons cons)
        (list 'null? null?)
        (list 'list list)
        (list 'pair? pair?)
        (list 'eq? eq?)
        (list 'equal? equal?)
        (list 'not not)
        (list 'length length)
        (list 'append append)
        (list 'reverse reverse)
        (list 'memq memq)
        (list 'assq assq)
        (list '+ +)
        (list '- -)
        (list '* *)
        (list '/ /)
        (list '= =)
        (list '< <)
        (list '> >)
        (list '<= <=)
        (list '>= >=)
        (list 'zero? zero?)
        ;; NB: SICP/MIT-Scheme 的 `1+'/`-1+' 不是合法 R5RS 标识符
        ;; （标识符不能以数字开头），改名为 inc/dec。
        (list 'inc (lambda (x) (+ x 1)))
        (list 'dec (lambda (x) (- x 1)))
        (list 'remainder remainder)
        (list 'display display)
        (list 'newline newline)))

(define (mce:setup-environment)
  (let ((initial-env
         (mce:extend-environment
          (map car mce:primitive-procedures)
          (map (lambda (p) (list 'primitive (cadr p)))
               mce:primitive-procedures)
          mce:the-empty-environment)))
    (mce:define-variable! '#t #t initial-env)
    (mce:define-variable! '#f #f initial-env)
    initial-env))

;; Convenience: evaluate a sequence of guest expressions in a fresh
;; global environment, returning the last value.
(define (mce:interpret exps)
  (let ((env (mce:setup-environment)))
    (mce:eval-sequence exps env)))

;;; ===== driver (added by scheme-rs; not part of the original text) =====
(display (mce:interpret '(
  (define (fact n) (if (= n 0) 1 (* n (fact (- n 1)))))
  (fact 10)))) (newline)                        ; ==> 3628800
(display (mce:interpret '(
  (define (fib n) (cond ((< n 2) n) (else (+ (fib (- n 1)) (fib (- n 2))))))
  (fib 12)))) (newline)                        ; ==> 144
(display (mce:interpret '(
  (define (make-adder n) (lambda (x) (+ x n)))
  ((make-adder 5) 37)))) (newline)             ; ==> 42
(display (mce:interpret '(
  (define (map2 f xs) (if (null? xs) (quote ()) (cons (f (car xs)) (map2 f (cdr xs)))))
  (map2 (lambda (x) (* x x)) (quote (1 2 3 4)))))) (newline)   ; ==> (1 4 9 16)
(display (mce:interpret '((let ((a 1) (b 2)) (+ a b))))) (newline)   ; ==> 3
;;; guest-level tail recursion is powered by the host's proper tail calls:
;;; eval-sequence tail-calls into the last expression's eval.
(display (mce:interpret '(
  (define (loop n) (if (= n 0) (quote done) (loop (- n 1))))
  (loop 5000)))) (newline)                     ; ==> done
