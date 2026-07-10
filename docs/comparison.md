# 项目对比

[English](comparison.en.md)

qweave 的定位是：**面向因子研究的轻量 Rust/Polars 内核**。它不试图一次性接管
数据平台、模型训练、组合优化和实盘执行，而是把最常见、最容易变慢、也最容易写
散的部分做好：因子表达式、批量计算、标签、评估和报告。

## 一句话区别

- **Qlib** 是完整 AI 量化平台；qweave 是可以嵌入现有数据管线的因子研究内核。
- **KunQuant** 是表达式编译与 JIT 执行器；qweave 是 Python 友好的 Rust/Polars
  workflow，不要求研究员管理 C++ 编译产物。
- **Alphalens 类工具** 更偏分析报表；qweave 把计算、标签和评估放在同一条 pipeline。

## 对比表

| 维度 | qweave | Qlib | KunQuant |
| --- | --- | --- | --- |
| 主要目标 | 因子加工、因子评估，并向建模/策略/回测扩展 | AI 量化研究平台，覆盖更完整投资链路 | 批量表达式优化、代码生成和 JIT 执行 |
| 输入体验 | Polars DataFrame 直接进入 Python API | 通常围绕 Qlib 数据 provider 和 handler | 表达式构图后编译执行 |
| 执行核心 | Rust kernel + Polars/PyO3 绑定 | Python 生态为主，部分路径依赖 pandas/NumPy | 生成优化 C++ 并通过 runner 执行 |
| 因子库 | WorldQuant 101、Qlib Alpha158，均为可组合表达式 | Alpha158/Alpha360 等 handler | 预置 Alpha101 路径 |
| 评估闭环 | `with_labels`、`evaluate`、IC/RankIC、分位收益、换手、报告 | 平台内有完整实验和回测能力 | 主要聚焦表达式计算，不是评估框架 |
| 适合场景 | 已有数据在 DataFrame/Parquet 中，希望快速批量计算和评估因子 | 希望使用完整研究平台和现成模型/实验管理 | 需要把表达式 bundle 编译成高性能执行路径 |
| 当前边界 | pre-1.0，建模/策略/回测仍在规划 | 平台较完整，但接入成本更高 | JIT/编译链路更强，但 workflow 需要额外拼装 |

## qweave 的优势在哪里

**减少 glue code。** 研究员可以在同一个 DataFrame 上完成 alpha、label、evaluation，
而不是手动维护多个宽表、长表和索引对齐。

**批量因子更自然。** 内置因子库返回普通表达式对象，所以可以只取一个子集、重命名
输入字段、加入自定义表达式，然后一次性提交给 evaluator。

**把性能优化放在库里。** 用户写的是 Python 表达式，执行时进入 Rust 侧的 DAG：
公共子表达式复用、slot 复用、节点级并行和 elementwise chain 融合都不需要用户
手动管理。

**评估口径明确。** `with_labels` 使用面板级日期网格，缺失 symbol-day 不会悄悄
压缩持有期；`evaluate` 明确区分 factor-valid、pair-valid、可交易样本和
min cross-section count。

## 不适合什么

- 如果你需要开箱即用的数据下载、模型训练模板、组合优化和回测实验管理，Qlib
  目前覆盖面更完整。
- 如果你的核心诉求是把固定表达式 bundle 编译成极致优化的 C++ 代码，KunQuant
  是更直接的工具。
- 如果你只想做一次性小样本 notebook 分析，pandas/Alphalens 风格工具可能更轻。

qweave 更适合的缝隙是：你已经有自己的数据、研究流程和工程约束，但希望因子计算
与评估这一段更快、更统一、更容易复现。

## 参考项目

- [Microsoft Qlib](https://github.com/microsoft/qlib)
- [KunQuant](https://github.com/Menooker/KunQuant)
