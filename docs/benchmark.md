# 性能与基准测试

[English](benchmark.en.md)

qweave 的性能目标不是一句孤立的“更快”，而是让大面板上的常见因子研究路径可以
稳定复现、容易解释、少受 Python 循环影响。

当前公开仓库不发布旧性能数字。项目开发环境已从 macOS 迁移到 Windows，历史测量
不再作为当前参考；新的性能结论必须在当前 Windows/PowerShell 环境重新测量，并
记录日期、机器配置、commit SHA 和完整命令。

## 为什么可能快

- **Rust 热路径：** 排序、校验、rolling window、截面算子、DAG evaluator 和评估
  统计都在 Rust 侧运行。
- **DAG 批量执行：** 多个 alpha 共享相同子表达式时，默认 evaluator 会复用计算
  结果，而不是把每个 alpha 当作孤立公式反复计算。
- **slot 复用：** 中间数组生命周期由 evaluator 管理，减少无谓分配。
- **Polars 入口和出口：** 用户仍然在 DataFrame 世界里工作，不需要为了性能把流程
  拆成零散的 NumPy buffer 管理代码。

这些是设计目标和实现路径，不等于跨机器、跨版本的固定性能承诺。请使用下面命令在
你的环境里测量。

## 公平对比原则

- 使用相同的合成 OHLCV 面板，不依赖真实市场数据。
- qweave 扩展使用 release 模式构建。
- 至少保留一次 warmup，避免把首次 import、bytecode compile 或 cache 初始化混入
  计算时间。
- 同时记录 best、mean、stdev、rows/s 和 cells/s。
- 比较 KunQuant 时保留 `compile_s`，因为编译新表达式 bundle 是端到端体验的一部分。

## 环境准备

```powershell
uv sync --dev
uv run maturin develop --uv --release
```

如果本机用户目录或 workspace 路径包含非 ASCII 字符，Qlib 或 KunQuant 的依赖/JIT
路径可能不够稳定。可把临时目录指向 ASCII-only 路径：

```powershell
New-Item -ItemType Directory -Force C:\qweave-bench-tmp | Out-Null
$env:TMP = "C:\qweave-bench-tmp"
$env:TEMP = "C:\qweave-bench-tmp"
```

## Qlib Alpha158

该路径比较 Qlib `Alpha158DL` 与 qweave 的 `qlib_alpha158()` 表达式库。

```powershell
uv run --frozen --with pyqlib python scripts\bench_factor_engines.py --workload alpha158 --engines qweave,qlib --symbols 6000 --days 800 --repeats 3 --warmups 1 --threads 1 --json results-alpha158.json
```

## KunQuant WorldQuant101

该路径比较 KunQuant Alpha101 JIT 与 qweave 的 `worldquant_alpha101()` 表达式库。

```powershell
uv run --frozen --with KunQuant --with setuptools python scripts\bench_factor_engines.py --workload worldquant101 --engines qweave,kunquant --symbols 6000 --days 800 --repeats 3 --warmups 1 --threads 1 --json results-worldquant101.json
```

## 常用参数

- `--symbols` 和 `--days` 控制合成面板规模。
- `--repeats` 和 `--warmups` 控制计时次数。
- `--names` 选择逗号分隔的因子子集。
- `--threads` 控制可选 runner 的线程数。
- `--json results.json` 保存机器可读结果。

结果发布建议：

```text
date:
commit:
machine:
command:
engine:
workload:
symbols:
days:
factors:
best_s:
mean_s:
stdev_s:
rows_per_s:
cells_per_s:
compile_s:  # KunQuant only, when present
```

脚本位置：[scripts/bench_factor_engines.py](../scripts/bench_factor_engines.py)。
