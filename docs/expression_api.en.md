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

- **Fixed-window aggregations** run per symbol over the most recent `d` bars. The
  output is NaN while the window is incomplete or contains any NaN, so the
  first `d - 1` rows of each symbol are NaN. `ts_count` is the missing-value
  exception: it counts present samples once all `d` rows are available.
- **Cross-sectional operators** run over the full cross-section of each
  timestamp; NaN samples do not participate and stay NaN.
- **Comparisons** output 1.0 / 0.0, and NaN when either operand is NaN.

### Elementwise

| Operator | Meaning |
| --- | --- |
| `+` `-` `*` `/`, unary `-` | arithmetic |
| `<` `>` `<=` `>=` `==` `!=` | comparison: 1.0 if true, else 0.0 |
| `&` / `\|` / `~` | three-valued logical AND / OR / NOT on Float64 masks |
| `abs()` | absolute value |
| `log()` | natural logarithm |
| `sign()` | sign function (-1 / 0 / 1) |
| `min(x, y)` / `max(x, y)` | elementwise min / max |
| `power(x, y)` | `x^y` |
| `signed_power(x, y)` | `sign(x) * abs(x)^y` |
| `where_(cond, a, b)` | `a` where `cond` holds, else `b` |

Comparisons and logical operators return Float64 masks (`1.0`, `0.0`, or NaN),
which can still be multiplied by values or passed to `ts_sum`. Logical inputs
use the same positive-is-true rule as `where_`: positive values (including
positive infinity) are true; zero, negative values, and negative infinity are
false; NaN/null is unknown. These are logical operations, not integer bitwise
operations. Both operands are evaluated; there is no short-circuit evaluation.

| `a` | `b` | `a & b` | `a \| b` |
| --- | --- | --- | --- |
| false | false | 0 | 0 |
| false | true | 0 | 1 |
| false | unknown | 0 | NaN |
| true | true | 1 | 1 |
| true | unknown | NaN | 1 |
| unknown | unknown | NaN | NaN |

AND/OR are symmetric. NOT maps true to 0, false to 1, and unknown to NaN.
Wrap constants in `qw.lit(...)` and parenthesize comparisons:

```python
mask = (qw.col("close") > qw.col("open")) & (qw.col("volume") != qw.lit(0.0))
up_days = mask.ts_sum(20).alias("up_days")
```

**Behavior tightening:** implicit Python truth conversion now raises `TypeError`.
Use `&`, `|`, and `~` instead of Python `and`, `or`, and `not`; `bool(expr)`,
`if expr`, and chained comparisons such as `x < y < z` are also rejected.
Write `(x < y) & (y < z)` instead.

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
| `ts_mad(d)` | mean absolute deviation about the same window's mean; full finite window required |
| `ts_count(d)` | count of non-NaN/null samples after `d` rows; zero and infinities count |
| `ts_std(d)` | sample standard deviation (`ddof = 1`) |
| `ts_skew(d)` | bias-corrected sample skewness, at least 3 samples; full constant windows yield 0 |
| `ts_kurt(d)` | bias-corrected Fisher excess kurtosis, at least 4 samples; full constant windows yield -3 |
| `slope(d)` / `rsquare(d)` / `resi(d)` | OLS of window values against the time index: slope / R² / last-point residual |
| `quantile(d, q)` | window quantile, `q ∈ [0, 1]`, linear interpolation |
| `wma(d)` / `decay_linear(d)` | identical linearly weighted means with weights `1..d`, newer bars weighted more |
| `correlation(x, y, d)` | window Pearson correlation; NaN when either side has zero variance |
| `covariance(x, y, d)` | window sample covariance (`ddof = 1`) |

Fixed-window interfaces consistently omit `min_periods`. `ts_skew` and `ts_kurt`
require a full finite window: NaN, null, or infinity makes the window result NaN,
as does a period below the statistical minimum. `wma` reuses `decay_linear`'s
full-window and missing-value behavior; both share the same DAG node.
Zero periods yield NaN; Python bindings reject negative or noninteger periods.

`ts_mad(d)` computes `mean(abs(x - mean(x)))` within one window, not median
absolute deviation or a composition of two rolling means. NaN, null, or infinity
makes the result NaN; constant windows and finite `d=1` windows yield 0. It uses
centering and scaling to limit intermediate magnitudes, takes `O(T*d)` time,
and allocates no temporary array per window.

`ts_count(d)` requires `d` rows, but permits missing values: all-missing full
windows yield 0. For `d=1`, missing samples yield 0 and all other samples yield 1.
The first `d-1` rows still yield NaN. It updates the count on entry/exit, taking
`O(T)` time and `O(1)` rolling state per symbol. Both operators support tree and
DAG execution; `d=0` yields NaN, and negative/noninteger periods are rejected.

### Exponential Smoothing

`ema(days)` computes a technical-indicator EMA per symbol. The mean of the first
`days` consecutive finite samples seeds the state, followed by the recurrence
`y = (1 - alpha) * y_prev + alpha * x`, where `alpha = 2 / (days + 1)`.
Outputs are NaN before initialization. NaN, null, or infinity resets the state
and requires another `days` consecutive finite samples. `days=1` preserves
finite inputs; `days=0` yields NaN throughout. This uses accumulated historical
state, not an exponential average recomputed over the last `days` bars.

```python
alphas = [
    qw.col("close").ts_skew(20).alias("skew20"),
    qw.col("close").ts_kurt(20).alias("kurt20"),
    qw.col("close").ema(20).alias("ema20"),
    qw.col("close").wma(20).alias("wma20"),
]
```

All four work in the tree and DAG engines. Skew/Kurt update rolling central
moments incrementally, rebuilding every `d` rows to limit accumulated error,
for `O(T)` total time under ordinary conditions. Large changes in scale or
shrinking variation trigger additional window rebuilds, so extreme inputs can
still require `O(T*d)` in the worst case. EMA and WMA run in `O(T)`, where `T`
is the number of rows per symbol.

**Rust WMA migration:** the former `alpha::wma`, using Guotai Junan's `0.9^age`
weights, is now `alpha::gtja_wma` with node `Expr::GtjaWma`. `alpha::wma` now
means standard linear WMA. Alpha191 calls have migrated without changing factor
values. The Guotai Junan variant is not exposed in Python. Identically named
Qlib operators are not always interchangeable; see the
[operator migration reference](qlib_operators.en.md).

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

Use `compute_alphas` when you want a tidy full-history panel sorted by `(symbol, time)`:

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
All three builders print the canonical-input-to-DataFrame mapping used by the
requested factors; unmapped fields appear as identity mappings. When a panel
contains both `close` and `close_adj`, this makes it possible to confirm that
`{"close": "close_adj"}` was supplied before computing factors.

## Next Steps

Once factors are computed, use [Factor Evaluation](factor_evaluation.en.md):
`with_labels` builds leakage-safe forward-return labels, `evaluate` produces
IC/quantile/turnover diagnostics, and `result.view()` opens the interactive
report.
