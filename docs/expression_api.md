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

- **时序算子**按 symbol 独立计算，窗口为最近 `d` 个 bar。窗口未满或窗口内含
  NaN 时输出 NaN，因此每个 symbol 的前 `d - 1` 行为 NaN。
- **截面算子**在每个时间点的全截面上计算，NaN 样本不参与且保持 NaN。
- **比较算子**输出 1.0 / 0.0；任一操作数为 NaN 时输出 NaN。

### 逐元素

| 算子 | 含义 |
| --- | --- |
| `+` `-` `*` `/`、一元 `-` | 算术运算 |
| `<` `>` `<=` `>=` `==` | 比较，成立为 1.0，否则 0.0 |
| `abs()` | 绝对值 |
| `log()` | 自然对数 |
| `sign()` | 符号函数（-1 / 0 / 1） |
| `min(x, y)` / `max(x, y)` | 逐元素最小 / 最大 |
| `power(x, y)` | `x^y` |
| `signed_power(x, y)` | `sign(x) * abs(x)^y` |
| `where_(cond, a, b)` | `cond` 成立取 `a`，否则取 `b` |

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
| `ts_std(d)` | 样本标准差（`ddof = 1`） |
| `slope(d)` / `rsquare(d)` / `resi(d)` | 窗口值对时间索引的 OLS 斜率 / R² / 最后一点残差 |
| `quantile(d, q)` | 窗口分位数，`q ∈ [0, 1]`，线性插值 |
| `decay_linear(d)` | 线性加权平均，权重 `1..d`，越新的 bar 权重越大 |
| `correlation(x, y, d)` | 窗口 Pearson 相关；任一侧零方差时为 NaN |
| `covariance(x, y, d)` | 窗口样本协方差（`ddof = 1`） |

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
