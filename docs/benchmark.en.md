# Performance And Benchmarks

[Chinese](benchmark.md)

qweave's performance goal is not a standalone "faster" claim. The goal is to
make common factor-research paths on large panels reproducible, explainable, and
less dominated by Python loops.

The development environment moved from macOS to Windows, so historical
measurements are no longer a current reference. The results below were rerun in
the current Windows/PowerShell environment with the machine profile, commit SHA,
and exact commands recorded.

## Current Verified Result: Batch DAG vs Per-Factor Calls

Measured on 2026-07-10 with the qweave v0.4.1 (`3eec6fc`) compute engine:

- Windows 11 Pro 10.0.26200;
- AMD Ryzen 9 9950X, 16 cores / 32 logical processors;
- 61.7 GiB memory;
- Python 3.12.13;
- Rust 1.99.0-nightly;
- 5,000 symbols × 1,000 days = 5,000,000 deterministic synthetic OHLCV rows;
- all 158 Qlib Alpha158 factors, three measured runs after one warmup.

| Execution path | Best | Mean | Factor cells/s | Process peak RSS |
| --- | ---: | ---: | ---: | ---: |
| qweave batch DAG | 2.9630 s | 3.0399 s | 266,621,244 | 9,934.4 MiB |
| qweave per-factor calls | 50.4918 s | 51.4664 s | 15,646,102 | 8,404.1 MiB |

On this machine and synthetic panel, the batch DAG's best time was about
**17.0× lower** than running one DAG call per factor. Both paths assemble the
same complete 158-factor output; the batch path trades about 1.5 GiB more peak
memory for substantially higher throughput. This demonstrates the value of
shared-DAG execution, not a cross-machine performance guarantee.

Run each path in a separate process so one engine's historical peak does not
contaminate the other's RSS:

```powershell
uv run python scripts\bench_factor_engines.py --workload alpha158 --engines qweave --symbols 5000 --days 1000 --repeats 3 --warmups 1 --json results-batch.json

uv run python scripts\bench_factor_engines.py --workload alpha158 --engines qweave-sequential --symbols 5000 --days 1000 --repeats 3 --warmups 1 --json results-sequential.json
```

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
- Record best, mean, stdev, rows/s, cells/s, and process peak RSS.
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
- `--engines qweave-sequential` runs the one-DAG-call-per-factor baseline; run
  it separately from the batch path for a valid peak-RSS comparison.
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
peak_rss_mib:
compile_s:  # KunQuant only, when present
```

The script lives at
[scripts/bench_factor_engines.py](../scripts/bench_factor_engines.py).
