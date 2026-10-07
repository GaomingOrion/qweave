# Python 表达式 API

[English](expression_api.en.md)

qweave 提供 eager expression API，用于快速构造和执行 alpha 表达式。你写的是
普通 Python 表达式，底层是 Rust `Expr` 树；执行时可以把一批表达式一起交给 DAG
evaluator，而不是在 Python 里一列一列循环。

## 构造表达式

```python
import qweave as qw

intraday_return = (
    (qw.col("close") - qw.col("open"))
    / (qw.col("high") - qw.col("low") + qw.lit(0.001))
).alias("intraday_return")
```

表达式传给 `compute_alphas` 或 `with_alphas` 前必须设置 alias；alias 会成为输出
列名。

## 算子速查

通用口径：

- **固定窗口聚合算子**按 symbol 独立计算，窗口为最近 `d` 个 bar。窗口未满或窗口内含
  NaN 时输出 NaN，因此每个 symbol 的前 `d - 1` 行为 NaN。`ts_count` 是缺失值规则的
  例外：满 `d` 行后统计非缺失样本数。
- **截面算子**在每个时间点的全截面上计算，NaN 样本不参与且保持 NaN。
- **比较算子**输出 1.0 / 0.0；任一操作数为 NaN 时输出 NaN。

### 逐元素

| 算子 | 含义 |
| --- | --- |
| `+` `-` `*` `/`、一元 `-` | 算术运算 |
| `<` `>` `<=` `>=` `==` `!=` | 比较，成立为 1.0，否则 0.0 |
| `&` / `\|` / `~` | Float64 掩码上的三值逻辑与 / 或 / 非 |
| `abs()` | 绝对值 |
| `log()` | 自然对数 |
| `sign()` | 符号函数（-1 / 0 / 1） |
| `min(x, y)` / `max(x, y)` | 逐元素最小 / 最大 |
| `power(x, y)` | `x^y` |
| `signed_power(x, y)` | `sign(x) * abs(x)^y` |
| `where_(cond, a, b)` | `cond` 成立取 `a`，否则取 `b` |

比较和逻辑运算均返回 Float64 掩码（`1.0`、`0.0` 或 NaN），仍可参与乘法和
`ts_sum`。逻辑输入与 `where_` 一样：正数（含正无穷）为真，零、负数和负无穷
为假，NaN/null 为未知。这些是逻辑运算，不执行整数位运算；两侧表达式均会计算，
没有短路求值。

| `a` | `b` | `a & b` | `a \| b` |
| --- | --- | --- | --- |
| 假 | 假 | 0 | 0 |
| 假 | 真 | 0 | 1 |
| 假 | 未知 | 0 | NaN |
| 真 | 真 | 1 | 1 |
| 真 | 未知 | NaN | 1 |
| 未知 | 未知 | NaN | NaN |

与、或交换左右输入不改变结果。非运算将真变为 0、假变为 1，未知仍为 NaN。
常量继续使用 `qw.lit(...)`；比较表达式需要括号：

```python
mask = (qw.col("close") > qw.col("open")) & (qw.col("volume") != qw.lit(0.0))
up_days = mask.ts_sum(20).alias("up_days")
```

**行为收紧：**表达式的隐式 Python 真假判断现在抛出 `TypeError`。请使用 `&`、`|`、
`~`，不要使用 Python 的 `and`、`or`、`not`。`bool(expr)`、`if expr` 和
`x < y < z` 这样的链式比较也会报错；链式比较应写成 `(x < y) & (y < z)`。

### 时序窗口（逐 symbol，窗口 `d`）

| 算子 | 含义 |
| --- | --- |
| `delay(d)` | `d` 个 bar 前的值 |
| `delta(d)` | `x - delay(x, d)` |
| `ts_sum(d)` / `ts_mean(d)` / `product(d)` | 窗口和 / 均值 / 乘积 |
| `ts_min(d)` / `ts_max(d)` | 窗口最小 / 最大 |
| `ts_argmin(d)` / `ts_argmax(d)` | 极值在窗口内的 0-based 位置（0 = 最旧，`d-1` = 当前；平手取最早） |
| `ts_rank(d)` | 当前值在窗口内的百分位 rank，取值 `(0, 1]`，ties 取平均（pandas `rank(pct=True)` 口径） |
| `ts_rank_raw(d)` | 当前值的 0-based 升序位置，ties 取最小（DolphinDB `mrank` 口径） |
| `ts_mad(d)` | 关于同一个窗口均值的平均绝对偏差；要求完整有限窗口 |
| `ts_count(d)` | 满 `d` 行后统计非 NaN/null 样本数，零及无穷均计入 |
| `ts_std(d)` | 样本标准差（`ddof = 1`） |
| `ts_skew(d)` | 偏差修正的样本偏度，至少 3 个样本；完整常数窗口为 0 |
| `ts_kurt(d)` | 偏差修正的 Fisher 超额峰度，至少 4 个样本；完整常数窗口为 -3 |
| `slope(d)` / `rsquare(d)` / `resi(d)` | 窗口值对时间索引的 OLS 斜率 / R² / 最后一点残差 |
| `quantile(d, q)` | 窗口分位数，`q ∈ [0, 1]`，线性插值 |
| `wma(d)` / `decay_linear(d)` | 相同的线性加权平均，权重 `1..d`，越新的 bar 权重越大 |
| `correlation(x, y, d)` | 窗口 Pearson 相关；任一侧零方差时为 NaN |
| `covariance(x, y, d)` | 窗口样本协方差（`ddof = 1`） |

固定窗口接口统一不提供 `min_periods`。`ts_skew`、`ts_kurt` 需要完整有限窗口，
NaN、null、无穷值都会使该窗口输出 NaN；不满足最低样本数时也输出 NaN。
`wma` 直接复用 `decay_linear` 的完整窗口与缺失值规则，二者在 DAG 中共用节点。
零窗口输出 NaN；负数或非整数窗口由 Python 绑定拒绝。

`ts_mad(d)` 在同一窗口内计算 `mean(abs(x - mean(x)))`，不是中位数绝对偏差，
也不能用两个 rolling mean 嵌套替代。窗口内有 NaN、null 或无穷值时输出 NaN；
常数窗口及 `d=1` 的有限窗口输出 0。通过平移、缩放限制中间值范围，总耗时为
`O(T*d)`，不逐窗口分配临时数组。

`ts_count(d)` 要求窗口满 `d` 行，但允许缺失值：完整窗口全缺失时输出 0。
`d=1` 时缺失值输出 0，其余输出 1；前 `d-1` 行仍为 NaN。通过移入、移出更新计数，
总耗时为 `O(T)`，每个 symbol 仅需 `O(1)` 滚动状态。两者均支持树执行器和 DAG；
`d=0` 输出 NaN，负数和非整数窗口被拒绝。

### 指数平滑

`ema(days)` 按 symbol 计算技术指标 EMA：前 `days` 个连续有限样本的均值作为
初值，此后按 `y = (1 - alpha) * y_prev + alpha * x` 递推，
`alpha = 2 / (days + 1)`。初始化前输出 NaN；NaN、null 或无穷值使状态清空，
需要重新收集 `days` 个连续有限样本。`days=1` 保留有限输入，`days=0` 全为 NaN。
它使用已有全部历史状态，而不是每行重算最近 `days` 个 bar 的指数加权平均。

```python
alphas = [
    qw.col("close").ts_skew(20).alias("skew20"),
    qw.col("close").ts_kurt(20).alias("kurt20"),
    qw.col("close").ema(20).alias("ema20"),
    qw.col("close").wma(20).alias("wma20"),
]
```

四项均支持树执行器和 DAG。Skew/Kurt 增量更新滑窗中心矩，每经过 `d` 行重新计算
一次以控制累计误差，常规情况下总耗时为 `O(T)`。数值尺度剧变或波动大幅缩小时
会额外重新计算窗口，因此极端输入下最坏为 `O(T*d)`。EMA 和 WMA 为 `O(T)`，
其中 `T` 为每个 symbol 的行数。

**Rust WMA 迁移：** 原 `alpha::wma` 的国泰君安 `0.9^age` 权重现名为
`alpha::gtja_wma`，对应节点为 `Expr::GtjaWma`；`alpha::wma` 现在表示标准线性
WMA。Alpha191 内部已迁移，因子数值保持不变。Python 不暴露国泰君安专用 WMA。
Qlib 的同名算子不能一概直接替换，见[算子迁移对照](qlib_operators.md)。

### 截面（逐时间点）

| 算子 | 含义 |
| --- | --- |
| `rank()` | 当日截面百分位 rank，取值 `(0, 1]`，ties 取平均 |
| `scale(scale_to=1.0)` | 缩放使当日截面 `sum(abs(x)) = scale_to`；全零截面输出 NaN |
| `group_rank(x, g)` | 在（日期, 分组）内的百分位 rank；`g` 必须是无空值的 String/整数列，整数须在 `i32` 范围内 |
| `group_neutralize(x, g)` | 减去（日期, 分组）内的均值；`g` 的类型约束同上 |

## 执行表达式

保留输入 DataFrame 并按原始行序追加因子列时，使用 `with_alphas`：

```python
out = qw.with_alphas(df, "asset", "time", [intraday_return])
```

需要完整历史、按 `(symbol, time)` 排序的 tidy panel 时，使用 `compute_alphas`：

```python
out = qw.compute_alphas(df, "asset", "time", [intraday_return])
```

`compute_alphas(..., output_path="alphas.parquet")` 会写出完整结果并返回摘要。
`with_alphas` 每个表达式会分配一个完整输出 buffer，再 scatter 回输入行序；大批量
因子且不需要保留原始 shape 时，优先使用 `compute_alphas`。

经验上可以这样选：

- notebook 探索、希望保留原始列：用 `with_alphas`。
- 批量因子产出、准备落盘或后续评估：用 `compute_alphas`。

## 复用模板

`collect_inputs()` 返回表达式引用的标准输入字段，`replace_inputs()` 把这些字段
映射到实际 DataFrame 列，同时保留表达式 alias：

```python
expr = ((qw.col("close") + qw.col("open")) / qw.lit(2.0)).alias("mid")
assert expr.collect_inputs() == {"close", "open"}

adjusted = expr.replace_inputs({"close": "adj_close", "open": "adj_open"})
```

字段映射是表达式树的一部分。可见的 alias 路径只有 `replace_inputs()`，或内置
因子库的 `input_alias` 参数。

## 内置因子库

```python
alphas = qw.worldquant_alpha101(
    {"close": "adj_close", "open": "adj_open"},
    alphas=["alpha13", "alpha101"],
)
out = qw.compute_alphas(df, "asset", "time", alphas)
```

`qw.qlib_alpha158(input_alias, alphas=None)` 以同样签名暴露 Qlib Alpha158。
`qw.gtja_alpha191(input_alias, alphas=None)` 暴露国泰君安 Alpha191，输出名为
`gtja_alpha001`–`gtja_alpha191`。
如果不需要字段映射，传入空 dict。实现口径和输入字段见
[WorldQuant 101](worldquant_alpha101.md)、[Qlib Alpha158](qlib_alpha158.md) 与
[国泰君安 Alpha191](gtja_alpha191.md)。
三个内置 builder 会打印本次所选因子实际使用的标准字段到 DataFrame 列名映射；未映射
字段显示为自身。因此同时拥有 `close` 和 `close_adj` 时，可以在计算前确认是否显式
传入了 `{"close": "close_adj"}`。

## 下一步

因子算完后，用[因子评估](factor_evaluation.md)的 `with_labels` 构造无前视
forward-return 标签，再用 `evaluate` 产出 IC/分位/换手诊断，并通过
`result.view()` 打开交互式报告。
