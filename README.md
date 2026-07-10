# qweave

[English](README.en.md)

qweave 是一个面向量化研究的 **Rust + Polars 因子工作流工具包**。它把常见
alpha 表达式、因子批量计算、forward-return 标签、IC/RankIC 评估、分位收益和
交互式报告放进同一条 Python DataFrame pipeline，目标是让研究员少写胶水代码，
更快完成从“想法”到“可比较结果”的闭环。

项目当前聚焦因子加工与因子评估，正在向量化建模、策略构建和回测扩展。API 仍处
于 pre-1.0 阶段，但已经适合本地研究实验和内部工作流试用。

## 为什么值得看

- **一条 DataFrame pipeline：** 传入 Polars DataFrame，追加 alpha、标签和评估
  结果，不需要在因子矩阵、标签矩阵和分析表之间来回 join。
- **Rust 热路径：** 面板排序、结构校验、滚动窗口、截面算子、表达式 DAG 求值和
  评估统计都放在 Rust 侧执行，Python 主要负责编排。
- **批量表达式执行：** 默认 DAG evaluator 会复用公共子表达式、复用中间 slot，
  并融合 elementwise chain，适合一次性计算上百个相互重叠的 alpha。
- **内置可复用因子库：** `worldquant_alpha101()` 和 `qlib_alpha158()` 返回普通
  表达式对象，可以筛选子集、替换输入字段，也可以和自定义表达式混合执行。
- **研究口径清楚：** forward return、可交易样本、分位分箱、去均值、换手和
  long-short 诊断都有明确默认口径，避免“结果看起来不错但定义不清”的问题。
- **性能可复现：** 仓库提供合成面板 benchmark，可直接比较 qweave、Qlib
  Alpha158 和 KunQuant Alpha101 路径。历史 macOS 数字已移除，当前性能结论应在
  Windows/PowerShell 环境重新测量后发布。

## 和 Qlib、KunQuant 的关系

qweave 不是要复制整个 Qlib，也不是 KunQuant 的 JIT 编译器替代品。它更像一个
轻量、高速、Polars 原生的因子研究内核，可以独立使用，也可以嵌入更大的研究平台。

| 项目 | 更擅长 | qweave 的侧重点 |
| --- | --- | --- |
| Qlib | 完整 AI 量化平台，覆盖数据、模型、组合、回测和执行链路 | 更轻量的因子计算与评估内核，直接接 Polars DataFrame，适合已有数据管线 |
| KunQuant | 将表达式 batch 编译成优化后的 C++/JIT 执行路径 | 不要求用户管理 C++ 编译/JIT 生命周期，强调 Python 易用性、Rust kernel 和评估闭环 |
| pandas/Alphalens 类工具 | 交互式分析和传统 DataFrame 工作流 | 将因子加工、标签和评估放入同一个 Rust/Polars pipeline，减少大面板上的 Python 循环 |

更详细的定位见 [项目对比](docs/comparison.md)，可复现实验见
[基准测试](docs/benchmark.md)。

## 快速开始

```python
import polars as pl
import qweave as qf

df = pl.DataFrame(
    {
        "asset": ["A", "A", "B", "B"],
        "time": [1, 2, 1, 2],
        "open": [10.0, 11.0, 20.0, 19.0],
        "close": [11.0, 12.0, 19.0, 21.0],
        "high": [12.0, 13.0, 21.0, 22.0],
        "low": [9.0, 10.0, 18.0, 18.5],
        "volume": [100.0, 120.0, 80.0, 90.0],
        "tradable": [True, True, True, True],
    }
)

alphas = [
    (
        (qf.col("close") - qf.col("open"))
        / (qf.col("high") - qf.col("low") + qf.lit(0.001))
    ).alias("intraday_return")
]

df = qf.with_alphas(df, "asset", "time", alphas)
df = qf.with_labels(
    df,
    symbol_col="asset",
    time_col="time",
    horizons=[1],
    entry_lag=0,
    entry_col="close",
    exit_col="close",
    tradable_col="tradable",
)

result = qf.evaluate(
    df,
    symbol_col="asset",
    time_col="time",
    factor_cols=["intraday_return"],
    quantiles=2,
    min_cs_count=2,
    tradable_col="tradable_entry",
)

print(result.summary)
```

批量计算内置因子：

```python
alphas = qf.worldquant_alpha101({}, alphas=["alpha13", "alpha101"])
out = qf.compute_alphas(df, "asset", "time", alphas)
```

`with_alphas` 会按输入 DataFrame 原始行序追加因子列；`compute_alphas` 会输出完整
的 `(time, symbol)` 面板，也可以写出 Parquet。

## 安装

当前仓库面向源码构建，尚未发布到 PyPI 或 crates.io。

前置要求：

- Python 3.10 或更新版本
- `uv`
- Rust nightly，包含 `rustfmt` 和 `clippy`

```powershell
uv sync --dev
uv run maturin develop
```

仓库包含 `rust-toolchain.toml`，Cargo 会自动使用固定的 nightly toolchain。

## 能力地图

**已经可用**

- WorldQuant 101 和 Qlib Alpha158 表达式因子库。
- Python 表达式 API：`col`、`lit`、算术/比较、rolling window、rank、
  neutralization、`replace_inputs()`。
- `compute_alphas` 和 `with_alphas`，覆盖批量 alpha 输出与原始 DataFrame 追加。
- DAG alpha evaluator，支持公共子表达式复用、slot 复用、节点级并行和 fused
  elementwise chain。
- `with_labels`、`evaluate`、`factor_correlation`、HTML 报告和交互式报告。

**计划中**

- 量化建模、策略构建和回测模块。
- 更完整的 API 参考和示例数据集。
- 发布到 PyPI 和 crates.io。

## 公开 API

- `qweave.compute_alphas(df, symbol_col, time_col, alphas, output_path=None)`
- `qweave.with_alphas(df, symbol_col, time_col, alphas)`
- `qweave.col(name)`、`qweave.lit(value)` 和表达式运算符
- `qweave.worldquant_alpha101(input_alias, alphas=None)`
- `qweave.qlib_alpha158(input_alias, alphas=None)`
- `qweave.with_labels(...)`、`qweave.evaluate(...)`
- `qweave.factor_correlation(...)`、`EvalResult.to_html(...)`、
  `EvalResult.view()`

输入规则：

- `symbol_col` 和 `time_col` 不能包含 null。
- 结构列不允许 NaN。
- 浮点输入列中的 null 会转成 NaN，让因子逻辑自然传播缺失值。
- 引擎会按 `(symbol_col, time_col)` 排序，并拒绝重复的 symbol-time。
- 字段映射存在于表达式树中：使用 `PyExpr.replace_inputs()`，或使用内置因子库的
  `input_alias` 参数。

## 文档

面向 GitHub 读者：

- [项目对比](docs/comparison.md)
- [性能与基准测试](docs/benchmark.md)
- [架构与设计取舍](docs/architecture.md)
- [Python 表达式 API](docs/expression_api.md)
- [因子评估](docs/factor_evaluation.md)
- [WorldQuant 101](docs/worldquant_alpha101.md)
- [Qlib Alpha158](docs/qlib_alpha158.md)

维护者入口：

- [开发者手册](docs/development.md)

本项目与 WorldQuant、Microsoft、Qlib 或 KunQuant 没有关联。

## 开发检查

提交前运行：

```powershell
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
uv run maturin develop
uv run python -m pytest
```

更多细节见 [开发者手册](docs/development.md)。

## License

MIT. See [LICENSE](LICENSE).
