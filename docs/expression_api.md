# Python Expression API

qweave 0.3 introduces an eager expression API for alpha research. Expressions
are plain Python objects backed by the Rust `Expr` tree; executors evaluate a
list of expressions immediately.

## Construct Expressions

```python
import qweave as qf

intraday_return = (
    (qf.col("close") - qf.col("open"))
    / (qf.col("high") - qf.col("low") + qf.lit(0.001))
).alias("intraday_return")
```

Expressions must be aliased before they are passed to `compute_alphas` or
`with_alphas`; the alias becomes the output column name.

Common operations include:

- arithmetic: `+`, `-`, `*`, `/`, unary `-`
- comparisons: `<`, `>`, `<=`, `>=`, `==`
- unary transforms: `abs`, `log`, `sign`, `rank`, `scale`
- time-series windows: `delay`, `delta`, `ts_sum`, `ts_mean`, `product`,
  `ts_min`, `ts_max`, `ts_argmin`, `ts_argmax`, `ts_rank`, `ts_rank_raw`,
  `ts_std`, `slope`, `rsquare`, `resi`, `quantile`, `decay_linear`
- binary functions: `min`, `max`, `power`, `signed_power`, `correlation`,
  `covariance`, `group_rank`, `group_neutralize`, `where_`

## Execute Expressions

Use `with_alphas` when you want to preserve the input DataFrame and append
factor columns in original row order:

```python
out = qf.with_alphas(df, "asset", "time", [intraday_return])
```

Use `compute_alphas` when you want a tidy full-history `(time, symbol)` panel:

```python
out = qf.compute_alphas(df, "asset", "time", [intraday_return])
```

`compute_alphas(..., output_path="alphas.parquet")` writes the full frame to
Parquet and returns a summary dict. The current 0.3 implementation still
materializes the full frame before writing; streaming/batched output is reserved
for a later release.

`with_alphas` preserves original row order by allocating one full-size output
buffer per expression and scattering evaluated `(time, symbol)` values back into
input order before appending the columns. For large batches, prefer
`compute_alphas` when you do not need to keep the original DataFrame shape.

## Reuse Templates

`collect_inputs()` reports canonical input fields, and `replace_inputs()` maps
those fields to physical DataFrame columns while preserving the expression
alias:

```python
expr = ((qf.col("close") + qf.col("open")) / qf.lit(2.0)).alias("mid")
assert expr.collect_inputs() == {"close", "open"}

adjusted = expr.replace_inputs({"close": "adj_close", "open": "adj_open"})
```

Field remapping is part of the expression tree, so there is a single visible
aliasing path (`replace_inputs()` or a library `input_alias`) rather than an
executor-level alias argument.

## Built-in Factor Libraries

```python
alphas = qf.worldquant_alpha101(
    {"close": "adj_close", "open": "adj_open"},
    alphas=["alpha13", "alpha101"],
)
out = qf.compute_alphas(df, "asset", "time", alphas)
```

`qf.qlib_alpha158(input_alias, alphas=None)` exposes the Qlib Alpha158 set with
the same signature. Pass an empty dict for identity input mapping. See
[worldquant_alpha101.md](worldquant_alpha101.md) and
[qlib_alpha158.md](qlib_alpha158.md) for implementation defaults and required
input fields.
