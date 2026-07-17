# WorldQuant 101 Alphas

[English](worldquant_alpha101.en.md)

qweave 将 `alpha1` 到 `alpha101` 构造成内置 alpha 表达式。你可以像使用自定义
表达式一样筛选、组合和字段映射这些 alpha，然后把它们与自己的因子一起批量提交给
Rust evaluator。

实现参考 Kakushadze 的 "101 Formulaic Alphas" 附录 A，并在本文记录项目自己的
默认口径。

本项目与 WorldQuant 没有关联。

## 公开接口

```python
alphas = qweave.worldquant_alpha101(
    {"close": "adj_close"},
    alphas=["alpha13", "alpha101"],
)
qweave.compute_alphas(df, "asset", "time", alphas)
```

`worldquant_alpha101(input_alias, alphas=None)` 返回 `PyExpr` 对象：

- 省略 `alphas` 时返回全部 101 个表达式。
- 传入 `alphas` 时按请求顺序返回指定子集。
- `input_alias` 把标准字段名（如 `close`）映射到实际列名（如 `adj_close`）。
  不需要映射时传入空 dict。
- 每次构造都会打印所选因子实际使用的标准字段到 DataFrame 列名映射；未映射字段
  显式显示为自身，便于确认 `close` 是否已映射到 `close_adj`。

`compute_alphas()` 在完整面板上执行表达式，返回按 `(symbol, time)` 排序的结果；
`with_alphas()` 按原始行序把表达式输出追加回输入 DataFrame。自定义表达式见
[Python 表达式 API](expression_api.md)。

## 默认口径

- `adv{d}` 实现为 `ts_mean(volume, d)`，使用股数成交量而不是成交额。
- 非整数 lookback window 使用 `floor(d)`。
- 论文中的 `min(x, d)` 和 `max(x, d)` 实现为 rolling `ts_min(x, d)` 和
  `ts_max(x, d)`。
- 动态指数公式使用表达式值版本的 `power` 和 `signed_power`。
- `IndClass.sector`、`IndClass.industry`、`IndClass.subindustry` 映射到数值列
  `sector`、`industry`、`subindustry`。

## 覆盖层级

- Tier A：基础 OHLCV 字段 `open`、`high`、`low`、`close`、`volume`；
  `returns` 从 `close` 派生。
- Tier B：Tier A 加上公式引用的 `vwap` 或 `cap`；`adv{d}` 从 `volume` 派生。
- Tier C：需要无空值的 String 或整数分组字段 `sector`、`industry` 或 `subindustry`（整数值必须在 `i32` 范围内）。

如果只使用基础 OHLCV 数据，可以先选择 Tier A alpha；数据中包含 `vwap`、`cap` 或
行业分类后，再逐步打开 Tier B/Tier C。

## 验证

- Rust 测试确认 builder 返回完整的 `alpha1` 到 `alpha101`。
- smoke test 在完整合成面板上计算全部 101 个 alpha。
- golden regression 使用冻结的合成 Parquet fixture 对照全部 alpha 输出。
- 独立 reference 测试覆盖代表性公式。
