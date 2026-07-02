# WorldQuant 101 Alphas

qfactors registers `alpha1` through `alpha101` as built-in alpha expressions.
The implementation follows the formula set from Kakushadze, "101 Formulaic
Alphas", Appendix A, with project-specific defaults documented here.

## Public Surface

Use:

```python
alphas = qfactors.worldquant101_alphas(
    {"close": "adj_close"},
    alphas=["alpha13", "alpha101"],
)
qfactors.compute_alphas(df, "asset", "time", alphas)
```

`worldquant101_alphas(input_alias, alphas=None)` returns expression objects for
the built-in `alpha1` through `alpha101` set. `input_alias` maps canonical input
names such as `close` to physical DataFrame columns such as `adj_close`; pass an
empty dict for identity mapping. `compute_alphas()` evaluates those expressions
over the full `(time, symbol)` panel, while `with_alphas()` appends them to the
input DataFrame in original row order. See
[expression_api.md](expression_api.md) for custom expression construction.
Alpha executors do not accept `column_aliases`; use `input_alias` or
`PyExpr.replace_inputs()` for alpha field remapping.

## Defaults

- `adv{d}` is implemented as `ts_mean(volume, d)`, using share volume rather
  than dollar volume.
- Non-integer lookback windows use `floor(d)`.
- Paper `min(x, d)` and `max(x, d)` are implemented as rolling `ts_min(x, d)`
  and `ts_max(x, d)`.
- Dynamic exponent formulas use expression-valued `power` and `signedpower`.
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

- Rust tests assert all `alpha1` through `alpha101` names are registered.
- Smoke tests compute all 101 alphas on a complete synthetic panel.
- Golden regression coverage compares all alpha outputs against a frozen
  synthetic Parquet fixture.
- Independent reference tests cover representative formulas.

This project is not affiliated with WorldQuant.
