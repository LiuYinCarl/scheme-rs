//! syntax-rules: pattern matching (with nested ellipses), template expansion,
//! and hygiene via renaming of template-introduced identifiers.
//!
//! # 设计要点
//!
//! 展开一个宏分三步：
//! 1. 模式匹配（match_pat/match_seq）：把使用处 form 按 pattern 的结构
//!    分解，收集"模式变量 → 匹配到的子形式"的绑定。字面量按
//!    free-identifier=? 比较（看绑定而不是名字），`_` 匹配一切。
//! 2. ellipsis：模式里 `x ...` 表示 x 可重复，匹配结果是每层 ellipsis
//!    套一层的 Match::Many 树；零次重复也必须把模式变量绑定为空序列
//!    （否则模板里的 `x ...` 会找不到变量）。模板里 `x ...`（可多个
//!    ellipsis 叠加以对应嵌套深度）按同一索引迭代展开。
//! 3. 模板展开（expand_tmpl）：模式变量原样替换；其余标识符是"宏引入
//!    的"，要做卫生重命名（rename_sym，记录原名与定义处环境）。例外是
//!    `(quote ...)` 内部——那里面的标识符是数据不是代码，必须保持字面
//!    不变；`(... ...)` 是转义写法，表示原样输出省略号。
//!
//! 辅助语法（else、=>、unquote、ellipsis 本身）在使用处被重新绑定后就
//! 失去特殊含义，判定方式是 env.rs 的 aux_name：沿重命名链找回原始
//! 名字，同时确认它在当前环境没有被局部绑定。

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::env::{free_id_eq, is_locally_bound, Env, Meaning};
use crate::printer::write_to_string;
use crate::value::{cons, intern, list_to_vec, rename_sym, scm_eqv, sym_str, Sym, Value};

pub struct Transformer {
    pub ellipsis: Sym,
    pub literals: Vec<Sym>,
    pub rules: Vec<(Value, Value)>,
    pub def_env: Rc<Env>,
}

#[derive(Clone)]
enum Match {
    One(Value),
    Many(Vec<Match>),
}

type Bindings = HashMap<Sym, Match>;

/// 模式匹配的只读上下文：使用处环境 + 生效的 ellipsis 标识符。
struct MatchCtx<'a> {
    use_env: &'a Rc<Env>,
    ell: Option<Sym>,
}

pub fn parse_transformer(spec: &Value, def_env: &Rc<Env>) -> Result<Rc<Transformer>, String> {
    let items =
        crate::value::proper_list(spec).ok_or_else(|| "syntax-rules: malformed".to_string())?;
    if items.len() < 2 {
        return Err("syntax-rules: malformed".into());
    }
    // items[0] is `syntax-rules` (possibly renamed); optional custom ellipsis
    let mut idx = 1;
    let mut ellipsis = intern("...");
    if let Value::Symbol(s) = &items[1] {
        ellipsis = *s;
        idx = 2;
    }
    if items.len() < idx + 1 {
        return Err("syntax-rules: missing literals".into());
    }
    let lit_list = crate::value::proper_list(&items[idx])
        .ok_or_else(|| "syntax-rules: bad literals".to_string())?;
    let mut literals = Vec::new();
    for l in lit_list {
        match l {
            Value::Symbol(s) => literals.push(s),
            _ => return Err("syntax-rules: literal must be identifier".into()),
        }
    }
    let mut rules = Vec::new();
    for r in &items[idx + 1..] {
        let rule =
            crate::value::proper_list(r).ok_or_else(|| "syntax-rules: bad rule".to_string())?;
        if rule.len() != 2 {
            return Err("syntax-rules: rule must be (pattern template)".into());
        }
        rules.push((rule[0].clone(), rule[1].clone()));
    }
    Ok(Rc::new(Transformer {
        ellipsis,
        literals,
        rules,
        def_env: def_env.clone(),
    }))
}

fn split_list(v: &Value) -> (Vec<Value>, Value) {
    match list_to_vec(v) {
        Some((items, rest)) => (items, rest),
        None => (Vec::new(), v.clone()),
    }
}

impl Transformer {
    /// Decide whether the ellipsis identifier is effective at the use site.
    /// If the user has locally rebound it, it loses its special meaning.
    fn effective_ellipsis(&self, use_env: &Rc<Env>) -> Option<Sym> {
        if is_locally_bound(use_env, &sym_str(self.ellipsis)) {
            None
        } else {
            Some(self.ellipsis)
        }
    }

    fn match_pat(&self, pat: &Value, form: &Value, b: &mut Bindings, ctx: &MatchCtx) -> bool {
        match pat {
            Value::Symbol(s) => {
                if *s == intern("_") {
                    return true;
                }
                if self.literals.contains(s) {
                    if let Value::Symbol(fs) = form {
                        return free_id_eq(*s, &self.def_env, *fs, ctx.use_env);
                    }
                    return false;
                }
                b.insert(*s, Match::One(form.clone()));
                true
            }
            Value::Nil => matches!(form, Value::Nil),
            Value::Vector(pv) => {
                let pv = pv.borrow().clone();
                let fv = match form {
                    Value::Vector(fv) => fv.borrow().clone(),
                    _ => return false,
                };
                self.match_seq(&pv, &fv, &Value::Nil, &Value::Nil, b, ctx)
            }
            Value::Pair(_) => {
                let (pats, rest_pat) = split_list(pat);
                let (forms, rest_form) = match list_to_vec(form) {
                    Some(x) => x,
                    None => return false,
                };
                self.match_seq(&pats, &forms, &rest_pat, &rest_form, b, ctx)
            }
            _ => scm_eqv(pat, form),
        }
    }

    /// Match a sequence of subpatterns against a sequence of forms, then
    /// rest_pat against rest_form. Handles one ellipsis at this level.
    fn match_seq(
        &self,
        pats: &[Value],
        forms: &[Value],
        rest_pat: &Value,
        rest_form: &Value,
        b: &mut Bindings,
        ctx: &MatchCtx,
    ) -> bool {
        // find first top-level ellipsis: pats[i+1] == ell
        let ell = ctx.ell;
        let ell_pos = ell.and_then(|e| {
            (0..pats.len().saturating_sub(1))
                .find(|&i| matches!(&pats[i + 1], Value::Symbol(s) if *s == e))
        });
        if let Some(pos) = ell_pos {
            let after = &pats[pos + 2..];
            // `after` must not contain further top-level ellipses (unsupported)
            if forms.len() < pos + after.len() {
                return false;
            }
            let n_rep = forms.len() - pos - after.len();
            for i in 0..pos {
                if !self.match_pat(&pats[i], &forms[i], b, ctx) {
                    return false;
                }
            }
            let mut reps: Vec<Bindings> = Vec::with_capacity(n_rep);
            for i in 0..n_rep {
                let mut bi = Bindings::new();
                if !self.match_pat(&pats[pos], &forms[pos + i], &mut bi, ctx) {
                    return false;
                }
                reps.push(bi);
            }
            // merge repeated bindings into Many; seed with the subpattern's
            // own pattern variables so that zero repetitions still bind them
            // (to empty sequences)
            let mut keys: Vec<Sym> = Vec::new();
            self.collect_pat_vars(&pats[pos], ell, &mut keys);
            for bi in &reps {
                for k in bi.keys() {
                    if !keys.contains(k) {
                        keys.push(*k);
                    }
                }
            }
            for k in keys {
                let collected: Vec<Match> = reps
                    .iter()
                    .map(|bi| {
                        bi.get(&k)
                            .cloned()
                            .unwrap_or(Match::One(Value::Unspecified))
                    })
                    .collect();
                b.insert(k, Match::Many(collected));
            }
            for (j, ap) in after.iter().enumerate() {
                if !self.match_pat(ap, &forms[pos + n_rep + j], b, ctx) {
                    return false;
                }
            }
            self.match_pat(rest_pat, rest_form, b, ctx)
        } else {
            if forms.len() != pats.len() {
                return false;
            }
            for (p, f) in pats.iter().zip(forms.iter()) {
                if !self.match_pat(p, f, b, ctx) {
                    return false;
                }
            }
            self.match_pat(rest_pat, rest_form, b, ctx)
        }
    }

    fn match_rule(&self, pat: &Value, form: &Value, b: &mut Bindings, ctx: &MatchCtx) -> bool {
        // first element of the pattern matches the keyword position: anything
        if let (Value::Pair(pp), Value::Pair(fp)) = (pat, form) {
            let (pc, fc) = {
                let (pb, fb) = (pp.borrow(), fp.borrow());
                (pb.1.clone(), fb.1.clone())
            };
            self.match_pat(&pc, &fc, b, ctx)
        } else {
            false
        }
    }

    /// Collect the pattern variables occurring in a (sub)pattern: symbols
    /// that are neither literals, `_`, nor the ellipsis.
    fn collect_pat_vars(&self, pat: &Value, ell: Option<Sym>, out: &mut Vec<Sym>) {
        match pat {
            Value::Symbol(s) => {
                if *s != intern("_")
                    && !self.literals.contains(s)
                    && Some(*s) != ell
                    && !out.contains(s)
                {
                    out.push(*s);
                }
            }
            Value::Pair(_) => {
                let (items, rest) = split_list(pat);
                for it in items {
                    self.collect_pat_vars(&it, ell, out);
                }
                self.collect_pat_vars(&rest, ell, out);
            }
            Value::Vector(v) => {
                for it in v.borrow().iter() {
                    self.collect_pat_vars(it, ell, out);
                }
            }
            _ => {}
        }
    }
}

struct Expander<'a> {
    t: &'a Transformer,
    ell: Option<Sym>,
    renames: HashMap<Sym, Sym>,
}

impl<'a> Expander<'a> {
    fn rename(&mut self, s: Sym) -> Sym {
        if let Some(f) = self.renames.get(&s) {
            return *f;
        }
        let fresh = rename_sym(s, &self.t.def_env);
        self.renames.insert(s, fresh);
        fresh
    }

    fn expand_tmpl(
        &mut self,
        tmpl: &Value,
        b: &Bindings,
        escape: bool,
        data: bool,
    ) -> Result<Value, String> {
        match tmpl {
            Value::Symbol(s) => {
                if let Some(m) = b.get(s) {
                    return match m {
                        Match::One(v) => Ok(v.clone()),
                        Match::Many(_) => Err(format!(
                            "syntax-rules: missing ellipsis for {}",
                            sym_str(*s)
                        )),
                    };
                }
                if data {
                    // quoted datum: introduced identifiers stay literal
                    return Ok(tmpl.clone());
                }
                if Some(*s) == self.ell {
                    if escape {
                        // literal ellipsis produced by (... ...)
                        return Ok(tmpl.clone());
                    }
                    return Err("syntax-rules: stray ellipsis in template".into());
                }
                Ok(Value::Symbol(self.rename(*s)))
            }
            Value::Pair(_) => {
                let (items, rest) = split_list(tmpl);
                // (quote <datum>) : identifiers in the datum are data
                if !data {
                    if let Some(Value::Symbol(q)) = items.first() {
                        if sym_str(*q) == "quote"
                            && !b.contains_key(q)
                            && items.len() == 2
                            && rest.is_nil()
                        {
                            let datum = self.expand_tmpl(&items[1], b, false, true)?;
                            return Ok(crate::value::list_from_vec(vec![
                                Value::Symbol(self.rename(*q)),
                                datum,
                            ]));
                        }
                    }
                }
                // (... <template>) escape
                if let Some(e) = self.ell {
                    if !escape {
                        if let Some(Value::Symbol(s0)) = items.first() {
                            if *s0 == e && items.len() == 2 && rest.is_nil() {
                                return self.expand_tmpl(&items[1], b, true, data);
                            }
                        }
                    }
                }
                let mut out: Vec<Value> = Vec::new();
                let mut i = 0;
                while i < items.len() {
                    // count ellipses following items[i]
                    let mut k = 0;
                    if !escape {
                        if let Some(e) = self.ell {
                            while i + 1 + k < items.len() {
                                if matches!(&items[i + 1 + k], Value::Symbol(s) if *s == e) {
                                    k += 1;
                                } else {
                                    break;
                                }
                            }
                        }
                    }
                    if k > 0 {
                        let expanded = self.expand_ell(&items[i], b, k, data)?;
                        out.extend(expanded);
                        i += 1 + k;
                    } else {
                        out.push(self.expand_tmpl(&items[i], b, escape, data)?);
                        i += 1;
                    }
                }
                let rest_out = if rest.is_nil() {
                    Value::Nil
                } else {
                    self.expand_tmpl(&rest, b, escape, data)?
                };
                let mut result = rest_out;
                for x in out.into_iter().rev() {
                    result = cons(x, result);
                }
                Ok(result)
            }
            Value::Vector(items) => {
                let as_list = crate::value::list_from_vec(items.borrow().clone());
                let expanded = self.expand_tmpl(&as_list, b, escape, data)?;
                let v = crate::value::proper_list(&expanded)
                    .ok_or_else(|| "syntax-rules: bad vector template".to_string())?;
                Ok(Value::Vector(Rc::new(std::cell::RefCell::new(v))))
            }
            other => Ok(other.clone()),
        }
    }

    /// Expand `item` under k ellipses, producing a flat list of expansions.
    fn expand_ell(
        &mut self,
        item: &Value,
        b: &Bindings,
        k: usize,
        data: bool,
    ) -> Result<Vec<Value>, String> {
        // pattern vars of item bound to Many at this level
        let mut vars: Vec<Sym> = Vec::new();
        let mut seen: HashSet<Sym> = HashSet::new();
        collect_ell_vars(item, b, &mut vars, &mut seen);
        if vars.is_empty() {
            return Err(format!(
                "syntax-rules: too many ellipsis (no repeatable var in {})",
                write_to_string(item)
            ));
        }
        let n = match b.get(&vars[0]) {
            Some(Match::Many(v)) => v.len(),
            _ => 0,
        };
        let mut out = Vec::new();
        for i in 0..n {
            let mut sub: Bindings = Bindings::new();
            for (key, m) in b {
                let nm = match m {
                    Match::Many(v) => v
                        .get(i)
                        .cloned()
                        .ok_or_else(|| "syntax-rules: ellipsis length mismatch".to_string())?,
                    one => one.clone(),
                };
                sub.insert(*key, nm);
            }
            if k == 1 {
                out.push(self.expand_tmpl(item, &sub, false, data)?);
            } else {
                out.extend(self.expand_ell(item, &sub, k - 1, data)?);
            }
        }
        Ok(out)
    }
}

/// Collect symbols in `v` that are bound to Match::Many in `b`.
fn collect_ell_vars(v: &Value, b: &Bindings, vars: &mut Vec<Sym>, seen: &mut HashSet<Sym>) {
    match v {
        Value::Symbol(s) => {
            if matches!(b.get(s), Some(Match::Many(_))) && seen.insert(*s) {
                vars.push(*s);
            }
        }
        Value::Pair(_) => {
            let (items, rest) = split_list(v);
            for x in items {
                collect_ell_vars(&x, b, vars, seen);
            }
            collect_ell_vars(&rest, b, vars, seen);
        }
        Value::Vector(items) => {
            for x in items.borrow().iter() {
                collect_ell_vars(x, b, vars, seen);
            }
        }
        _ => {}
    }
}

pub fn expand(t: &Rc<Transformer>, form: &Value, use_env: &Rc<Env>) -> Result<Value, String> {
    let ell = t.effective_ellipsis(use_env);
    let ctx = MatchCtx { use_env, ell };
    for (pat, tmpl) in &t.rules {
        let mut b = Bindings::new();
        if t.match_rule(pat, form, &mut b, &ctx) {
            let mut ex = Expander {
                t,
                ell,
                renames: HashMap::new(),
            };
            return ex.expand_tmpl(tmpl, &b, false, false);
        }
    }
    Err(format!(
        "syntax-rules: no matching clause for {}",
        write_to_string(form)
    ))
}

/// Is this form a use of a macro (head resolves to a macro)?
pub fn macro_head(env: &Rc<Env>, form: &Value) -> Option<(Rc<Transformer>, Sym)> {
    if let Value::Pair(p) = form {
        let head = p.borrow().0.clone();
        if let Value::Symbol(s) = head {
            if let Meaning::Macro(t) = crate::env::resolve(env, s) {
                return Some((t, s));
            }
        }
    }
    None
}
