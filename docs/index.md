# qweave 文档

[English](index.en.md)

qweave 是 Polars 原生、Rust 加速的因子研究引擎：批量因子计算、无前视标签、
IC/分位/换手评估和交互式报告在一条 DataFrame pipeline 中完成。在 480 万行面板上
一次执行全部 158 个 Qlib Alpha158 因子约 2.2 秒（[实测数据](benchmark.md)）。

文档按一次因子研究的实际顺序组织。第一次使用建议从仓库首页的
[快速开始](../README.md#从行情面板到因子报告)和可运行的
[示例](../examples/README.md)开始。

## 学习路径

1. **安装和完整示例：** [README 快速开始](../README.md#从行情面板到因子报告)。
2. **编写与组合因子：** [Python 表达式 API](expression_api.md)。
3. **使用经典因子库：** [WorldQuant 101](worldquant_alpha101.md)、
   [Qlib Alpha158](qlib_alpha158.md) 和 [国泰君安 Alpha191](gtja_alpha191.md)。
4. **构造标签和评估因子：** [因子评估](factor_evaluation.md)。
5. **理解执行原理：** [架构与设计取舍](architecture.md)。
6. **运行大规模实验：** [性能与基准测试](benchmark.md)。

## 定位与对比

- [项目对比](comparison.md)说明 qweave 与 Qlib、KunQuant 和 pandas/Alphalens
  类工具各自的适用场景，以及同面板的实测性能对比。
- qweave 聚焦因子计算与因子诊断这一层，可以独立使用，也可以嵌入更大的研究平台。
- 维护者和贡献者请阅读[开发者手册](development.md)与
  [CONTRIBUTING](../CONTRIBUTING.md)。
