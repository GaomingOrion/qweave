# Python Expression API

[Chinese](expression_api.md)

qweave provides an eager expression API for alpha research. You write normal
Python expressions backed by a Rust `Expr` tree; execution can submit a batch to
the DAG evaluator instead of looping one column at a time in Python.

## Construct Expressions

```python
import qweave as qw

intraday_return = (
    (qw.col("close") - qw.col("open"))
    / (qw.col("high") - qw.col("low") + qw.lit(0.001))
).alias("intraday_return")
```

Expressions must be aliased before they are passed to `compute_alphas` or
`with_alphas`; the alias becomes the output column name.

## Operator Quick Reference

Shared calibers:

- **Time-series operators** run per symbol over the most recent `d` bars. The
  output is NaN while the window is incomplete or contains any NaN, so the
  first `d - 1` rows of each symbol are NaN.
- **Cross-sectional operators** run over the full cross-section of each
  timestamp; NaN samples do not participate and stay NaN.
- **Comparisons** output 1.0 / 0.0, and NaN when either operand is NaN.

### Elementwise

| Operator | Meaning |
| --- | --- |
| `+` `-` `*` `/`, unary `-` | arithmetic |
| `<` `>` `<=` `>=` `==` | comparison: 1.0 if true, else 0.0 |
| `abs()` | absolute value |
| `log()` | natural logarithm |
| `sign()` | sign function (-1 / 0 / 1) |
| `min(x, y)` / `max(x, y)` | elementwise min / max |
| `power(x, y)` | `x^y` |
| `signed_power(x, y)` | `sign(x) * abs(x)^y` |
| `where_(cond, a, b)` | `a` where `cond` holds, else `b` |

### Time-Series Windows (per symbol, window `d`)

| Operator | Meaning |
| --- | --- |
| `delay(d)` | value `d` bars ago |
| `delta(d)` | `x - delay(x, d)` |
| `ts_sum(d)` / `ts_mean(d)` / `product(d)` | window sum / mean / product |
| `ts_min(d)` / `ts_max(d)` | window min / max |
| `ts_argmin(d)` / `ts_argmax(d)` | 0-based position of the extremum (0 = oldest, `d-1` = current; earliest wins ties) |
| `ts_rank(d)` | percentile rank of the current value within the window, in `(0, 1]`, ties averaged (pandas `rank(pct=True)` caliber) |
| `ts_rank_raw(d)` | 0-based ascending position of the current value, minimum on ties (DolphinDB `mrank` caliber) |
| `ts_std(d)` | sample standard deviation (`ddof = 1`) |
| `slope(d)` / `rsquare(d)` / `resi(d)` | OLS of window values against the time index: slope / R² / last-point residual |
| `quantile(d, q)` | window quantile, `q ∈ [0, 1]`, linear interpolation |
| `decay_linear(d)` | linearly weighted mean with weights `1..d`, newer bars weighted more |
| `correlation(x, y, d)` | window Pearson correlation; NaN when either side has zero variance |
| `covariance(x, y, d)` | window sample covariance (`ddof = 1`) |

### Cross-Sectional (per timestamp)

| Operator | Meaning |
| --- | --- |
| `rank()` | cross-sectional percentile rank, in `(0, 1]`, ties averaged |
| `scale(scale_to=1.0)` | rescale so the day's `sum(abs(x)) = scale_to`; all-zero cross-sections yield NaN |
| `group_rank(x, g)` | percentile rank within each (date, group); `g` must be a non-null String/integer column, with integers in the `i32` range |
| `group_neutralize(x, g)` | subtract the (date, group) mean; `g` has the same type constraint |

## Execute Expressions

Use `with_alphas` when you want to preserve the input DataFrame and append
factor columns in original row order:

```python
out = qw.with_alphas(df, "asset", "time", [intraday_return])
```

Use `compute_alphas` when you want a tidy full-history `(time, symbol)` panel:

```python
out = qw.compute_alphas(df, "asset", "time", [intraday_return])
```

`compute_alphas(..., output_path="alphas.parquet")` writes the full result and
returns a summary. `with_alphas` allocates one full-size output buffer per
expression and scatters values back into input row order; for large factor
batches where the original shape is not needed, prefer `compute_alphas`.

As a rule of thumb:

- Use `with_alphas` for notebook exploration or when you want to preserve the
  original columns.
- Use `compute_alphas` for batch factor output, Parquet export, or downstream
  evaluation.

## Reuse Templates

`collect_inputs()` reports canonical input fields, and `replace_inputs()` maps
those fields to physical DataFrame columns while preserving the expression
alias:

```python
expr = ((qw.col("close") + qw.col("open")) / qw.lit(2.0)).alias("mid")
assert expr.collect_inputs() == {"close", "open"}

adjusted = expr.replace_inputs({"close": "adj_close", "open": "adj_open"})
```

Field remapping is part of the expression tree, so there is a single visible
aliasing path (`replace_inputs()` or a library `input_alias`) rather than an
executor-level alias argument.

## Built-in Factor Libraries

```python
alphas = qw.worldquant_alpha101(
    {"close": "adj_close", "open": "adj_open"},
    alphas=["alpha13", "alpha101"],
)
out = qw.compute_alphas(df, "asset", "time", alphas)
```

`qw.qlib_alpha158(input_alias, alphas=None)` exposes the Qlib Alpha158 set with
the same signature. `qw.gtja_alpha191(input_alias, alphas=None)` exposes Guotai
Junan Alpha191 with padded names from `gtja_alpha001` through `gtja_alpha191`.
Pass an empty dict for identity input mapping. See
[WorldQuant 101](worldquant_alpha101.en.md) and
[Qlib Alpha158](qlib_alpha158.en.md), plus
[Guotai Junan Alpha191](gtja_alpha191.en.md), for implementation defaults and
required input fields.

## Next Steps

Once factors are computed, use [Factor Evaluation](factor_evaluation.en.md):
`with_labels` builds leakage-safe forward-return labels, `evaluate` produces
IC/quantile/turnover diagnostics, and `result.view()` opens the interactive
report.
