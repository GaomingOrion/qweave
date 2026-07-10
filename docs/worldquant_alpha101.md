# WorldQuant 101 Alphas

qweave builds `alpha1` through `alpha101` as built-in alpha expressions.
The implementation follows the formula set from Kakushadze, "101 Formulaic
Alphas", Appendix A, with project-specific defaults documented here.

## Public Surface

Use:

```python
alphas = qweave.worldquant_alpha101(
    {"close": "adj_close"},
    alphas=["alpha13", "alpha101"],
)
qweave.compute_alphas(df, "asset", "time", alphas)
```

`worldquant_alpha101(input_alias, alphas=None)` returns `PyExpr` objects for the
built-in `alpha1` through `alpha101` set (all 101 when `alphas` is omitted, or
the named subset in request order). `input_alias` maps canonical input names
such as `close` to physical DataFrame columns such as `adj_close`; pass an empty
dict for identity mapping. `compute_alphas()` evaluates those expressions over
the full `(time, symbol)` panel, while `with_alphas()` appends them to the input
DataFrame in original row order. See
[expression_api.md](expression_api.md) for custom expression construction.
Field remapping happens inside the expression tree, so use `input_alias` (or
`PyExpr.replace_inputs()`) rather than a separate executor-level alias argument.

## Defaults

- `adv{d}` is implemented as `ts_mean(volume, d)`, using share volume rather
  than dollar volume.
- Non-integer lookback windows use `floor(d)`.
- Paper `min(x, d)` and `max(x, d)` are implemented as rolling `ts_min(x, d)`
  and `ts_max(x, d)`.
- Dynamic exponent formulas use expression-valued `power` and `signed_power`.
- `IndClass.sector`, `IndClass.industry`, and `IndClass.subindustry` map to
  numeric input columns named `sector`, `industry`, and `subindustry`.

## Coverage Tiers

Tier A requires base OHLCV columns: `open`, `high`, `low`, `close`, and
`volume`. `returns` is derived from `close`.

Tier B requires the Tier A columns plus any referenced `vwap` or `cap` field.
`adv{d}` is derived from `volume`.

Tier C requires numeric group classification columns: `sector`, `industry`, or
`subindustry`, depending on the formula.

The detailed implementation manifest is kept in
[docs/plans/worldquant101_manifest.md](plans/worldquant101_manifest.md).

## Verification

- Rust tests assert the builder returns exactly `alpha1` through `alpha101`.
- Smoke tests compute all 101 alphas on a complete synthetic panel.
- Golden regression coverage compares all alpha outputs against a frozen
  synthetic Parquet fixture.
- Independent reference tests cover representative formulas.

This project is not affiliated with WorldQuant.
