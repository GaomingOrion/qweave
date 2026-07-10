# qweave

[Chinese](README.md)

qweave is a **Rust + Polars factor workflow toolkit** for quantitative research.
It puts alpha expressions, batch factor computation, forward-return labels,
IC/RankIC evaluation, quantile returns, and interactive reports into one Python
DataFrame pipeline so researchers can move from an idea to comparable results
with less glue code.

The project currently focuses on factor processing and factor evaluation, with
quantitative modeling, strategy construction, and backtesting planned next. The
API is pre-1.0, but it is already useful for local research and internal
workflows.

## Why It Is Interesting

- **One DataFrame pipeline:** pass in a Polars DataFrame, append alphas, labels,
  and evaluation results without bouncing between factor matrices, label
  matrices, and analysis tables.
- **Rust hot path:** panel sorting, validation, rolling windows,
  cross-sectional operators, expression DAG evaluation, and evaluation
  statistics run on the Rust side. Python mostly orchestrates.
- **Batch expression execution:** the default DAG evaluator reuses common
  subexpressions, reuses intermediate slots, and fuses elementwise chains, which
  is useful when computing hundreds of overlapping alphas at once.
- **Reusable built-in factor libraries:** `worldquant_alpha101()` and
  `qlib_alpha158()` return normal expression objects. You can select subsets,
  remap inputs, and mix them with custom expressions.
- **Explicit research calibers:** forward returns, tradable samples, quantile
  binning, demeaning, turnover, and long-short diagnostics have documented
  defaults.
- **Reproducible performance story:** the repository includes synthetic-panel
  benchmarks for qweave, Qlib Alpha158, and KunQuant Alpha101 paths. Historical
  macOS numbers were removed; current claims should be re-measured in the
  Windows/PowerShell environment.

## Relationship To Qlib And KunQuant

qweave is not trying to clone all of Qlib, and it is not a drop-in replacement
for KunQuant's JIT compiler. It is a lightweight, fast, Polars-native factor
research kernel that can be used directly or embedded into a larger platform.

| Project | Strength | qweave focus |
| --- | --- | --- |
| Qlib | Full AI quant platform covering data, models, portfolios, backtesting, and execution workflows | Lighter factor-computation and evaluation kernel that plugs directly into existing Polars DataFrames |
| KunQuant | Compiles expression batches into optimized C++/JIT execution paths | Avoids user-managed C++/JIT lifecycle and emphasizes Python ergonomics, Rust kernels, and the evaluation loop |
| pandas/Alphalens-style tools | Interactive analysis and traditional DataFrame workflows | Keeps factor processing, labels, and evaluation in one Rust/Polars pipeline, reducing Python loops on large panels |

See [Comparison](docs/comparison.en.md) for positioning and
[Benchmarks](docs/benchmark.en.md) for reproducible measurements.

## Quick Start

```python
import polars as pl
import qweave as qf

df = pl.DataFrame(
    {
        "asset": ["A", "A", "B", "B"],
        "time": [1, 2, 1, 2],
        "open": [10.0, 11.0, 20.0, 19.0],
        "close": [11.0, 12.0, 19.0, 21.0],
        "high": [12.0, 13.0, 21.0, 22.0],
        "low": [9.0, 10.0, 18.0, 18.5],
        "volume": [100.0, 120.0, 80.0, 90.0],
        "tradable": [True, True, True, True],
    }
)

alphas = [
    (
        (qf.col("close") - qf.col("open"))
        / (qf.col("high") - qf.col("low") + qf.lit(0.001))
    ).alias("intraday_return")
]

df = qf.with_alphas(df, "asset", "time", alphas)
df = qf.with_labels(
    df,
    symbol_col="asset",
    time_col="time",
    horizons=[1],
    entry_lag=0,
    entry_col="close",
    exit_col="close",
    tradable_col="tradable",
)

result = qf.evaluate(
    df,
    symbol_col="asset",
    time_col="time",
    factor_cols=["intraday_return"],
    quantiles=2,
    min_cs_count=2,
    tradable_col="tradable_entry",
)

print(result.summary)
```

Compute built-in factors in bulk:

```python
alphas = qf.worldquant_alpha101({}, alphas=["alpha13", "alpha101"])
out = qf.compute_alphas(df, "asset", "time", alphas)
```

`with_alphas` appends factor columns in original input row order.
`compute_alphas` emits a full `(time, symbol)` panel and can write Parquet.

## Installation

This repository currently targets source builds and is not published to PyPI or
crates.io yet.

Prerequisites:

- Python 3.10 or newer
- `uv`
- Rust nightly with `rustfmt` and `clippy`

```powershell
uv sync --dev
uv run maturin develop
```

The repository includes `rust-toolchain.toml`, so Cargo uses the pinned nightly
toolchain automatically.

## Capability Map

**Available today**

- WorldQuant 101 and Qlib Alpha158 expression libraries.
- Python expression API: `col`, `lit`, arithmetic/comparison operators, rolling
  windows, ranks, neutralization, and `replace_inputs()`.
- `compute_alphas` and `with_alphas` for batch alpha output and input-frame
  augmentation.
- DAG alpha evaluator with common-subexpression reuse, slot reuse, node-level
  parallelism, and fused elementwise chains.
- `with_labels`, `evaluate`, `factor_correlation`, HTML reports, and
  interactive reports.

**Planned**

- Quantitative modeling, strategy construction, and backtesting modules.
- More complete API references and example datasets.
- Publication to PyPI and crates.io.

## Public API

- `qweave.compute_alphas(df, symbol_col, time_col, alphas, output_path=None)`
- `qweave.with_alphas(df, symbol_col, time_col, alphas)`
- `qweave.col(name)`, `qweave.lit(value)`, and expression operators
- `qweave.worldquant_alpha101(input_alias, alphas=None)`
- `qweave.qlib_alpha158(input_alias, alphas=None)`
- `qweave.with_labels(...)`, `qweave.evaluate(...)`
- `qweave.factor_correlation(...)`, `EvalResult.to_html(...)`,
  `EvalResult.view()`

Input rules:

- `symbol_col` and `time_col` cannot contain nulls.
- Structural columns cannot contain NaN.
- Nulls in floating input columns are converted to NaN so factor logic can
  propagate missing values naturally.
- The engine sorts by `(symbol_col, time_col)` and rejects duplicate
  symbol-time rows.
- Field remapping lives in the expression tree through `PyExpr.replace_inputs()`
  or the built-in libraries' `input_alias` argument.

## Documentation

For GitHub readers:

- [Comparison](docs/comparison.en.md)
- [Performance and Benchmarks](docs/benchmark.en.md)
- [Architecture and Design Tradeoffs](docs/architecture.en.md)
- [Python Expression API](docs/expression_api.en.md)
- [Factor Evaluation](docs/factor_evaluation.en.md)
- [WorldQuant 101](docs/worldquant_alpha101.en.md)
- [Qlib Alpha158](docs/qlib_alpha158.en.md)

For maintainers:

- [Development Guide](docs/development.en.md)

This project is not affiliated with WorldQuant, Microsoft, Qlib, or KunQuant.

## Development Checks

```powershell
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
uv run maturin develop
uv run python -m pytest
```

See the [Development Guide](docs/development.en.md) for details.

## License

MIT. See [LICENSE](LICENSE).
