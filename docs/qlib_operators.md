# Qlib 算子迁移对照

[English](qlib_operators.en.md)

本表覆盖 [Qlib v0.9.7 的 OpsList](https://github.com/microsoft/qlib/blob/v0.9.7/qlib/data/ops.py)
中注册的全部算子。qweave 提供表达式能力映射，不解析 Qlib 表达式字符串，也不承诺
完整平台或全序列数值兼容。这里的 `x`、`y` 是 `qw.col(...)` 表达式，数值常量用
`qw.lit(...)` 包装。qweave 输入数值列为 Float64，null 按 NaN 处理。

## 逐元素算子

| Qlib | qweave | 迁移注意事项 |
| --- | --- | --- |
| Abs、Log | `x.abs()`、`x.log()` | Log 的非正值处理不同：qweave 输出 NaN，Qlib 的零输入为负无穷 |
| Sign | `x.sign()` | Qlib 先转 float32；极小值的符号可能不同 |
| Power | `qw.power(x, y)` | qweave 对负底数、非整数指数输出 NaN |
| Add、Sub、Mul、Div | `x+y`、`x-y`、`x*y`、`x/y` | 使用浮点算术 |
| Greater、Less | `qw.max(x, y)`、`qw.min(x, y)` | 是逐元素取极值，不是比较 |
| Gt、Ge、Lt、Le、Eq | `x>y`、`x>=y`、`x<y`、`x<=y`、`x==y` | qweave 返回 1/0，任一输入 NaN 则返回 NaN；Qlib 返回布尔值 |
| If | `qw.where_(condition, x, y)` | qweave 的 NaN 条件输出 NaN，不能照搬 NumPy 的真值转换规则 |
| Ne | `x != y` | 返回 Float64 1/0，任一输入 NaN 则返回 NaN；不同于 Qlib 的布尔不等比较 |
| And、Or、Not | `x & y`、`x \| y`、`~x` | Float64 三值逻辑：正数为真，非正数为假，NaN 为未知；Qlib 使用位运算 |

逻辑掩码继续采用 Float64，可参与算术运算。假与未知为 0，真或未知为 1，其余
无法确定的结果为 NaN；非未知仍为 NaN，交换左右操作数不改变结果。这不复刻 Qlib
的整数位运算语义。组合比较时使用括号，例如 `(x > y) & (x != qw.lit(0.0))`。
隐式 Python 真假判断现在抛出 `TypeError`：使用 `&`、`|`、`~`，不要使用
`and`、`or`、`not`。

## 时序算子

qweave 固定窗口要求最近 `days` 个 bar 齐全，不暴露 `min_periods`。缺失值通常使
窗口失效；`ts_count` 是例外，满 `days` 行后统计非缺失样本数。
Qlib 多数窗口使用 `min_periods=1`，部分支持 `N=0` expanding；因此预热期和有缺失值
的结果通常不同。输入交易日、缺行和历史起点也必须由调用方统一，qweave 不自动补齐。

| Qlib | qweave | 迁移注意事项 |
| --- | --- | --- |
| Ref | `x.delay(days)` | 仅正整数滞后同义；Qlib 的 `N=0` 取首值、`N<0` 取未来值；qweave 的 0 为原值 |
| Delta | `x.delta(days)` | 正整数为当前值减滞后值；Qlib 的 0 为减首值，qweave 的 0 为自身相减 |
| Mean、Sum、Std | `x.ts_mean(days)`、`x.ts_sum(days)`、`x.ts_std(days)` | Std 均为样本标准差；窗口及非有限值口径需另行核对 |
| Var | `s * s`，其中 `s=x.ts_std(days)` | 可组合得到样本方差；不新增独立别名 |
| Max、Min | `x.ts_max(days)`、`x.ts_min(days)` | 使用完整窗口规则 |
| IdxMax、IdxMin | `x.ts_argmax(days)`、`x.ts_argmin(days)` | qweave 从 0 计数、平手取最早；Qlib 从 1 计数，迁移时加 `qw.lit(1.0)` |
| Rank | `x.ts_rank(days)` | 是时序百分位排名；不要映射为截面的 `x.rank()` |
| Quantile | `x.quantile(days, q)` | 线性插值；使用完整窗口规则 |
| Med | `x.quantile(days, 0.5)` | 可组合得到中位数 |
| Slope、Rsquare、Resi | `x.slope(days)`、`x.rsquare(days)`、`x.resi(days)` | 对时间索引回归；预热、零方差及近零方差阈值有差异 |
| Corr、Cov | `qw.correlation(x, y, days)`、`qw.covariance(x, y, days)` | Pearson 相关与样本协方差；缺失配对、预热及近零方差处理不同 |
| Skew、Kurt | `x.ts_skew(days)`、`x.ts_kurt(days)` | 相同的偏差修正公式；qweave 要求完整有限窗口；最低样本数分别为 3、4 |
| EMA | `x.ema(days)` | qweave 为均值初始化的递归 EMA；Qlib 为校正权重 EWM，初值和缺失值处理不同 |
| WMA | `x.wma(days)`，同 `x.decay_linear(days)` | qweave 为标准线性加权均值；对齐 Qlib 额外归一化的写法见下文 |
| Mad | `x.ts_mad(days)` | 关于同一个窗口均值的平均绝对偏差；qweave 要求完整有限窗口，Qlib 跳过 NaN 且允许部分窗口 |
| Count | `x.ts_count(days)` | 非 NaN/null 计数，零和无穷均计入；完整全缺失窗口为 0；不同于 Qlib，前 `days-1` 行为 NaN |
| Rolling | 无通用字符串分发接口 | 按具体统计量选择方法；不支持 Qlib 的 expanding／特殊小数窗口分支 |

`Skew`、`Kurt` 的公式分别参考 pandas 的[偏度](https://pandas.pydata.org/pandas-docs/version/2.2/reference/api/pandas.core.window.rolling.Rolling.skew.html)
和[超额峰度](https://pandas.pydata.org/pandas-docs/version/2.2/reference/api/pandas.core.window.rolling.Rolling.kurt.html)。
完整常数窗口输出分别为 0、-3；不满足最低样本数时输出 NaN。

标准 WMA 使用从旧到新的权重 `1..days`，除以权重和，见 [TA-Lib WMA](https://ta-lib.org/functions/wma.html)。
Qlib v0.9.7 对已经归一化的加权值又取 `nanmean`：完整窗口 `[1,2,3]` 的结果为
`7/9`，标准 WMA 为 `7/3`；常数 1 的三期窗口则分别为 `1/3` 和 `1`。
对于正整数窗口长度 `N`，在完整且所有值均有限（无 NaN、null 或无穷值）的窗口上，
Qlib 的 WMA 等于标准 WMA 除以 `N`。例如，对齐 Qlib 的 20 期 WMA 可写为：

```python
qlib_wma20 = x.wma(20) / qw.lit(20.0)
```

该组合仍遵循 qweave 的完整窗口与缺失值规则，不复刻 Qlib 的预热和缺失值处理。

EMA 的均值初始化参考 [TA-Lib EMA](https://ta-lib.org/functions/ema.html)；
qweave 另行规定非有限值后清空状态、重新预热。它不支持 Qlib 的 `N=0` 或小数 `N`。
完整参数与缺失值规则见[表达式 API](expression_api.md)。

## 数据与框架算子

| Qlib | qweave 处理方式 |
| --- | --- |
| Feature | 将已有面板列传入 `qw.col(name)`，无 Qlib provider 加载行为 |
| PFeature | 不支持 PIT 财报数据的 provider 语义；由调用方预先构造可用时点正确的列 |
| ChangeInstrument、Mask | 不提供表达式内切换标的；由调用方先把参考标的序列按时间连接到面板 |
| TResample | 不在表达式引擎中改变时间轴；由调用方先用 Polars 重采样 |
