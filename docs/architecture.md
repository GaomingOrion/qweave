# 架构与设计取舍

[English](architecture.en.md)

qweave 的架构围绕一个简单目标：**让研究员写 Python 表达式，让性能敏感路径留在
Rust。** Python API 负责易用性，Rust workspace 负责面板布局、表达式求值、评估
统计和报告输出。

## 设计原则

- **DataFrame 进，DataFrame 出。** qweave 不要求用户先迁移到专用数据 provider。
  只要数据已经在 Polars DataFrame 或 Parquet workflow 中，就可以直接接入。
- **表达式是研究接口。** 内置因子和自定义因子都用同一种 `PyExpr` 表示，便于筛选、
  组合、字段映射和批量执行。
- **批量计算优先。** alpha 通常不是一个个孤立运行，而是一批有大量重叠窗口和输入
  的公式。默认 evaluator 会把整批表达式降成共享 DAG。
- **口径显式。** 面板排序、重复 symbol-time 检查、NaN 传播、窗口 warmup、标签
  calendar 和可交易样本过滤都在文档和测试中固定下来。

## 数据流

```text
Polars DataFrame
  -> Python API validates request shape
  -> Rust panel layout sorts and checks (symbol, time)
  -> expression DAG evaluates alphas over the full panel
  -> labels/evaluation/reporting reuse the same aligned frame
  -> Polars DataFrame, Parquet, HTML, or interactive report
```

`with_alphas` 适合保留原始 DataFrame shape：它会计算完整面板，再把结果按原始行序
scatter 回输入。`compute_alphas` 适合大批量因子产出：它直接返回 tidy
`(time, symbol)` 结果，或写出 Parquet。

## Alpha evaluator

默认 alpha evaluator 是 DAG 引擎。它会把请求的表达式降成共享 DAG，用于：

- 复用公共子表达式，例如多个 alpha 都使用同一个 rolling mean。
- 复用中间 slot，减少临时数组分配。
- 对可融合的 elementwise chain 做连续执行。
- 在已验证有效的位置使用节点级并行。

Tree evaluator 作为独立参考实现保留，便于排查 DAG 优化引入的问题：

```powershell
$env:QWEAVE_ENGINE = "tree"
uv run python -m pytest
Remove-Item Env:\QWEAVE_ENGINE
```

有效值是 `dag` 和 `tree`。

## Workspace 结构

- `qweave-core`：面板布局、列校验、alpha 表达式求值和结果 sink。
- `qweave-factors`：内置 alpha builder，包括 WorldQuant 101 和 Qlib Alpha158。
- `qweave-eval`：forward-return 标签、因子评估、相关性、报告表和 HTML 输出。
- `qweave-server`：交互式评估报告的 Axum 服务。
- `qweave-py`：PyO3 扩展模块，对 Python 暴露为 `qweave`。

## 当前边界

qweave 聚焦因子计算与评估这一层：不模拟撮合、滑点、资金曲线或组合约束。
API 处于 pre-1.0 阶段，1.0 前字段名或统计输出可能调整。
