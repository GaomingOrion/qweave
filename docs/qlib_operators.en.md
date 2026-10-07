# Qlib Operator Migration Reference

[Chinese](qlib_operators.md)

This table covers every operator registered in
[Qlib v0.9.7's OpsList](https://github.com/microsoft/qlib/blob/v0.9.7/qlib/data/ops.py).
qweave maps expression capabilities; it does not parse Qlib expression strings
or promise full-platform or full-series numerical compatibility. Here `x` and
`y` are `qw.col(...)` expressions; wrap numeric constants in `qw.lit(...)`.
qweave numeric inputs are Float64 columns, with null treated as NaN.

## Elementwise Operators

| Qlib | qweave | Migration notes |
| --- | --- | --- |
| Abs, Log | `x.abs()`, `x.log()` | Nonpositive Log inputs differ: qweave yields NaN; Qlib yields negative infinity for zero |
| Sign | `x.sign()` | Qlib casts to float32 first; extremely small values can differ |
| Power | `qw.power(x, y)` | qweave yields NaN for negative bases with noninteger exponents |
| Add, Sub, Mul, Div | `x+y`, `x-y`, `x*y`, `x/y` | Floating-point arithmetic |
| Greater, Less | `qw.max(x, y)`, `qw.min(x, y)` | Elementwise extrema, not comparisons |
| Gt, Ge, Lt, Le, Eq | `x>y`, `x>=y`, `x<y`, `x<=y`, `x==y` | qweave returns 1/0, or NaN if either operand is NaN; Qlib returns booleans |
| If | `qw.where_(condition, x, y)` | qweave yields NaN for a NaN condition; NumPy truth conversion is not equivalent |
| Ne | `x != y` | Float64 1/0; either input NaN yields NaN, unlike Qlib's boolean inequality |
| And, Or, Not | `x & y`, `x \| y`, `~x` | Float64 three-valued logic: positive = true, nonpositive = false, NaN = unknown; Qlib uses bitwise operations |

Logical masks remain Float64 and can participate in arithmetic. False AND
unknown is 0; true OR unknown is 1; otherwise indeterminate results are NaN.
NOT unknown is NaN. Operand order does not matter. These operators do not
reproduce Qlib's integer bitwise semantics. Use parentheses around comparisons,
e.g. `(x > y) & (x != qw.lit(0.0))`. Implicit Python truth testing now raises
`TypeError`: use `&`, `|`, `~` instead of `and`, `or`, `not`.

## Time-Series Operators

qweave fixed windows require the latest `days` bars and do not expose
`min_periods`. Missing values normally invalidate a window; `ts_count` instead
counts nonmissing samples once all `days` rows are available. Most Qlib windows
use `min_periods=1`, and some support expanding windows with `N=0`. Warmup and missing-value results therefore usually differ.
Callers must also align trading calendars, absent rows, and history start points;
qweave does not fill these automatically.

| Qlib | qweave | Migration notes |
| --- | --- | --- |
| Ref | `x.delay(days)` | Positive integer lags correspond; Qlib's 0 selects the first value and negative N selects future values; qweave's 0 returns the input |
| Delta | `x.delta(days)` | Positive integers subtract the lagged value; Qlib's 0 subtracts the first value, qweave's 0 subtracts the input from itself |
| Mean, Sum, Std | `x.ts_mean(days)`, `x.ts_sum(days)`, `x.ts_std(days)` | Std uses sample variance; verify window and non-finite-value behavior separately |
| Var | `s * s`, where `s=x.ts_std(days)` | Compose sample variance; no separate alias |
| Max, Min | `x.ts_max(days)`, `x.ts_min(days)` | Full-window rules apply |
| IdxMax, IdxMin | `x.ts_argmax(days)`, `x.ts_argmin(days)` | qweave is 0-based, earliest wins ties; Qlib is 1-based, so add `qw.lit(1.0)` |
| Rank | `x.ts_rank(days)` | Time-series percentile rank, not cross-sectional `x.rank()` |
| Quantile | `x.quantile(days, q)` | Linear interpolation with full-window rules |
| Med | `x.quantile(days, 0.5)` | Compose the median |
| Slope, Rsquare, Resi | `x.slope(days)`, `x.rsquare(days)`, `x.resi(days)` | Regression against the time index; warmup, zero-variance, and near-zero thresholds differ |
| Corr, Cov | `qw.correlation(x, y, days)`, `qw.covariance(x, y, days)` | Pearson correlation and sample covariance; missing pairs, warmup, and near-zero variance handling differ |
| Skew, Kurt | `x.ts_skew(days)`, `x.ts_kurt(days)` | Same bias-corrected formulas; qweave requires full finite windows and at least 3/4 samples respectively |
| EMA | `x.ema(days)` | qweave uses mean-seeded recursive EMA; Qlib uses adjusted EWM, with different initialization and missing-value behavior |
| WMA | `x.wma(days)`, equivalent to `x.decay_linear(days)` | Standard linearly weighted mean; see below for Qlib's extra normalization |
| Mad | `x.ts_mad(days)` | Mean absolute deviation about the same window's mean; qweave requires full finite windows, while Qlib skips NaN and uses partial windows |
| Count | `x.ts_count(days)` | Non-NaN/null count (including zero and infinities); full all-missing windows yield 0; unlike Qlib, first `days-1` rows yield NaN |
| Rolling | No general string dispatch | Choose the specific statistic; Qlib's expanding/special fractional-window branches are unsupported |

Skew and Kurt follow pandas' bias-corrected
[skewness](https://pandas.pydata.org/pandas-docs/version/2.2/reference/api/pandas.core.window.rolling.Rolling.skew.html)
and [excess kurtosis](https://pandas.pydata.org/pandas-docs/version/2.2/reference/api/pandas.core.window.rolling.Rolling.kurt.html)
formulas. Full constant windows yield 0 and -3 respectively; insufficient sample
counts yield NaN.

Standard WMA uses oldest-to-newest weights `1..days`, divided by their sum; see
[TA-Lib WMA](https://ta-lib.org/functions/wma.html). Qlib v0.9.7 applies `nanmean`
to already normalized weighted values: `[1,2,3]` yields `7/9`, versus standard
WMA's `7/3`. A full three-bar window of ones yields `1/3` versus `1`.
For a positive integer window length `N`, on full windows containing only finite
values (no NaN, null, or infinity), Qlib's WMA equals standard WMA divided by `N`.
For example, to match Qlib's 20-period WMA:

```python
qlib_wma20 = x.wma(20) / qw.lit(20.0)
```

This composition still follows qweave's full-window and missing-value rules;
it does not reproduce Qlib's warmup or missing-value handling.

EMA's mean initialization follows [TA-Lib EMA](https://ta-lib.org/functions/ema.html).
qweave additionally specifies state reset and renewed warmup after non-finite
values. Qlib's `N=0` and fractional N are unsupported. See the
[expression API](expression_api.en.md) for parameter and missing-value rules.

## Data and Framework Operators

| Qlib | qweave approach |
| --- | --- |
| Feature | Use `qw.col(name)` for an existing panel column; no Qlib provider loading |
| PFeature | No PIT financial-data provider semantics; callers prepare columns with correct availability timestamps |
| ChangeInstrument, Mask | No instrument switching inside expressions; join reference-instrument series to the panel by time first |
| TResample | No expression-level time-axis changes; resample with Polars beforehand |
