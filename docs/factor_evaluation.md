# 因子评估

[English](factor_evaluation.en.md)

评估器回答的问题是：**这个因子是否携带 forward return 信息？** 它关注预测力
（IC / RankIC）、单调性（分位收益）和交易可用性（换手、long-short 组合）。
它不是回测器：不模拟撮合、滑点、成交约束或退出侧流动性。

## Pipeline

评估流程是单 DataFrame pipeline。每一步都按原始行序追加列，不做额外 join：

```python
import qweave as qf

# 1) 计算或准备因子列
df = qf.with_alphas(
    df,
    symbol_col="symbol",
    time_col="date",
    alphas=qf.worldquant_alpha101({}),
)

# 2) 追加 forward-return 标签
df = qf.with_labels(
    df,
    symbol_col="symbol",
    time_col="date",
    horizons=[1, 5, 10, 20],
    entry_lag=1,
    entry_col="close",
    exit_col="close",
    tradable_col="tradable",
)

# 3) 评估因子列
result = qf.evaluate(
    df,
    symbol_col="symbol",
    time_col="date",
    factor_cols=[f"alpha{i}" for i in range(1, 102)],
    quantiles=10,
    binning="daily",
    tradable_col="tradable_entry",
    demean="none",
)

result.summary.sort("rank_ic_ir", descending=True).head(20)
result.save("runs/2026-07-04/")
```

## `with_labels`

标签定义：

```text
ret_h(t) = exit(t + entry_lag + h) / entry(t + entry_lag) - 1
```

bar offset 基于**全面板日期网格**，即所有 symbol 日期的 union，或调用方显式传入
的 `calendar`。如果某个 symbol 在某天缺行，该位置输出 NaN，而不是悄悄压缩持有
期。

| 参数 | 默认值 | 含义 |
| --- | --- | --- |
| `horizons` | 必填 | panel bar 维度的持有期，必须为正整数且唯一 |
| `entry_lag` | `1` | 信号日 T 到入场日之间的 bar 数 |
| `entry_col` / `exit_col` | `"close"` | 入场/出场价格列 |
| `tradable_col` | `None` | 入场日是否可交易的 boolean 列 |
| `calendar` | `None` | 显式交易日序列，用于严格校验 |

默认口径是较保守的 T+1 close-to-close：在 T 日收盘计算信号，T+1 收盘入场，持有
`h` 个 bar，不引入 look-ahead。

如果提供 `tradable_col`，`with_labels` 会把入场日的可交易标记 shift 回信号日，
追加为 `tradable_entry`，供 `evaluate` 使用。入场日缺行或标记为 null 都视为
不可交易。

## `evaluate`

```python
result = qf.evaluate(
    df,
    symbol_col,
    time_col,
    factor_cols,
    label_cols=None,        # None => 自动识别 ret_{h}
    quantiles=10,
    binning="daily",        # "daily" | "global"
    group_col=None,
    tradable_col=None,
    demean="none",          # "none" | "universe" | "group"
    min_cs_count=30,
    cost_bps=0.0,
    weighting="quantile",   # "quantile" | "factor"
    output_dir=None,
)
```

`factor_cols` 必填，因为输入 frame 会同时包含价格列、因子列和标签列。
`label_cols=None` 时自动识别所有 `ret_{h}` 命名列，并从列名中解析 horizon。

### 有效样本层

逐日、逐因子：

- **factor-valid** = 可交易且因子非 NaN。
- **pair-valid(h)** = factor-valid 且 `ret_h` 非 NaN。
- factor-valid 少于 `min_cs_count` 的日期会被整日跳过。
- pair-valid 少于 `min_cs_count` 的 horizon 会得到 NaN 的 IC/RankIC/spread，
  但分位桶均值仍保留，`count` 会告诉你样本有多薄。

不可交易样本会从整个截面中剔除，并计入 coverage。退出侧流动性、买卖方向和成交
模拟不在这里处理，那属于回测。

### IC 与 RankIC

- **IC** = Pearson(factor value, ret_h)。
- **RankIC** = Pearson(factor rank, label rank)，即 Spearman，ties 取平均 rank。

summary 按 `(factor, horizon)` 输出 mean、std、IR、`t_nw`、`win_rate` 等统计。
`t_nw` 是 Newey-West t-stat，lag 使用 `h - 1`，用于处理 h>1 forward return
导致的 daily IC 序列重叠自相关。

### 分箱与分位收益

`binning="daily"`：每日把 factor-valid 样本稳定排序并切成近似等样本分位桶。
这是组合解释最直接的口径：例如 Q10 表示每天做多因子值最高的 10%。

`binning="global"`：用全样本 pooled distribution 的 type-7 quantile 做固定边界。
它回答的是“因子值本身是否与未来收益单调相关”这个分布问题。注意，global 边界
包含全样本信息，不应直接当作可交易组合边界。

`quantile_returns` 表是一行一个 `(date, factor, bin)`：

```text
date | factor | bin | bin_lo | bin_hi | count | mean_ret_1 | mean_ret_5 | ...
```

summary 中的 `spread` 是 top bucket 均值减 bottom bucket 均值；
`monotonicity` 是 bucket index 和全期 bucket 平均收益之间的 Kendall tau。

### Demean

- `none`：使用原始收益。
- `universe`：减去当日可交易 universe 等权平均收益。
- `group`：减去当日 group 均值，用于行业内 IC；需要 `group_col`，且拒绝 null group。

### 换手、组合与自相关

跨日指标逐因子顺序计算，并在因子之间并行：

- `turnover`：top/bottom bucket 的成员换手。
- `portfolio`：需要 `ret_1`。`weighting="quantile"` 使用 top/bottom 等权
  long-short；`weighting="factor"` 使用因子去均值后的幅度权重。h>1 时使用最近 h
  个信号日权重的 staggered 平均。
- `rank_autocorr`：day t 与 day t-lag 的因子 rank Pearson correlation，
  默认统计 lag `{1, 5, 10, 20}`。

summary 还会包含 annualized long-short gross/net return、IR、换手等字段。

## 结果对象与 streaming

常用属性：

- `result.summary`
- `result.ic`
- `result.quantile_returns`
- `result.coverage`
- `result.turnover`
- `result.portfolio`
- `result.rank_autocorr`
- `result.ic_monthly`（Date/Datetime time 列时存在）
- `result.meta`
- `result.save(dir)`

设置 `output_dir` 时，大表会按 factor batch 流式写入 parquet，并以
`polars.LazyFrame` scan 的形式返回；小表保留在内存中。对于千级因子 run，这能
限制峰值内存。若输入宽表本身就是内存瓶颈，可用 `factor_source=<parquet>` 从磁盘
批量读取因子列。

## 交互式报告

`result.view()` 会启动嵌入式 `qweave-server`，在浏览器打开 Vue + ECharts 的
交互式报告（summary 表 + 单因子 Returns/IC tearsheet），无需任何外部文件。
该视图适合筛选后的 shortlist，不建议直接用于千级因子全量 run。

`result.to_html(path, max_detail_factors=200)` 可写出单文件 HTML 报告。

streaming `output_dir` 可通过 CLI 打开：

```powershell
cargo run -p qweave-server -- --dir <output_dir> --open
```

## `factor_correlation`

```python
corr = qf.factor_correlation(
    df,
    symbol_col,
    time_col,
    factor_cols,
    tradable_col=None,
    min_cs_count=30,
)
```

返回时间平均的每日截面 rank correlation，是一个带 `factor` 首列的对称宽表。
它适合 `evaluate` 后筛出的 shortlist，不适合直接对几千个原始因子使用，因为它会
把所有因子列密集加载进内存。

## 验证

1. Rust 小面板手算单测锁定精确值。
2. `tests/test_evaluate.py` 和 `tests/test_flows.py` 用独立 NumPy reference 重新
   推导各项指标，并覆盖 NaN、ties、tradable mask、streaming vs memory 等场景。

与 alphalens 的差异是有意设计：Newey-West t-stat、确定性 rank bucketing、
显式 `entry_lag`、tradable mask、同时报告 Pearson IC 和 Spearman RankIC，以及不
提供带 look-ahead 风险的 future-return z-score clipping。

## 非目标

当前评估器不做组合优化、Barra 风格风险模型中性化、event study、pyfolio 集成、
精细回测、撮合、滑点、退出侧流动性或资金曲线模拟。它衡量因子是否有信息，不回答
策略最终能赚多少钱。
