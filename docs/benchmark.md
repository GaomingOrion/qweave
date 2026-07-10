# Benchmarks

This document records cross-engine factor-computation benchmarks for qweave.
The benchmark script uses deterministic synthetic OHLCV panels so runs are
repeatable without external market data.

## Scope

The published comparison focuses on engines with representative factor-library
execution paths:

- Qlib `Alpha158DL`, compared with qweave's `qlib_alpha158()` expression
  library.
- KunQuant Alpha101 JIT execution, compared with qweave's
  `worldquant_alpha101()` expression library.


## Environment Notes

Last measured: 2026-07-08.

- Host: Windows, PowerShell, 32 logical cores, 64 GB RAM.
- Dataset size for the recorded run: 6000 symbols x 800 days, or 4,800,000
  rows.
- Python dependencies are managed with `uv`.
- qweave extension was built with `uv run maturin develop --uv --release`.
- Qlib and KunQuant's dependency/JIT paths are not robust to non-ASCII
  characters in the user profile or workspace path. If your machine's paths
  are pure ASCII (as on this run), no workaround is needed. Otherwise, point
  `TMP`/`TEMP` (and optionally `UV_CACHE_DIR`/`UV_PYTHON_INSTALL_DIR`) at an
  ASCII-only directory before running, e.g. `$env:TMP = "C:\qweave-bench-tmp"`.
- Qlib pays a one-time cold-start cost (first-ever import/bytecode compile
  and internal cache setup) the first time it runs in a fresh `uv run --with
  pyqlib` environment; that cost is unrelated to dataset size and can dwarf
  the actual per-call work (observed: >60s cold vs a few seconds once warm).
  Always use at least `--warmups 1` for Qlib runs, otherwise the reported
  time mixes cold-start overhead with computation.

Shared setup:

```powershell
uv run maturin develop --uv --release
```

## Qlib Alpha158

Command:

```powershell
uv run --frozen --with pyqlib python scripts\bench_factor_engines.py --workload alpha158 --engines qweave,qlib --symbols 6000 --days 800 --repeats 3 --warmups 1 --threads 1
```

Results:

| engine | workload | rows | factors | best_s | mean_s | cells/s |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| qweave | alpha158 | 4,800,000 | 158 | 2.0587 | 2.2114 | 368,384,847 |
| qlib | alpha158 | 4,800,000 | 158 | 215.5896 | 222.7666 | 3,517,794 |

Interpretation: qweave is about 105x faster than Qlib by best elapsed time on
this generated-provider Alpha158DL setup, computing the full 158-factor set
(all rolling windows), not a reduced subset. This comparison measures Qlib
through its real `Alpha158DL` data-handler path over a generated local Qlib
binary provider, not just isolated arithmetic kernels.

## KunQuant WorldQuant101

Command:

```powershell
uv run --frozen --with KunQuant --with setuptools python scripts\bench_factor_engines.py --workload worldquant101 --engines qweave,kunquant --symbols 6000 --days 800 --repeats 3 --warmups 1 --threads 1
```

Results:

| engine | workload | rows | factors | best_s | cells/s | compile_s |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| qweave | worldquant101 | 4,800,000 | 82 | 3.2740 | 120,218,919 | - |
| kunquant | worldquant101 | 4,800,000 | 82 | 29.3771 | 13,398,202 | 3.2493 |

Interpretation: qweave is about 9.0x faster than KunQuant on end-to-end
compile-plus-run time. If KunQuant's observed compile phase is subtracted,
KunQuant runtime is about 26.13 seconds, and qweave is still about 8.0x
faster on this run.

The current KunQuant package exposes 82 of the WorldQuant Alpha101 formulas used
by this benchmark. The KunQuant timing includes JIT compilation because that is
the end-to-end cost users pay when compiling a fresh expression bundle.

## Reproduction Options

Useful script options:

- `--symbols` and `--days` scale the synthetic panel.
- `--repeats` and `--warmups` control timing runs.
- `--names` selects a comma-separated factor subset.
- `--json results.json` saves machine-readable results.

The benchmark script is [scripts/bench_factor_engines.py](../scripts/bench_factor_engines.py).
