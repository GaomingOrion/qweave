# 国泰君安 Alpha191

[English](gtja_alpha191.en.md)

qweave 将国泰君安 191 个短周期价量因子构造成内置表达式，公开 builder 名称为
`gtja_alpha191`，输出名称固定为 `gtja_alpha001` 至 `gtja_alpha191`。

```python
alphas = qweave.gtja_alpha191(
    {"close": "adj_close"},
    alphas=["gtja_alpha001", "gtja_alpha191"],
)
out = qweave.compute_alphas(df, "asset", "time", alphas)
```

省略 `alphas` 时返回全部 191 个表达式；指定名称时保持请求顺序。`input_alias`
将标准输入名映射到 DataFrame 实际列名。

## 输入与口径

基础输入为 `open`、`close`、`high`、`low`、`volume` 和 `vwap`。部分因子还需要：

- `index_open`、`index_close`：市场基准日线；
- `mkt`、`smb`、`hml`：Alpha30 的三因子回归输入。

额外时间序列应作为面板列提供，并在同一交易日对所有资产重复。`amount` 在公式内部
按 `volume * vwap` 派生。qweave 不自动决定复权、停牌填充或指数补日，调用方必须在
输入阶段统一这些规则。

`SMA(A,n,m)` 使用递归平滑系数 `m/n`；`WMA(A,n)` 使用原研报定义的 `0.9^i`
权重。排名、满窗口和缺失值沿用 qweave 表达式引擎口径。详细来源、原始公式页码和
已知歧义见[公式来源说明](gtja_alpha191_sources.md)。

## 验证

- 名称集合测试精确覆盖 `gtja_alpha001`–`gtja_alpha191`；
- 全量 smoke test 在 320 日合成面板上运行全部 191 个因子；
- 新增递归平滑、加权移动平均和回归内核有独立手算测试；
- 测试数据全部为合成数据，不包含真实市场数据。

本项目与国泰君安证券没有关联，内置公式不构成投资建议。
