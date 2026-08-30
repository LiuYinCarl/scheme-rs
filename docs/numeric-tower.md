# 数字塔：精确数与 inexact 实数

实现位于 `src/number.rs`。scheme-rs 支持 R5RS 数字塔的三种表示，
**有意省略复数**（见文末）。

## 表示

```rust
Value::Int(BigInt)              // 精确整数，无界
Value::Rational(Rc<BigRational>)// 精确有理数，始终约分
Value::Real(f64)                // inexact 实数
```

**始终约分**（`norm`，src/number.rs:31）：每次精确运算后把结果规范化
——BigRational 构造时自动约分，分母为 1 时退化为 `Int`。好处是
`eqv?`/`equal?` 对精确数直接比较分子分母即可，不需要交叉相乘，
也保证 `(denominator (/ 6 4))` 得 2 而不是 4。

## 精确/inexact 传染（R5RS 6.2.2）

一次运算中只要有一个操作数是 `Real`，整组运算改走 f64；否则全程用
BigRational 精确计算。比较运算（`= < > <= >=`）同样按此规则选择
比较路径。`exact->inexact` 是精确值转 f64；`inexact->exact` 用
`BigRational::from_float` 得到 f64 的精确二进制值
（`(inexact->exact 0.5)` ⇒ 1/2）。

整数类运算（`quotient remainder modulo gcd lcm`）接受**整数值的**
inexact 参数，结果保持 inexact（R5RS 6.2.5：
`(remainder -13 -4.0)` ⇒ -1.0）；非整数 inexact 则报错。

## round：逢半取偶（6.2.5）

`round` 必须 round half to even：`(round 7/2)` ⇒ 4 而
`(round 5/2)` ⇒ 2（`round_op`，src/number.rs:338——精确路径手动
比较分数部分与 1/2，平局看奇偶；f64 路径用 `round_ties_even`）。
相应的 `floor/ceiling/truncate` 对精确有理数直接取 BigRational 的
对应运算，inexact 输入给 inexact 结果。

`sqrt` 对完全平方（含分数，如 4/9）给精确结果（牛顿法求整数平方根
后验证），否则退到 f64；负数报错（复数省略的缘故）。

## rationalize：Stern-Brocot 找"最简"有理数

`(rationalize x e)` 要返回区间内**最简**的有理数（6.2.5：
`(rationalize 3/10 1/10)` ⇒ 1/3）。`simplest_between`
（src/number.rs:383）是标准的连分数/Stern-Brocot 递归：

1. 若 lo 本身是整数，答案就是 lo；
2. 若 ceil(lo) ≤ hi，区间里有整数，取最小那个（分母为 1，最简）；
3. 否则两端同层（floor 相同），对两边的小数部分取倒数后递归，
   再把结果倒回来。

## 词法：进制与精确性前缀（6.2.4）

`parse_number_radix`（src/number.rs:591）支持至多两个可组合前缀
（`#b #o #d #x` 进制、`#e #i` 精确性），之后是主体：

- 整数（任意进制）、分数 `n/d`；
- 十进制小数与指数（`1.5e3`；指数标记只支持 `e/E`，R5RS 的
  `s f d l` 单/双精度标记未支持——本就极少实现支持）；
- **`#` 占位数字**：`15##` ⇒ 1500.0——`#` 是"未指定的数字"，出现即
  inexact（把 `#` 换成 0 解析后转 inexact）；
- `#e`/`#i` 在解析后强制转换精确性（`#e1.5` ⇒ 3/2）。

reader 与 `string->number` 共用这条路径，所以源码字面量与
`(string->number "15##")` 行为一致。`number->string` 用 BigInt 的
任意进制输出（`(number->string 255 16)` ⇒ "ff"）。

## 有意省略：复数

R5RS 数字塔的顶层是复数，但实现代价（直角/极坐标两种构造、整套
运算与谓词、词法 `a+bi`/`a@b`）远超收益，故有意省略：

- `complex?` 对现有三种数都答 `#t`（R5RS 里 real 也是 complex）；
- `sqrt` 负数参数报错而不是返回虚数；
- 相关的 `make-rectangular` 等构造器不存在。
README 与 r5rs-compliance.md 都记录了这条偏差。
