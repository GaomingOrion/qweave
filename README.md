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
- **WorldQuant 101 built in:** `worldquant101_alphas()` returns expression
  objects for `alpha1` through `alpha101`, with documented project defaults and
  input aliasing for adjusted or vendor-specific column names.
- **Regression guarded:** every registered alpha is checked against a frozen
  synthetic golden fixture at `1e-8` tolerance, so engine changes are reviewed
  against stable numerical output.
- **Extensible Rust kernels:** procedural macros register custom factor kernels
  with windows, parameters, and multi-output support.

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
- WorldQuant 101 alphas (`alpha1`–`alpha101`).
- v0.3.0 Python expression API: `PyExpr`, `with_alphas`, full-history
  `compute_alphas`, input replacement templates, and type stubs.

**Experimental**

- DAG evaluator (`QF_ENGINE=dag`) with hash-consed common
  subexpression elimination and slot-reuse. It is gated behind a flag and
  benchmarked against the default tree engine; an optimization is promoted only
  when it demonstrably beats the current default.

**Planned**

- Node-level parallelism and fewer layout transposes in the evaluator.
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

alphas = qfactors.worldquant101_alphas({}, alphas=["alpha101"])
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

`compute_panel` computes registered factor kernels at requested observation
times. `compute_alphas` computes expression alphas over the full panel and
returns a Polars DataFrame by default, or a summary dict when `output_path` is
provided. `with_alphas` appends expression outputs to the input DataFrame in its
original row order.

## Public API

Python functions:

- `qfactors.compute_panel(df, symbol_col, time_col, factors, observation_times, column_aliases=None, output_path=None)`
- `qfactors.compute_alphas(df, symbol_col, time_col, alphas, output_path=None)`
- `qfactors.with_alphas(df, symbol_col, time_col, alphas)`
- `qfactors.col(name)`, `qfactors.lit(value)`, and expression operators
- `qfactors.worldquant101_alphas(input_alias, alphas=None)`
- `qfactors.factor_catalog()`

Input rules:

- `symbol_col`, `time_col`, and `compute_panel` observation times cannot contain
  nulls.
- Structural NaN values are rejected.
- Float input nulls are converted to NaN so factor logic can propagate missing
  data.
- The engine sorts panel rows by `(symbol_col, time_col)` and rejects duplicate
  symbol-time pairs.
- For `compute_panel`, `column_aliases` maps logical names such as `close` to
  physical input columns such as `adj_close`. Alpha expressions use
  `replace_inputs()` or `worldquant101_alphas(input_alias=...)` instead; the
  alpha executors do not accept `column_aliases`.

Memory note:

- `with_alphas` preserves original input row order by scattering each evaluated
  alpha column into a new full-size output buffer before appending it. For very
  wide alpha batches, `compute_alphas` is the more memory-lean executor because
  it can move evaluated columns directly into the result frame.

## Alpha Engine

`compute_alphas` uses the tree evaluator by default. An experimental DAG
evaluator can be selected for local benchmarking:

```bash
QF_ENGINE=dag uv run pytest
```

Valid values are `tree` and `dag`; invalid values raise an error. The tree
engine remains the default until the DAG path is fully benchmarked and promoted.

## WorldQuant 101

The built-in alpha library includes `alpha1` through `alpha101`. See
[docs/worldquant101.md](docs/worldquant101.md) for supported input fields,
coverage tiers, and implementation defaults.

This project is not affiliated with WorldQuant.

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
