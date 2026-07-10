# qlib Alpha158

qweave builds the Microsoft Qlib `Alpha158` feature set as 158 built-in alpha
expressions. The formulas follow Qlib's `Alpha158` handler; project-specific
calibers are documented below.

## Structure

The 158 factors break down as:

- **9 kbar** candlestick-shape factors: `KMID`, `KLEN`, `KMID2`, `KUP`, `KUP2`,
  `KLOW`, `KLOW2`, `KSFT`, `KSFT2`.
- **4 price** factors normalized by close: `OPEN0`, `HIGH0`, `LOW0`, `VWAP0`.
- **29 rolling groups × 5 windows** (`{5, 10, 20, 30, 60}`), named `<GROUP><d>`
  (e.g. `MA5`, `CORR60`):

  | Group | Meaning |
  | --- | --- |
  | `ROC` | rate of change, `delay(close, d) / close` |
  | `MA` | `ts_mean(close, d) / close` |
  | `STD` | `ts_std(close, d) / close` |
  | `BETA` | linear-trend slope over the window, `/ close` |
  | `RSQR` | R² of the same linear fit |
  | `RESI` | residual of the last point from the fit, `/ close` |
  | `MAX` | `ts_max(high, d) / close` |
  | `MIN` | `ts_min(low, d) / close` |
  | `QTLU` | 0.8 quantile of close over the window, `/ close` |
  | `QTLD` | 0.2 quantile of close over the window, `/ close` |
  | `RANK` | time-series percentile rank of close |
  | `RSV` | `(close - ts_min(low, d)) / (ts_max(high, d) - ts_min(low, d))` |
  | `IMAX` | window position of the high's max (see calibers) |
  | `IMIN` | window position of the low's min |
  | `IMXD` | `IMAX` − `IMIN` position difference |
  | `CORR` | correlation of close with `log(volume + 1)` |
  | `CORD` | correlation of close return with log volume ratio |
  | `CNTP` | fraction of up days (`close > delay(close, 1)`) |
  | `CNTN` | fraction of down days |
  | `CNTD` | `CNTP − CNTN` |
  | `SUMP` | up-move sum / total abs-move sum (close) |
  | `SUMN` | down-move sum / total abs-move sum (close) |
  | `SUMD` | `(up − down)` sum / total abs-move sum (close) |
  | `VMA` | `ts_mean(volume, d) / volume` |
  | `VSTD` | `ts_std(volume, d) / volume` |
  | `WVMA` | std / mean of `|return| * volume` over the window |
  | `VSUMP` / `VSUMN` / `VSUMD` | `SUMP` / `SUMN` / `SUMD` on volume |

All factors are per-symbol time-series or element-wise; none use a cross-section.

## Public Surface

```python
alphas = qweave.qlib_alpha158(
    {"close": "adj_close"},
    alphas=["KMID", "MA5", "CORR20"],
)
qweave.compute_alphas(df, "asset", "time", alphas)
```

`qlib_alpha158(input_alias, alphas=None)` returns `PyExpr` objects for the
built-in set (all 158 when `alphas` is omitted, or the named subset in request
order). `input_alias` maps canonical input names such as `close` to physical
DataFrame columns; pass an empty dict for identity mapping. `compute_alphas()`
evaluates the expressions over the full `(time, symbol)` panel, while
`with_alphas()` appends them in original row order. See
[expression_api.md](expression_api.md) for custom expression construction.

## Input Fields

Every factor references only `open`, `high`, `low`, `close`, `volume`, and
`vwap`. There are no group-classification or fundamental inputs.

## Calibers (differences from Qlib)

- **Warmup.** Qlib rolls with `min_periods=1`, emitting partial-window values
  from the first row. qweave requires a full, NaN-free window, so each symbol's
  first `d − 1` rows are `NaN`. Factors built on a one-step delay/delta (`ROC`,
  `CNTP`/`CNTN`/`CNTD`, the `SUM*`/`VSUM*` families, `CORD`) begin one row later
  because the `delay(x, 1)` term is `NaN` on the first row and propagates through
  the window.
- **`IMAX`/`IMIN` offset.** qweave `ts_argmax`/`ts_argmin` are 0-based (position
  within the window, 0 = oldest) while Qlib `IdxMax`/`IdxMin` are 1-based. The
  builder adds `+1` so `IMAX`/`IMIN` match Qlib. In `IMXD` the two offsets cancel,
  so it is built without `+1`.
- **Comparison / boolean NaN.** `close > delay(close, 1)` is `NaN` (not `0`) when
  the prior close is missing, so the `CNT*` averages ignore no rows but inherit
  the warmup `NaN` above.
- **Quantile.** `QTLU`/`QTLD` use pandas `linear` interpolation:
  `pos = q · (N − 1)`, interpolating between the floor and ceil order statistics.
- **Standard deviation.** `STD`, `VSTD`, and `WVMA` use the sample standard
  deviation (`ddof = 1`), matching pandas/Qlib.
- **Correlation.** `CORR`/`CORD` are Pearson correlations and return `NaN` when
  either window has zero variance.

`BETA`/`RSQR`/`RESI` and `QTLU`/`QTLD` are backed by dedicated Rust kernels
(`slope`, `rsquare`, `resi`, `quantile`) rather than being composed from other
operators. The regression kernels use the closed-form ordinary-least-squares fit
against the window index `0..d-1` (x̄ = (d−1)/2, Sxx = d(d²−1)/12); `RSQR` is
`NaN` when the window has zero variance.

## Verification

- Rust unit tests assert the builder returns exactly 158 uniquely named factors
  that reference only OHLCV and vwap, and that `IMAX` keeps its `+1` offset.
- A smoke test computes all 158 on a complete synthetic panel.
- Python tests compare 18 representative factors (one per distinct kernel and
  caliber, including the four new kernels and the warmup/NaN edges) against an
  independent NumPy reference at `1e-10` tolerance.
- A frozen synthetic Parquet golden fixture guards all 158 outputs against
  unintended numerical drift.

This project is not affiliated with Microsoft or Qlib.
