#!/usr/bin/env python3
"""Extract `expr ===> result` examples from the R5RS HTML report and emit a
Scheme test file. Usage: extract_r5rs_examples.py ch7.html ch8.html ch9.html > out.scm"""
import re, sys, html

def tt_blocks(text):
    # grab <tt>...</tt> regions; <tt> may nest (e.g. `<tt>#f</tt>` inside an
    # example block), so match tags with a depth counter instead of regex.
    for m in re.finditer(r'<tt>', text):
        depth = 1
        j = m.end()
        end = None
        while depth > 0:
            nxt = re.search(r'</?tt>', text[j:])
            if not nxt:
                break
            if nxt.group(0) == '<tt>':
                depth += 1
            else:
                depth -= 1
                if depth == 0:
                    end = j + nxt.start()
            j += nxt.end()
        if end is not None:
            yield text[m.end():end]

def block_to_lines(block):
    b = block.replace('&nbsp;', ' ')
    b = re.sub(r'<br\s*/?>', '\n', b)
    b = re.sub(r'<[^>]+>', '', b)
    b = html.unescape(b)
    return b.split('\n')

ARROW = re.compile(r'===>\s*(.*)$')

def extract(files):
    tests = []
    for f in files:
        text = open(f, encoding='utf-8', errors='replace').read()
        for block in tt_blocks(text):
            lines = block_to_lines(block)
            expr_lines = []
            i = 0
            while i < len(lines):
                line = lines[i]
                i += 1
                m = ARROW.search(line)
                if not m:
                    expr_lines.append(line)
                    continue
                before = line[:m.start()].strip()
                if before:
                    expr_lines.append(before)
                # prose periods glued to a closing paren (e.g. "...cdr x))).")
                # are sentence punctuation, not dotted-pair syntax; a real
                # dot is always space-delimited in R5RS.
                expr_lines = [re.sub(r'\)\.(?=\s|$)', ')', l) for l in expr_lines]
                result = m.group(1).strip()
                expr = '\n'.join(l.rstrip() for l in expr_lines).strip()
                expr_lines = []
                if not expr:
                    continue

                def salvage_defines():
                    # An example we skip may share its block with `define`s
                    # that later examples depend on (e.g. the delay/force
                    # examples before `p ===> a promise`); keep those.
                    for d in iter_data(expr):
                        if (d.startswith('(define')
                                and not re.search(r'<[a-zA-Z]', d)
                                and not d.startswith('(define define')):
                            tests.append((d, None))
                # skip error examples entirely
                if not result or 'error' in result or result.startswith(';'):
                    continue
                # examples with an "unspecified" result still matter for
                # their side effects (e.g. a `set!` between two tests):
                # emit the bare expression so sequencing is preserved.
                if 'unspecified' in result:
                    tests.append((expr, None))
                    continue
                # pseudo-examples referencing hypothetical objects/numbers
                if re.search(r'\bobj[12]\b|\bn(1|2|q|r|m)\b', expr):
                    salvage_defines()
                    continue
                # skip complex-number examples (complex tower not implemented)
                # and pseudo-examples that reference undefined variables.
                if re.search(r'[0-9)][-+][0-9./e]*i\b|[0-9][-+][0-9./e]*i\b|[-+]i\b', expr):
                    salvage_defines()
                    continue
                if re.search(r'make-rectangular|make-polar|real-part|imag-part'
                             r'|magnitude|angle|#[ei][-+0-9.]*i\b', expr):
                    salvage_defines()
                    continue
                if '|' in result or '|' in expr:
                    salvage_defines()
                    continue
                # known-corrupted or non-standard fragments from the report:
                # - char examples whose `#\` prefix was lost in extraction
                # - metavariable prose like (char<=? (integer->char x) ...)
                # - "implicit forcing", an optional extension, not the standard
                if ('char<=? a b' in expr or '(char->integer a)' in expr
                        or '(char->integer b)' in expr
                        or '(integer->char x)' in expr
                        or '(+ (delay' in expr
                        or '+:' in expr):
                    salvage_defines()
                    continue
                # a result datum may span several lines; keep consuming the
                # following lines until it balances into exactly one datum.
                while not parens_balance(result) and i < len(lines):
                    result += '\n' + lines[i].strip()
                    i += 1
                # result must be exactly one datum; phrases like
                # "a procedure" are informal descriptions, skip them.
                if not one_datum_p(result):
                    salvage_defines()
                    continue
                # formal grammar fragments with metasyntactic variables
                if re.search(r'<[a-zA-Z]', expr):
                    salvage_defines()
                    continue
                tests.append((expr, result))
            # Leftover lines with no arrow: complete top-level `define`s are
            # setup code shared by later examples (everything else is prose
            # or formal templates). Parse data one by one so blank lines
            # inside a definition don't split it.
            setup = '\n'.join(l.rstrip() for l in expr_lines).strip()
            setup = re.sub(r'\)\.(?=\s|$)', ')', setup)  # prose periods, see above
            for datum in iter_data(setup):
                if (datum.startswith('(define')
                        and not re.search(r'<[a-zA-Z]', datum)
                        and not datum.startswith('(define define')):
                    tests.append((datum, None))
    return tests


def parens_balance(src):
    """Cheap paren-balance check, ignoring strings and ; comments."""
    depth = 0
    i, n = 0, len(src)
    while i < n:
        c = src[i]
        if c == ';':
            while i < n and src[i] != '\n':
                i += 1
        elif c == '"':
            i += 1
            while i < n and src[i] != '"':
                i += 2 if src[i] == '\\' else 1
        elif c == '(':
            depth += 1
        elif c == ')':
            depth -= 1
            if depth < 0:
                return False
        i += 1
    return depth == 0


def _skip_ws(src, i):
    n = len(src)
    while i < n:
        c = src[i]
        if c.isspace(): i += 1
        elif c == ';':
            while i < n and src[i] != '\n': i += 1
        else: break
    return i

def _datum(src, i):
    """Parse one datum starting at/after i; return end index or None."""
    n = len(src)
    i = _skip_ws(src, i)
    if i >= n: return None
    c = src[i]
    if c in "'`,":
        j = i + 1
        if c == ',' and j < n and src[j] == '@': j += 1
        return _datum(src, j)
    if c == '(':
        j = i + 1
        while True:
            j = _skip_ws(src, j)
            if j >= n: return None
            if src[j] == ')': return j + 1
            j = _datum(src, j)
            if j is None: return None
    if c == '"':
        j = i + 1
        while j < n and src[j] != '"':
            j += 2 if src[j] == '\\' else 1
        return j + 1
    if c == '#':
        j = i + 1
        if j < n and src[j] == '(':
            return _datum(src, j)
        while j < n and not src[j].isspace() and src[j] not in '()': j += 1
        return j
    # atom
    j = i
    while j < n and not src[j].isspace() and src[j] not in "()',;`\"": j += 1
    return j

def one_datum_p(src):
    """True if src is exactly one datum (ignoring comments/whitespace)."""
    end = _datum(src, 0)
    if end is None: return False  # unparseable / unbalanced
    return _skip_ws(src, end) >= len(src)

def iter_data(src):
    """Yield the text of each complete top-level datum in src."""
    i = 0
    while True:
        i = _skip_ws(src, i)
        if i >= len(src): return
        end = _datum(src, i)
        if end is None: return
        if end == i:  # stray ')' etc. would make no progress; skip it
            i += 1
            continue
        yield src[i:end]
        i = end

PRELUDE = ''';; Auto-extracted from the R5RS report examples (chapters 4, 5, 6).
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

'''

def main():
    tests = extract(sys.argv[1:])
    out = [PRELUDE]
    for expr, result in tests:
        if not one_datum_p(expr):
            expr = '(begin ' + expr + ')'
        if result is None:
            out.append('%s\n' % expr)   # unspecified result: run for side effects
        else:
            out.append('(test \'%s\n%s)\n' % (result, expr))
    out.append('(test-end)\n')
    sys.stdout.write('\n'.join(out))

main()
