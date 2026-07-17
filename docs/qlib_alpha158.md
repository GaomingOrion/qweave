# Qlib Alpha158

[English](qlib_alpha158.en.md)

qweave 将 Microsoft Qlib 的 `Alpha158` 特征集构造成 158 个内置 alpha 表达式。
它适合用作研究 pipeline 的基准特征集：输入字段少、结构清楚、计算路径覆盖大量
rolling kernel，也便于和 Qlib handler 做口径对照。

公式参考 Qlib `Alpha158` handler；项目差异口径记录在本文。

本项目与 Microsoft 或 Qlib 没有关联。

## 结构

158 个因子由以下部分组成：

- **9 个 kbar 蜡烛形态因子：** `KMID`、`KLEN`、`KMID2`、`KUP`、`KUP2`、
  `KLOW`、`KLOW2`、`KSFT`、`KSFT2`。
- **4 个价格因子：** `OPEN0`、`HIGH0`、`LOW0`、`VWAP0`，均按 close 归一化。
- **29 组 rolling 因子 x 5 个窗口：** 窗口为 `{5, 10, 20, 30, 60}`，
  命名为 `<GROUP><d>`，例如 `MA5`、`CORR60`。

常见 rolling group：

| Group | 含义 |
| --- | --- |
| `ROC` | `delay(close, d) / close` |
| `MA` | `ts_mean(close, d) / close` |
| `STD` | `ts_std(close, d) / close` |
| `BETA` | 窗口线性趋势斜率，按 close 归一化 |
| `RSQR` | 同一线性拟合的 R2 |
| `RESI` | 窗口最后一点相对拟合线的残差，按 close 归一化 |
| `MAX` / `MIN` | high/low 的 rolling max/min，按 close 归一化 |
| `QTLU` / `QTLD` | close 的 0.8 / 0.2 rolling quantile，按 close 归一化 |
| `RANK` | close 的时序百分位 rank |
| `RSV` | `(close - ts_min(low, d)) / (ts_max(high, d) - ts_min(low, d))` |
| `IMAX` / `IMIN` / `IMXD` | high/low 极值在窗口内的位置及差值 |
| `CORR` / `CORD` | close 与成交量相关性相关特征 |
| `CNTP` / `CNTN` / `CNTD` | 上涨/下跌天数比例及差值 |
| `SUMP` / `SUMN` / `SUMD` | 上涨/下跌幅度和及差值 |
| `VMA` / `VSTD` / `WVMA` | 成交量相关 rolling 特征 |
| `VSUMP` / `VSUMN` / `VSUMD` | 成交量版本的 `SUMP` / `SUMN` / `SUMD` |

所有因子都是逐 symbol 的时序或逐元素表达式，不使用截面信息。

## 公开接口

```python
alphas = qweave.qlib_alpha158(
    {"close": "adj_close"},
    alphas=["KMID", "MA5", "CORR20"],
)
qweave.compute_alphas(df, "asset", "time", alphas)
```

`qlib_alpha158(input_alias, alphas=None)` 返回 `PyExpr` 对象。省略 `alphas`
时返回全部 158 个因子；传入 `alphas` 时按请求顺序返回指定子集。
`input_alias` 将标准输入字段映射到实际 DataFrame 列；不需要映射时传空 dict。
每次构造都会打印所选因子实际使用的标准字段到 DataFrame 列名映射；未映射字段显式
显示为自身，便于确认 `close` 是否已映射到 `close_adj`。

## 输入字段

所有因子只引用 `open`、`high`、`low`、`close`、`volume`、`vwap`。
不需要行业分类或基本面字段。

## 与 Qlib 的差异口径

- **Warmup：** Qlib 使用 `min_periods=1`，会在窗口未满时输出部分窗口值。
  qweave 要求完整且非 NaN 的窗口，因此每个 symbol 的前 `d - 1` 行为 NaN。
- **`IMAX` / `IMIN` offset：** qweave 的 `ts_argmax` / `ts_argmin` 是 0-based；
  Qlib 的 `IdxMax` / `IdxMin` 是 1-based。builder 会加 `+1` 对齐 Qlib。
  `IMXD` 中两个 offset 会抵消，因此不加 `+1`。
- **比较和 boolean NaN：** `close > delay(close, 1)` 在前值缺失时为 NaN，
  不会被强制转成 0。
- **Quantile：** `QTLU` / `QTLD` 使用 pandas `linear` 插值口径。
- **标准差：** `STD`、`VSTD`、`WVMA` 使用样本标准差（`ddof = 1`）。
- **相关性：** `CORR` / `CORD` 是 Pearson correlation；任一窗口零方差时返回 NaN。

`BETA`、`RSQR`、`RESI`、`QTLU`、`QTLD` 使用专门的 Rust kernel，而不是由其他
表达式组合出来。

## 验证

- Rust 单测确认 builder 返回 158 个唯一命名因子，并且只引用 OHLCV 和 vwap。
- smoke test 在完整合成面板上计算全部 158 个因子。
- Python 测试用独立 NumPy reference 覆盖代表性因子和边界口径。
- 冻结的合成 Parquet golden fixture 防止非预期数值漂移。
