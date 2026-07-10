# qweave

[English](README.en.md)

[![CI](https://github.com/GaomingOrion/qweave/actions/workflows/ci.yml/badge.svg)](https://github.com/GaomingOrion/qweave/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/GaomingOrion/qweave)](https://github.com/GaomingOrion/qweave/releases)
[![Python](https://img.shields.io/badge/Python-3.10%2B-3776ab)](https://www.python.org/)
[![License](https://img.shields.io/github/license/GaomingOrion/qweave)](LICENSE)

**Polars 原生、Rust 加速的因子研究引擎。** qweave 在一条 DataFrame pipeline 中
完成可组合因子计算、无前视 forward-return 标签、IC/分位/换手评估和交互式报告。

> 带上你自己的 Polars 行情面板。保留现有数据管线。把昂贵的因子研究循环交给 Rust。

![qweave 从行情面板到因子报告的完整流程](docs/assets/qweave-overview.svg)

qweave 适合已经用 Parquet/Polars 管理数据，希望减少 Python 逐因子循环、重复 rolling
计算和多张矩阵对齐工作的量化研究者。它聚焦“因子是否携带稳定未来收益信息”的研究
层，不是数据供应商、撮合模拟器或完整投资平台。

## 安装

qweave 尚未发布到 PyPI。v0.4.1 已提供 CPython 3.10+ stable ABI 的 Windows、Linux
和 macOS wheels，可从 [GitHub Releases](https://github.com/GaomingOrion/qweave/releases/latest)
下载对应文件后安装。例如 Windows x64：

```powershell
python -m pip install .\qweave-0.4.1-cp310-abi3-win_amd64.whl
```

从源码开发或试用：

```powershell
git clone https://github.com/GaomingOrion/qweave.git
Set-Location qweave
uv sync --dev --locked
uv run maturin develop --uv --release
```

源码构建需要 Python 3.10+、`uv` 和仓库固定的 Rust nightly。更多信息见
[开发者手册](docs/development.md)。

## 从行情面板到因子报告

仓库内置一个 80 个资产 × 320 个交易日的确定性合成面板。以下代码混合两个经典因子
与一个自定义表达式，并完成标签、评估和报告导出：

```python
import polars as pl
import qweave as qf

df = pl.read_parquet("examples/data/sample_daily.parquet")

alphas = qf.worldquant_alpha101({}, alphas=["alpha13", "alpha101"])
alphas.append(
    (-(qf.col("close") / qf.col("close").delay(20) - qf.lit(1.0)))
    .alias("mean_reversion_20")
)

df = qf.with_alphas(df, "asset", "date", alphas)
df = qf.with_labels(
    df,
    symbol_col="asset",
    time_col="date",
    horizons=[1, 5, 20],
    entry_lag=1,
    tradable_col="tradable",
)

result = qf.evaluate(
    df,
    symbol_col="asset",
    time_col="date",
    factor_cols=["alpha13", "alpha101", "mean_reversion_20"],
    quantiles=5,
    min_cs_count=30,
    tradable_col="tradable_entry",
)

print(result.summary)
result.to_html("qweave-report.html")
```

也可以直接运行完整示例：

```powershell
uv run python examples\quickstart.py
```

合成面板的 5 日实际输出如下；它用于验证流程，不代表真实市场表现：

| factor | RankIC mean | RankIC IR | top-bottom spread mean |
| --- | ---: | ---: | ---: |
| `alpha13` | 0.008565 | 0.070432 | 0.000899 |
| `alpha101` | -0.002198 | -0.019618 | -0.000278 |
| `mean_reversion_20` | 0.022806 | 0.181762 | 0.002273 |

<p align="center">
  <img src="docs/assets/report-demo.png" width="420" alt="qweave 因子评估报告：汇总表、分位收益和月度 IC">
</p>

## 为什么是 qweave

- **一条 DataFrame pipeline：** 因子、标签和评估都围绕输入 Polars DataFrame
  追加和流转，减少独立矩阵、重复转换和索引错位。
- **一次执行整批因子：** 多个表达式进入同一 Rust DAG，统一完成公共子表达式复用、
  中间 slot 复用、elementwise chain 融合和节点级并行。
- **259 个可组合经典因子：** WorldQuant Alpha101 与 Qlib Alpha158 使用和自定义
  表达式相同的 API，可筛选、改字段、混合并批量执行。
- **明确的研究语义：** union calendar、`entry_lag`、入场日可交易性、确定性分箱和
  重叠持有期 Newey–West t-stat 都有公开定义。
- **报告直接可用：** `EvalResult.to_html()` 导出自包含报告，`view()` 打开 Vue +
  ECharts 交互界面；千级因子还可流式写入 Parquet。

## 批量 DAG 的实际收益

2026-07-10 在 Windows 11、Ryzen 9 9950X、61.7 GiB 内存上重新测量 5,000 个股票 ×
1,000 天 × Alpha158 全部 158 因子。两条路径都组装相同的完整输出：

| 执行方式 | 最佳耗时 | 平均耗时 | 进程峰值 RSS |
| --- | ---: | ---: | ---: |
| qweave 批量 DAG | **2.9630 s** | 3.0399 s | 9,934.4 MiB |
| qweave 逐因子调用 | 50.4918 s | 51.4664 s | 8,404.1 MiB |

这次合成面板测量中，批量 DAG 的最佳耗时约为逐因子路径的 **1/17.0**，代价是约
1.5 GiB 更高的峰值内存。结果依赖机器、版本和数据形态；完整环境、命令与口径见
[性能与基准测试](docs/benchmark.md)。

## 评估口径先于漂亮数字

默认标签定义为：

```text
信号日 T ── entry_lag ──> 入场日 T+1 ── horizon h ──> 退出日 T+1+h
```

- 日期偏移使用全市场 union calendar，不会因为某个资产缺行而偷偷压缩持有期。
- `tradable_entry` 把入场日的可交易状态回移到信号日，明确样本为何可用。
- 同时计算 Pearson IC 与 Spearman RankIC、分位收益、换手、rank autocorrelation
  和多空组合诊断。
- 重叠 forward returns 的均值检验使用 Newey–West t-stat。

详细定义和非目标见[因子评估](docs/factor_evaluation.md)。

## 与其他项目的边界

| 如果你需要 | 更合适的选择 |
| --- | --- |
| 数据、模型、组合、回测和执行的完整 AI 量化平台 | Qlib |
| 将表达式编译为 C++/JIT 执行路径 | KunQuant |
| pandas 生态中的传统单因子分析工作流 | Alphalens 类工具 |
| 在现有 Polars 数据管线中完成批量因子计算、严格标签和评估报告 | **qweave** |

qweave 可以独立使用，也可以作为更大研究平台里的因子研究内核。详细比较见
[项目对比](docs/comparison.md)。

## 文档路径

从[文档首页](docs/index.md)按顺序阅读：

1. [可运行示例](examples/README.md)
2. [Python 表达式 API](docs/expression_api.md)
3. [WorldQuant 101](docs/worldquant_alpha101.md) / [Qlib Alpha158](docs/qlib_alpha158.md)
4. [因子评估](docs/factor_evaluation.md)
5. [架构](docs/architecture.md) / [性能与基准测试](docs/benchmark.md)

## 项目状态

qweave 的因子计算、标签、评估和报告链路已经可用，API 仍处于 pre-1.0 阶段。
项目当前不模拟撮合、滑点、退出侧流动性或完整策略资金曲线。

欢迎阅读 [CONTRIBUTING](CONTRIBUTING.md) 参与开发。项目与 WorldQuant、Microsoft、
Qlib 或 KunQuant 没有关联。

## License

MIT. See [LICENSE](LICENSE).
