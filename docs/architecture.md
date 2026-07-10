# Architecture

qweave is organized as a Rust workspace with a Python extension module.

## Crates

- `qweave-core`: panel layout, column validation, alpha expression evaluation,
  and result sinks.
- `qweave-factors`: built-in alpha builders — WorldQuant 101 and Qlib
  Alpha158.
- `qweave-py`: PyO3 extension module exposing the Rust engine to Python as
  `qweave`.

## Data Flow

1. Python or Rust callers provide a Polars DataFrame, symbol/time column names,
   and a list of aliased alpha expressions.
2. `qweave-core` validates structural columns, sorts the panel by
   `(symbol, time)`, and builds the internal cell set.
3. The evaluator computes each expression over the full panel.
4. Results are returned in memory as a `(time, symbol)` frame, appended to the
   input in original row order (`with_alphas`), or written to Parquet through the
   sink layer.

## Alpha Evaluation

The default alpha evaluator walks expression trees independently. The optional
`QWEAVE_ENGINE=dag` evaluator lowers requested alphas into a shared DAG for local
benchmarking and common-subexpression reuse. The DAG engine is experimental and
not the default.
