# Qlib Alpha158

[Chinese](qlib_alpha158.md)

qweave builds the Microsoft Qlib `Alpha158` feature set as 158 built-in alpha
expressions. It is useful as a baseline feature set for research pipelines: it
has a small input surface, a clear structure, broad rolling-kernel coverage, and
an obvious reference point against the Qlib handler.

The formulas follow Qlib's `Alpha158` handler; project-specific calibers are
documented here.

This project is not affiliated with Microsoft or Qlib.

## Structure

The 158 factors break down as:

- **9 kbar** candlestick-shape factors: `KMID`, `KLEN`, `KMID2`, `KUP`, `KUP2`,
  `KLOW`, `KLOW2`, `KSFT`, `KSFT2`.
- **4 price** factors normalized by close: `OPEN0`, `HIGH0`, `LOW0`, `VWAP0`.
- **29 rolling groups x 5 windows** (`{5, 10, 20, 30, 60}`), named
  `<GROUP><d>`, for example `MA5` and `CORR60`.

Common rolling groups:

| Group | Meaning |
| --- | --- |
| `ROC` | `delay(close, d) / close` |
| `MA` | `ts_mean(close, d) / close` |
| `STD` | `ts_std(close, d) / close` |
| `BETA` | linear-trend slope over the window, normalized by close |
| `RSQR` | R2 of the same linear fit |
| `RESI` | residual of the last point from the fit, normalized by close |
| `MAX` / `MIN` | rolling high/low max/min, normalized by close |
| `QTLU` / `QTLD` | 0.8 / 0.2 rolling quantile of close, normalized by close |
| `RANK` | time-series percentile rank of close |
| `RSV` | `(close - ts_min(low, d)) / (ts_max(high, d) - ts_min(low, d))` |
| `IMAX` / `IMIN` / `IMXD` | high/low extreme positions and their difference |
| `CORR` / `CORD` | close and volume correlation features |
| `CNTP` / `CNTN` / `CNTD` | up/down-day fractions and their difference |
| `SUMP` / `SUMN` / `SUMD` | up/down move sums and their difference |
| `VMA` / `VSTD` / `WVMA` | volume rolling features |
| `VSUMP` / `VSUMN` / `VSUMD` | volume versions of `SUMP` / `SUMN` / `SUMD` |

All factors are per-symbol time-series or element-wise expressions. None use a
cross-section.

## Public Surface

```python
alphas = qweave.qlib_alpha158(
    {"close": "adj_close"},
    alphas=["KMID", "MA5", "CORR20"],
)
qweave.compute_alphas(df, "asset", "time", alphas)
```

`qlib_alpha158(input_alias, alphas=None)` returns `PyExpr` objects. Omitting
`alphas` returns all 158 factors; passing `alphas` returns the named subset in
request order. `input_alias` maps canonical input names to physical DataFrame
columns; pass an empty dict for identity mapping.
Each build prints the canonical-input-to-DataFrame mapping used by the requested
factors. Unmapped fields appear as identity mappings, making it clear whether
`close` was mapped to `close_adj`.

## Input Fields

Every factor references only `open`, `high`, `low`, `close`, `volume`, and
`vwap`. There are no group-classification or fundamental inputs.

## Differences from Qlib

- **Warmup:** Qlib rolls with `min_periods=1`; qweave requires a full, non-NaN
  window, so each symbol's first `d - 1` rows are NaN.
- **`IMAX` / `IMIN` offset:** qweave `ts_argmax` / `ts_argmin` are 0-based,
  while Qlib `IdxMax` / `IdxMin` are 1-based. The builder adds `+1` to match
  Qlib. `IMXD` cancels the offsets and is built without `+1`.
- **Comparison / boolean NaN:** `close > delay(close, 1)` is NaN when the prior
  close is missing; it is not coerced to 0.
- **Quantile:** `QTLU` / `QTLD` use pandas `linear` interpolation.
- **Standard deviation:** `STD`, `VSTD`, and `WVMA` use sample standard
  deviation (`ddof = 1`).
- **Correlation:** `CORR` / `CORD` are Pearson correlations and return NaN when
  either window has zero variance.

`BETA`, `RSQR`, `RESI`, `QTLU`, and `QTLD` use dedicated Rust kernels rather than
being composed from other operators.

## Verification

- Rust tests assert the builder returns 158 uniquely named factors and only
  references OHLCV plus vwap.
- A smoke test computes all 158 factors on a complete synthetic panel.
- Python tests compare representative factors against an independent NumPy
  reference.
- A frozen synthetic Parquet golden fixture guards all 158 outputs.
