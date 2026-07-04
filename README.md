# qfactors

qfactors is a Rust factor and alpha computation engine with Python bindings for
Polars panels. It is built for research workflows that need Python ergonomics
without moving the hot path out of Rust.

## Why qfactors

- **Polars-native Python workflow:** pass in a Polars DataFrame and get a Polars
  DataFrame back. `with_alphas` appends results in the original row order, while
  `compute_alphas` emits a full `(time, symbol)` panel for downstream scans.
- **Rust execution core:** panel sorting, validation, rolling windows,
  cross-sectional operators, and expression evaluation run in Rust with rayon
  parallelism where it is already proven useful.
- **Expression API for research iteration:** compose alphas with
  `qfactors.col("close")`, `qfactors.lit(1.0)`, operators, windows, ranks,
  neutralization, and `replace_inputs()` templates.
- **Factor libraries built in:** `worldquant_alpha101()` returns `alpha1`
  through `alpha101`, and `qlib_alpha158()` returns the Qlib Alpha158 feature
  set — both as expression objects, with documented project defaults and input
  aliasing for adjusted or vendor-specific column names.
- **Regression guarded:** every built-in alpha is checked against a frozen
  synthetic golden fixture at `1e-8` tolerance, so engine changes are reviewed
  against stable numerical output.

- **Factor evaluation (experimental):** `with_labels` appends forward-return
  labels, `evaluate` scores factor columns for predictive power (IC / RankIC
  with Newey–West t-stats), monotonicity (quantile returns), and tradability
  (turnover, a staggered long-short portfolio), and `factor_correlation`
  measures redundancy — all in the same single-DataFrame pipeline. See
  [docs/factor_evaluation.md](docs/factor_evaluation.md). Calibers are validated
  against an independent numpy reference and alphalens-reloaded, but the surface
  is experimental until a frozen golden fixture lands.

The project is early-stage. APIs are usable for experimentation and internal
research workflows, but should be treated as pre-1.0.

## Roadmap

qfactors is pre-1.0 and under active development. The current focus is the
performance of the alpha expression engine while keeping results numerically
stable — a frozen golden baseline guards every change at `1e-8` tolerance.

**Done**

- v0.1.0 baseline frozen behind a golden regression safety net.
- `O(n)` rolling-window kernels (Welford variance, monotonic-deque min/max,
  rolling sum/mean/decay) replacing per-window recomputation.
- Global allocator (jemalloc on Unix, mimalloc on Windows).
- WorldQuant 101 (`alpha1`–`alpha101`) and Qlib Alpha158 factor libraries, both
  as plain expression builders.
- v0.3.0 Python expression API: `PyExpr`, `with_alphas`, full-history
  `compute_alphas`, input replacement templates, and type stubs.
- DAG evaluator — hash-consed common-subexpression elimination, slot reuse,
  node-level parallelism, and fused elementwise chains — promoted to the default
  engine after benchmarking faster than the tree evaluator across WorldQuant 101
  and Alpha158. The tree evaluator remains available (`QF_ENGINE=tree`) as an
  independent reference.

**Planned**

- Node-level parallelism and fewer layout transposes in the evaluator.
- Factor evaluation suite (`with_labels` / `evaluate` / `factor_correlation`) —
  landed as experimental; promotion pending a frozen golden fixture, a browser
  dashboard, and parquet factor-source streaming for thousand-factor runs.
- Publish to PyPI and crates.io.
- Expanded factor / alpha API documentation.

## Installation

This repository currently targets source builds. It is not published to PyPI or
crates.io yet.

Prerequisites:

- Python 3.10 or newer
- `uv`
- Rust nightly with `rustfmt` and `clippy`

Set up a local development environment:

```bash
uv sync --dev
uv run maturin develop
```

The repository includes `rust-toolchain.toml`, so Cargo will use the pinned
nightly toolchain automatically.

## Quick Start

```python
import polars as pl
import qfactors

df = pl.DataFrame(
    {
        "asset": ["A", "A", "B", "B"],
        "time": [1, 2, 1, 2],
        "open": [10.0, 11.0, 20.0, 19.0],
        "close": [11.0, 12.0, 19.0, 21.0],
        "high": [12.0, 13.0, 21.0, 22.0],
        "low": [9.0, 10.0, 18.0, 18.5],
        "volume": [100.0, 120.0, 80.0, 90.0],
    }
)

alphas = qfactors.worldquant_alpha101({}, alphas=["alpha101"])
out = qfactors.compute_alphas(
    df=df,
    symbol_col="asset",
    time_col="time",
    alphas=alphas,
)

df_with_alpha = qfactors.with_alphas(
    df=df,
    symbol_col="asset",
    time_col="time",
    alphas=[
        (
            (qfactors.col("close") - qfactors.col("open"))
            / (qfactors.col("high") - qfactors.col("low") + qfactors.lit(0.001))
        ).alias("intraday_return")
    ],
)
```

`compute_alphas` computes expression alphas over the full panel and returns a
Polars DataFrame by default, or a summary dict when `output_path` is provided.
`with_alphas` appends expression outputs to the input DataFrame in its original
row order.

## Public API

Python functions:

- `qfactors.compute_alphas(df, symbol_col, time_col, alphas, output_path=None)`
- `qfactors.with_alphas(df, symbol_col, time_col, alphas)`
- `qfactors.col(name)`, `qfactors.lit(value)`, and expression operators
- `qfactors.worldquant_alpha101(input_alias, alphas=None)`
- `qfactors.qlib_alpha158(input_alias, alphas=None)`
- `qfactors.with_labels(...)`, `qfactors.evaluate(...)`,
  `qfactors.factor_correlation(...)` (experimental — see
  [docs/factor_evaluation.md](docs/factor_evaluation.md))

Input rules:

- `symbol_col` and `time_col` cannot contain nulls.
- Structural NaN values are rejected.
- Float input nulls are converted to NaN so factor logic can propagate missing
  data.
- The engine sorts panel rows by `(symbol_col, time_col)` and rejects duplicate
  symbol-time pairs.
- Field remapping lives in the expression tree: use `PyExpr.replace_inputs()` or
  the `input_alias` argument of `worldquant_alpha101()` / `qlib_alpha158()`.

Memory note:

- `with_alphas` preserves original input row order by scattering each evaluated
  alpha column into a new full-size output buffer before appending it. For very
  wide alpha batches, `compute_alphas` is the more memory-lean executor because
  it can move evaluated columns directly into the result frame.

## Alpha Engine

`compute_alphas` uses the DAG evaluator by default. The tree evaluator can be
selected explicitly — it serves as an independent reference implementation:

```bash
QF_ENGINE=tree uv run pytest
```

Valid values are `dag` and `tree`; invalid values raise an error. Both engines
are held to the same golden baseline at `1e-8` tolerance.

## Factor Libraries

Two built-in alpha libraries ship as expression builders:

- **WorldQuant 101** (`alpha1`–`alpha101`) — see
  [docs/worldquant_alpha101.md](docs/worldquant_alpha101.md) for supported input
  fields, coverage tiers, and implementation defaults.
- **Qlib Alpha158** (9 kbar + 4 price + 29 rolling groups × 5 windows) — see
  [docs/qlib_alpha158.md](docs/qlib_alpha158.md) for the factor list and caliber
  notes.

This project is not affiliated with WorldQuant, Microsoft, or Qlib.

## Development Checks

Run the same checks expected by CI:

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
uv run maturin develop
uv run pytest
```

See [docs/development.md](docs/development.md) for more detail.

For alpha expression construction and execution details, see
[docs/expression_api.md](docs/expression_api.md).

## License

MIT. See [LICENSE](LICENSE).
