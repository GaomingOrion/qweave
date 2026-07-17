# qweave

[English](README.en.md)

[![CI](https://github.com/GaomingOrion/qweave/actions/workflows/ci.yml/badge.svg)](https://github.com/GaomingOrion/qweave/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/GaomingOrion/qweave)](https://github.com/GaomingOrion/qweave/releases)
[![Python](https://img.shields.io/badge/Python-3.10%2B-3776ab)](https://www.python.org/)
[![License](https://img.shields.io/github/license/GaomingOrion/qweave)](LICENSE)

**Polars 原生、Rust 加速的因子研究引擎。** qweave 在一条 DataFrame pipeline 中
完成可组合因子计算、无前视 forward-return 标签、IC/分位/换手评估和交互式报告。

- ⚡ **480 万行 × 158 因子 ≈ 2.2 秒：** Qlib Alpha158 全部因子在 6,000 股 ×
  800 天面板上一次执行完成（Ryzen 9 9950X 实测）。
- 🆚 **比 Qlib Alpha158DL 快 23.85×：** 同一面板、同一因子集、同为 32 线程的
  实测对比；端到端比 KunQuant 的 JIT C++ 路径快 2.56×，且不需要 C++ 工具链。
  命令与完整数据见[性能与基准测试](docs/benchmark.md)。
- 🧩 **450 个内置经典因子：** WorldQuant Alpha101 + Qlib Alpha158 + 国泰君安
  Alpha191，时序与
  截面算子同一套表达式 API，可筛选、改字段、混合后批量执行。
- 📊 **一行打开交互报告：** `result.view()` 启动内嵌 Vue + ECharts 界面，
  逐因子查看分位收益、月度 IC 和多空诊断。

![qweave 从行情面板到因子报告的完整流程](docs/assets/qweave-overview.svg)

> 带上你自己的 Polars 行情面板。保留现有数据管线。把昂贵的因子研究循环交给 Rust。

qweave 适合已经用 Parquet/Polars 管理数据，希望减少 Python 逐因子循环、重复 rolling
计算和多张矩阵对齐工作的量化研究者。它聚焦"因子是否携带稳定未来收益信息"的研究
层，不是数据供应商、撮合模拟器或完整投资平台。

## 安装

从 [GitHub Releases](https://github.com/GaomingOrion/qweave/releases/latest) 直链安装
（CPython 3.10+ stable ABI）：

```powershell
# Windows x64
python -m pip install https://github.com/GaomingOrion/qweave/releases/download/v0.6.0/qweave-0.6.0-cp310-abi3-win_amd64.whl
```

```bash
# Linux x86_64
pip install https://github.com/GaomingOrion/qweave/releases/download/v0.6.0/qweave-0.6.0-cp310-abi3-manylinux_2_17_x86_64.manylinux2014_x86_64.whl

# macOS arm64
pip install https://github.com/GaomingOrion/qweave/releases/download/v0.6.0/qweave-0.6.0-cp310-abi3-macosx_11_0_arm64.whl
```

其他平台（Linux aarch64、macOS x86_64）的 wheel 见
[Releases 页面](https://github.com/GaomingOrion/qweave/releases/latest)。
从源码构建见[开发者手册](docs/development.md)。

## 从行情面板到因子报告

仓库内置一个 80 个资产 × 320 个交易日的确定性合成面板。以下代码混合两个经典因子
与一个自定义表达式，并完成标签、评估和交互式报告：

```python
import polars as pl
import qweave as qw

df = pl.read_parquet("examples/data/sample_daily.parquet")

alphas = qw.worldquant_alpha101({}, alphas=["alpha13", "alpha101"])
alphas.append(
    (-(qw.col("close") / qw.col("close").delay(20) - qw.lit(1.0)))
    .alias("mean_reversion_20")
)

df = qw.with_alphas(df, "asset", "date", alphas)
df = qw.with_labels(
    df,
    symbol_col="asset",
    time_col="date",
    horizons=[1, 5, 20],
    entry_lag=1,
    tradable_col="tradable",
)

result = qw.evaluate(
    df,
    symbol_col="asset",
    time_col="date",
    factor_cols=["alpha13", "alpha101", "mean_reversion_20"],
    quantiles=5,
    min_cs_count=30,
    tradable_col="tradable_entry",
)

print(result.summary)
result.view()   # 在浏览器打开交互式评估报告
```

也可以直接运行完整示例：

```powershell
python examples\quickstart.py
```

<p align="center">
  <img src="docs/assets/report-demo.gif" width="720" alt="qweave 交互式因子评估报告：点选因子行，查看分位收益、多空净值和 IC tearsheet">
</p>

## 为什么是 qweave

- **DataFrame 进，DataFrame 出：** 直接接收并返回 Polars DataFrame，不需要
  先把面板转成 NumPy 数组、算完再拼回来，也不需要迁移到专用数据 provider。
- **截面因子和时序因子写在同一个表达式里：** `rank`、`group_neutralize` 等
  截面算子与 rolling 时序算子在同一个 DAG 中，一次 `compute_alphas` 跑完，
  不用像 provider/handler 工作流那样分阶段先取特征、再另行组织截面计算。
- **一次执行整批因子：** 多个表达式进入同一 Rust DAG，统一完成公共子表达式复用、
  中间 slot 复用、elementwise chain 融合和节点级并行。
- **450 个可组合经典因子：** WorldQuant Alpha101、Qlib Alpha158 和国泰君安
  Alpha191 使用和自定义表达式相同的 API，可筛选、改字段、混合并批量执行。
- **报告直接可用：** `result.view()` 打开 Vue + ECharts 交互界面，逐因子下钻；
  千级因子还可流式写入 Parquet。

## 性能：与 Qlib、KunQuant 同口径对比

实测于 Windows 11 / Ryzen 9 9950X（16 核 32 线程）/ 61.7 GiB 内存，同一份
6,000 股 × 800 天（480 万行）合成 OHLCV 面板，所有引擎 32 线程，1 次 warmup
后测量 3 次取最佳。KunQuant 为 f64 端到端口径（含输入/输出 DataFrame 转换与
JIT 编译）：

| 工作负载 | qweave | 对比引擎 | 结论 |
| --- | ---: | ---: | --- |
| Qlib Alpha158 全部 158 因子 | **2.24 s** | Qlib Alpha158DL：53.37 s | 快 23.85×，峰值内存少约 46% |
| WorldQuant Alpha101（82 因子） | **3.11 s** | KunQuant f64：7.95 s | 端到端快 2.56×，峰值内存少约 31%，无需 C++ 工具链 |

快的来源：排序、rolling、截面算子和评估统计都在 Rust 热路径；整批表达式进入
同一 DAG，公共子表达式复用、slot 复用和节点级并行由引擎统一管理。完整环境、
命令与口径见[性能与基准测试](docs/benchmark.md)。

## 因子评估：无前视、一次全套

```text
信号日 T ── entry_lag ──> 入场日 T+1 ── horizon h ──> 退出日 T+1+h
```

- 默认 T+1 入场，无 look-ahead；停牌缺行不会悄悄缩短持有期，入场日可交易性
  自动对齐回信号日。
- 一次 `evaluate` 产出 IC/RankIC、分位收益、换手、rank 自相关和多空诊断，
  重叠持有期的显著性用 Newey–West t-stat 校正。
- 结果 `result.view()` 直接打开交互报告，千级因子可流式写 Parquet。

完整口径与参数见[因子评估](docs/factor_evaluation.md)。

## 什么时候选 qweave

- 行情数据已经在 Parquet/Polars 管线里，不想为了算因子迁移到专用数据格式，
  或拆成 provider/handler 的分阶段工作流。
- 一次要计算和评估几十到上千个因子，Python 逐因子循环已经成为研究迭代的瓶颈。
- 在搭建自动化因子挖掘或投研 Agent 系统：易写的表达式 API、高吞吐批量执行和
  统一评估口径，天然适合作为 Agent「生成 → 计算 → 评估」实验闭环的底层内核。

需要数据下载、模型训练和回测实验管理的完整平台时，Qlib 更合适——qweave 也可以
作为这类平台或 Agent 系统里的因子计算与评估内核嵌入使用。详细比较见
[项目对比](docs/comparison.md)。

## Roadmap

- **面向投研 Agent 的实验内核：** 易写的表达式 API + 高吞吐批量执行 + 统一
  评估口径，支撑「生成因子 → 批量计算 → 严格评估」的自动化因子挖掘闭环。
- 发布到 PyPI，安装收敛为一行 `pip install qweave`。
- 扩充内置因子库与时序/截面算子覆盖。
- 交互式报告持续增强，作为评估结果的默认查看方式。

## 文档路径

从[文档首页](docs/index.md)按顺序阅读：

1. [可运行示例](examples/README.md)
2. [Python 表达式 API](docs/expression_api.md)
3. [WorldQuant 101](docs/worldquant_alpha101.md) / [Qlib Alpha158](docs/qlib_alpha158.md) /
   [国泰君安 Alpha191](docs/gtja_alpha191.md)
4. [因子评估](docs/factor_evaluation.md)
5. [架构](docs/architecture.md) / [性能与基准测试](docs/benchmark.md)

## 项目状态

qweave 的因子计算、标签、评估和报告链路已经可用，API 处于 pre-1.0 阶段。
欢迎阅读 [CONTRIBUTING](CONTRIBUTING.md) 参与开发。项目与国泰君安证券、WorldQuant、
Microsoft、Qlib 或 KunQuant 没有关联。

## License

MIT. See [LICENSE](LICENSE).
