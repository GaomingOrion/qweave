# Performance And Benchmarks

[Chinese](benchmark.md)

qweave's performance goal is not a standalone "faster" claim. The goal is to
make common factor-research paths on large panels reproducible, explainable, and
less dominated by Python loops.

The public repository does not publish old performance numbers. The development
environment moved from macOS to Windows, so historical measurements are no
longer a current reference. New performance claims should be re-measured in the
current Windows/PowerShell environment with the date, machine profile, commit
SHA, and exact command.

## Why It Can Be Fast

- **Rust hot path:** sorting, validation, rolling windows, cross-sectional
  operators, the DAG evaluator, and evaluation statistics run on the Rust side.
- **Batch DAG execution:** when multiple alphas share subexpressions, the default
  evaluator reuses results instead of treating every alpha as an isolated formula.
- **Slot reuse:** intermediate arrays are managed by the evaluator to reduce
  unnecessary allocation.
- **Polars in and out:** users stay in DataFrame workflows instead of splitting
  research code into ad hoc NumPy buffers for performance.

These are design goals and implementation paths, not fixed performance promises
across machines and versions. Use the commands below to measure your environment.

## Fair Comparison Principles

- Use the same deterministic synthetic OHLCV panel, without external market data.
- Build the qweave extension in release mode.
- Keep at least one warmup run so first import, bytecode compilation, or cache
  initialization is not mixed into compute time.
- Record best, mean, stdev, rows/s, and cells/s.
- Keep `compile_s` for KunQuant, because compiling a new expression bundle is
  part of the end-to-end experience.

## Environment Setup

```powershell
uv sync --dev
uv run maturin develop --uv --release
```

If your user profile or workspace path contains non-ASCII characters, Qlib or
KunQuant dependency/JIT paths may be unstable. Point temporary directories at an
ASCII-only path:

```powershell
New-Item -ItemType Directory -Force C:\qweave-bench-tmp | Out-Null
$env:TMP = "C:\qweave-bench-tmp"
$env:TEMP = "C:\qweave-bench-tmp"
```

## Qlib Alpha158

This path compares Qlib `Alpha158DL` with qweave's `qlib_alpha158()` expression
library.

```powershell
uv run --frozen --with pyqlib python scripts\bench_factor_engines.py --workload alpha158 --engines qweave,qlib --symbols 6000 --days 800 --repeats 3 --warmups 1 --threads 1 --json results-alpha158.json
```

## KunQuant WorldQuant101

This path compares KunQuant Alpha101 JIT execution with qweave's
`worldquant_alpha101()` expression library.

```powershell
uv run --frozen --with KunQuant --with setuptools python scripts\bench_factor_engines.py --workload worldquant101 --engines qweave,kunquant --symbols 6000 --days 800 --repeats 3 --warmups 1 --threads 1 --json results-worldquant101.json
```

## Useful Options

- `--symbols` and `--days` scale the synthetic panel.
- `--repeats` and `--warmups` control timing runs.
- `--names` selects a comma-separated factor subset.
- `--threads` controls the optional runner thread count.
- `--json results.json` saves machine-readable results.

Suggested result record:

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

The script lives at
[scripts/bench_factor_engines.py](../scripts/bench_factor_engines.py).
