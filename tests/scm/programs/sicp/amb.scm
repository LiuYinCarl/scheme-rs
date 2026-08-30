;;; amb.scm -- The amb nondeterministic evaluator from SICP section 4.3
;;; ("Nondeterministic Computing"), Structure and Interpretation of
;;; Computer Programs, Abelson & Sussman. SICP is licensed CC BY-SA 4.0.
;;;
;;; The evaluator CPS-es every guest expression: (analyze exp) returns an
;;; execution procedure of three arguments (env succeed fail). Backtracking
;;; is done by invoking the failure continuation, which is just a closure —
;;; this is a heavy stress test of first-class procedures, environments and
;;; (indirectly) the host's tail calls. A direct call/cc implementation of
;;; amb is included at the end for contrast.

(define (amb:tagged-list? exp tag)
  (if (pair? exp) (eq? (car exp) tag) #f))

(define (amb:ambeval exp env succeed fail)
  ((amb:analyze exp) env succeed fail))

(define (amb:analyze exp)
  (cond ((or (number? exp) (string? exp) (boolean? exp))
         (lambda (env succeed fail) (succeed exp fail)))
        ((symbol? exp)
         (lambda (env succeed fail)
           (succeed (amb:lookup-variable-value exp env) fail)))
        ((amb:tagged-list? exp 'quote)
         (let ((qval (cadr exp)))
           (lambda (env succeed fail) (succeed qval fail))))
        ((amb:tagged-list? exp 'lambda)
         (let ((vars (cadr exp)) (bproc (amb:analyze-sequence (cddr exp))))
           (lambda (env succeed fail)
             (succeed (list 'procedure vars bproc env) fail))))
        ((amb:tagged-list? exp 'amb)
         (amb:analyze-amb (cdr exp)))
        ((amb:tagged-list? exp 'define)
         (amb:analyze-definition exp))
        ((amb:tagged-list? exp 'set!)
         (amb:analyze-assignment exp))
        ((amb:tagged-list? exp 'if)
         (amb:analyze-if exp))
        ((amb:tagged-list? exp 'begin)
         (amb:analyze-sequence (cdr exp)))
        ((amb:tagged-list? exp 'let)
         (amb:analyze (amb:let->combination exp)))
        ((amb:tagged-list? exp 'cond)
         (amb:analyze (amb:cond->if (cdr exp))))
        ((pair? exp)
         (amb:analyze-application exp))
        (else
         (error "amb: unknown expression type" exp))))

(define (amb:let->combination exp)
  (let ((bindings (cadr exp)) (body (cddr exp)))
    (cons (cons 'lambda (cons (map car bindings) body))
          (map cadr bindings))))

(define (amb:cond->if clauses)
  (if (null? clauses)
      #f
      (let ((first (car clauses)))
        (if (eq? (car first) 'else)
            (cons 'begin (cdr first))
            (list 'if (car first)
                  (cons 'begin (cdr first))
                  (amb:cond->if (cdr clauses)))))))

(define (amb:analyze-amb choices)
  (let ((cprocs (map amb:analyze choices)))
    (lambda (env succeed fail)
      (define (try-next choices)
        (if (null? choices)
            (fail)
            ((car choices) env succeed
                            (lambda () (try-next (cdr choices))))))
      (try-next cprocs))))

(define (amb:analyze-definition exp)
  (let ((var (if (symbol? (cadr exp)) (cadr exp) (caadr exp)))
        (vproc (amb:analyze
                (if (symbol? (cadr exp))
                    (caddr exp)
                    (cons 'lambda (cons (cdadr exp) (cddr exp)))))))
    (lambda (env succeed fail)
      (vproc env
             (lambda (val fail2)
               (amb:define-variable! var val env)
               (succeed 'ok fail2))
             fail))))

(define (amb:analyze-assignment exp)
  (let ((var (cadr exp))
        (vproc (amb:analyze (caddr exp))))
    (lambda (env succeed fail)
      (vproc env
             (lambda (val fail2)
               (let ((old-value (amb:lookup-variable-value var env)))
                 (amb:set-variable-value! var val env)
                 (succeed 'ok
                          (lambda ()
                            (amb:set-variable-value! var old-value env)
                            (fail2)))))
             fail))))

(define (amb:analyze-if exp)
  (let ((pproc (amb:analyze (cadr exp)))
        (cproc (amb:analyze (caddr exp)))
        (aproc (amb:analyze (cadddr exp))))
    (lambda (env succeed fail)
      (pproc env
             (lambda (pred-value fail2)
               (if (amb:true? pred-value)
                   (cproc env succeed fail2)
                   (aproc env succeed fail2)))
             fail))))

(define (amb:true? x) (not (eq? x #f)))

(define (amb:analyze-sequence exps)
  (define (sequentially a b)
    (lambda (env succeed fail)
      (a env (lambda (a-value fail2) (b env succeed fail2)) fail)))
  (define (loop first-proc rest-procs)
    (if (null? rest-procs)
        first-proc
        (loop (sequentially first-proc (car rest-procs))
              (cdr rest-procs))))
  (let ((procs (map amb:analyze exps)))
    (if (null? procs)
        (error "amb: empty sequence")
        (loop (car procs) (cdr procs)))))

(define (amb:analyze-application exp)
  (let ((fproc (amb:analyze (car exp)))
        (aprocs (map amb:analyze (cdr exp))))
    (lambda (env succeed fail)
      (fproc env
             (lambda (proc fail2)
               (amb:get-args aprocs env
                             (lambda (args fail3)
                               (amb:execute-application proc args succeed fail3))
                             fail2))
             fail))))

(define (amb:get-args aprocs env succeed fail)
  (if (null? aprocs)
      (succeed '() fail)
      ((car aprocs) env
                   (lambda (arg fail2)
                     (amb:get-args (cdr aprocs) env
                                   (lambda (args fail3)
                                     (succeed (cons arg args) fail3))
                                   fail2))
                   fail)))

(define (amb:execute-application proc args succeed fail)
  (cond ((amb:tagged-list? proc 'primitive)
         (succeed (apply (cadr proc) args) fail))
        ((amb:tagged-list? proc 'procedure)
         ((caddr proc)
          (amb:extend-environment (cadr proc) args (cadddr proc))
          succeed
          fail))
        (else
         (error "amb: unknown procedure type" proc))))

;; environments (same representation as the SICP mceval)
(define (amb:make-frame variables values) (cons variables values))
(define (amb:extend-environment vars vals base-env)
  (if (= (length vars) (length vals))
      (cons (amb:make-frame vars vals) base-env)
      (error "amb: argument count mismatch" (cons vars vals))))

(define (amb:lookup-variable-value var env)
  (define (env-loop env)
    (define (scan vars vals)
      (cond ((null? vars) (env-loop (cdr env)))
            ((eq? var (car vars)) (car vals))
            (else (scan (cdr vars) (cdr vals)))))
    (if (null? env)
        (error "amb: unbound variable" var)
        (let ((frame (car env)))
          (scan (car frame) (cdr frame)))))
  (env-loop env))

(define (amb:set-variable-value! var val env)
  (define (env-loop env)
    (define (scan vars vals)
      (cond ((null? vars) (env-loop (cdr env)))
            ((eq? var (car vars)) (set-car! vals val))
            (else (scan (cdr vars) (cdr vals)))))
    (if (null? env)
        (error "amb: unbound variable -- set!" var)
        (let ((frame (car env)))
          (scan (car frame) (cdr frame)))))
  (env-loop env))

(define (amb:define-variable! var val env)
  (let ((frame (car env)))
    (define (scan vars vals)
      (cond ((null? vars)
             (set-car! frame (cons var (car frame)))
             (set-cdr! frame (cons val (cdr frame))))
            ((eq? var (car vars)) (set-car! vals val))
            (else (scan (cdr vars) (cdr vals)))))
    (scan (car frame) (cdr frame))))

(define amb:primitive-procedures
  (list (list 'car car) (list 'cdr cdr) (list 'cons cons)
        (list 'null? null?) (list 'list list) (list 'pair? pair?)
        (list 'not not) (list 'eq? eq?) (list 'equal? equal?)
        (list 'member member) (list 'memq memq)
        (list '+ +) (list '- -) (list '* *) (list '/ /)
        (list '= =) (list '< <) (list '> >)
        (list '<= <=) (list '>= >=) (list 'zero? zero?)
        (list 'even? even?) (list 'remainder remainder)
        (list 'display display) (list 'newline newline)))

(define (amb:setup-environment)
  (let ((env (amb:extend-environment
              (map car amb:primitive-procedures)
              (map (lambda (p) (list 'primitive (cadr p)))
                   amb:primitive-procedures)
              '())))
    (amb:define-variable! '#t #t env)
    (amb:define-variable! '#f #f env)
    env))

;; Evaluate a guest expression, returning its first solution (or 'failed).
(define (amb:interpret exp)
  (amb:ambeval exp
               (amb:setup-environment)
               (lambda (value fail) value)
               (lambda () 'failed)))

;; Evaluate a guest expression, returning the list of ALL solutions.
(define (amb:interpret-all exp)
  (let ((results '()))
    (amb:ambeval exp
                 (amb:setup-environment)
                 (lambda (value fail)
                   (set! results (cons value results))
                   (fail))
                 (lambda () (reverse results)))))

;;; ===== driver (added by scheme-rs; not part of the original text) =====
;;; SICP 4.3.3's prime-sum-pair: first solution is (3 20).
(display (amb:interpret '(begin
  (define (require p) (if (not p) (amb) #t))
  (define (an-element-of items)
    (require (not (null? items)))
    (amb (car items) (an-element-of (cdr items))))
  (define (prime? n)
    (define (smallest-divisor n)
      (define (find-divisor test)
        (cond ((> (* test test) n) n)
              ((= (remainder n test) 0) test)
              (else (find-divisor (+ test 1)))))
      (find-divisor 2))
    (= n (smallest-divisor n)))
  (let ((a (an-element-of (quote (1 3 5 8))))
        (b (an-element-of (quote (20 35 110)))))
    (require (prime? (+ a b)))
    (list a b))))) (newline)
;;; enumerating all solutions exercises backtracking through every choice
(display (amb:interpret-all '(amb 1 2 3 4))) (newline)
(display (amb:interpret '(begin
  (define (require p) (if (not p) (amb) #t))
  (define (an-element-of items)
    (require (not (null? items)))
    (amb (car items) (an-element-of (cdr items))))
  (let ((a (an-element-of (quote (1 2 3))))
        (b (an-element-of (quote (4 5 6)))))
    (require (= (+ a b) 7))
    (list a b))))) (newline)
