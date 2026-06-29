# Architecture

qfactors is organized as a Rust workspace with a Python extension module.

## Crates

- `qfactors-core`: panel layout, column validation, factor execution, alpha
  expression evaluation, catalogs, and result sinks.
- `qfactors-factors`: built-in factor kernels and WorldQuant 101 alpha
  registrations.
- `qfactors-macros`: procedural macros used to register factor kernels and
  generate catalog metadata.
- `qfactors-py`: PyO3 extension module exposing the Rust engine to Python as
  `qfactors`.

## Data Flow

1. Python or Rust callers provide a Polars DataFrame, symbol/time column names,
   requested factor or alpha names, and observation times.
2. `qfactors-core` validates structural columns, resolves logical input fields
   through optional aliases, sorts the panel, and builds the internal cell set.
3. Factor kernels or alpha expressions compute full panel values.
4. Results are sampled at the requested observation times and returned in memory
   or written to Parquet through the sink layer.

## Alpha Evaluation

The default alpha evaluator walks expression trees independently. The optional
`QF_ENGINE=dag` evaluator lowers requested alphas into a shared DAG for local
benchmarking and common-subexpression reuse. The DAG engine is experimental and
not the default.
