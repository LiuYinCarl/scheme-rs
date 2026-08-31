# 宏系统：syntax-rules 与重命名式卫生

实现位于 `src/syntax_rules.rs`（解析、匹配、展开）与 `src/env.rs`
（rename 感知的解析）。本文按数据流讲：一个宏调用如何变成普通代码。

## 三步流程

```
(syntax-rules (lits...) ((pat tmpl) ...))
        │  parse_transformer (syntax_rules.rs:37)
        ▼
Transformer { ellipsis, literals, rules, def_env }   ← 记住定义处环境
        │  遇到宏调用 (foo a b c)：expand (syntax_rules.rs:460)
        ▼
1. 模式匹配  match_pat / match_seq (syntax_rules.rs:111,149)
   ─ 按 pattern 结构分解使用处 form，收集 "模式变量 → 子形式" 绑定
2. 模板展开  expand_tmpl (syntax_rules.rs:285)
   ─ 模式变量原位替换；其余标识符做卫生重命名
        ▼
展开后的 form，回到求值器继续 eval
```

Transformer 在 `define-syntax`/`let-syntax` 求值时创建，`def_env` 记录
宏定义处的环境——这是卫生的锚点。

## 模式匹配的细节

- 规则的 pattern 第一个元素匹配关键字位置（任意东西都行），所以从
  cdr 开始比（`match_rule`）。
- 字面量按 **free-identifier=?** 比较（`free_id_eq`，env.rs）：比的是
  "绑定"而不是"名字"——宏定义处的 `else` 与使用处被局部绑定的 `else`
  不是同一个绑定，因此不该按字面量命中。
- `_` 匹配一切且不产生绑定。
- 非字面量符号就是模式变量；嵌套 list/vector 结构递归匹配；点对尾
  （`rest_pat`）匹配剩余部分。

## ellipsis：深度、嵌套与零次重复

绑定值是一棵 `Match::One / Match::Many` 树：模式里 `x ...` 每多一层
ellipsis，匹配结果就多套一层 `Many`。模板里 `x ...`（或 `x ... ...`）
由 `expand_ell`（syntax_rules.rs:392）按同一索引迭代展开：把每个
`Many` 绑定"下降一层"后递归展开子模板。`(... ...)` 是转义写法，原样
输出省略号（R5RS 4.3.2 的 be-like-begin 例子走这条路）。

**零次重复也必须绑定**：`(when test stmt1 stmt2 ...)` 匹配
`(when if (set! if 'now))` 时 `stmt2` 重复 0 次。若此时不给 `stmt2`
任何绑定，模板里的 `stmt2 ...` 会报"没有可重复的变量"。修法是在合并
重复绑定前，先用 `collect_pat_vars`（syntax_rules.rs:241）把被重复
子模式自身的模式变量预填进 keys——零次重复就绑成空的 `Many([])`。
自定义省略号（`(syntax-rules ::: () ...)`）与"使用处把 `...` 绑成了
普通变量"（此时它失去特殊含义，按普通模式变量处理）都已支持。

## 重命名式卫生的三条原则

1. **模板引入的标识符一律重命名**：展开时，凡不是模式变量的标识符
   都替换成 fresh 符号（`rename_sym`，value.rs）——名字含空格，reader
   永远读不出来，不会撞用户符号；同时在 rename 表登记
   `(原符号, 定义处环境)`。同一次展开内同一原名映射到同一 fresh 符号
   （`Expander.renames` 缓存），保证"引入的绑定"与"引入的引用"一致。
2. **自由标识符按定义处解析**：env.rs 的 `lookup_var`/`resolve` 沿环境
   链找不到 fresh 符号时，查 rename 表，回到定义处环境解析原名。
   于是 pitfall 3.1 里宏模板中的 `+` 在使用处 `(let ((+ *)) ...)` 下
   仍然指全局的加法。
3. **`quote` 内部不重命名**：`(quote x)` 里的标识符是数据不是代码，
   必须字面保留（否则 `'ok` 会展开成 `' ok.2`）。`expand_tmpl` 对
   quote 的参数进入 data 模式：替换模式变量，但引入标识符原样保留。
   判定"这是 quote 特殊形式"不只看名字字符串，而是经 def_env 解析
   确认它仍指向内建 quote 关键字——宏定义环境若把 quote 重绑定为
   变量/宏，模板里的 `(quote x)` 按普通组合展开为引用。

### 已知局限

这是"够用"的卫生，不是完整语法对象系统：

- **宏生成宏**时，被生成宏的模板若引用了"生成宏定义处不存在、
  使用处才存在"的绑定，rename 回退可能解析不到（rename 只锚定一层
  def_env）。R5RS 的嵌套示例（pitfall 3.3、be-like-begin）都能过，
  但更刁钻的合成宏没有保证。
- **同一 pattern 中重复出现的模式变量不做查错**（后绑定覆盖先绑定，
  R5RS 说那是 an error）。
- pattern 同一层只支持一个 ellipsis（`x ... y ...` 这样并列两个会
  只处理第一个）。

## 辅助语法的 aux_name 判定

`else`、`=>`、`unquote`、`unquote-splicing`、`quasiquote` 以及 ellipsis
本身都是"辅助语法"：它们不该被当作普通变量，但用户**可以局部重绑**
它们，重绑后特殊含义消失（R5RS 4.3 的精神）。

`aux_name`（src/env.rs）的做法：先查当前环境链——若该符号有变量
或宏绑定，则它已被遮蔽，返回 None；否则沿 rename 链找回原始名字并
返回。链长超过上限（1000，对应宏展开嵌套深度）时显式报错，而不是
静默返回中间名导致 else/=>/unquote 判定悄悄出错。于是：

- `(let ((=> 1)) (cond (#t => 'ok)))`：`=>` 被局部绑定 → 不当 => 子句，
  按普通表达式序列求值 ⇒ 'ok。
- `(let ((unquote 1)) \`(,foo))`：`unquote` 被局部绑定 → quasiquote
  不做解引用，`(,foo)` 作为数据 ⇒ `(,foo)`。
- 宏模板里引入的 `else`/`unquote` 是重命名过的符号，`aux_name` 沿
  rename 链找回原名后仍然被正确识别为辅助语法。

quasiquote 的深度计数（嵌套 `` ` `` 与 `,` 的配对）在
`src/eval.rs` 的 `quasiquote` 函数，对 chibi 套件的
`` `(a `(b ,(+ 1 2) ,(foo ,(+ 1 3) d) e) f) `` 这类双重嵌套用例
逐层正确处理。
