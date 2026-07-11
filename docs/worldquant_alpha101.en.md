# WorldQuant 101 Alphas

[Chinese](worldquant_alpha101.md)

qweave builds `alpha1` through `alpha101` as built-in alpha expressions. You can
select, compose, and remap these alphas the same way you would handle custom
expressions, then batch-submit them to the Rust evaluator with your own factors.

The implementation follows the formula set from Kakushadze, "101 Formulaic
Alphas", Appendix A, with project-specific defaults documented here.

This project is not affiliated with WorldQuant.

## Public Surface

```python
alphas = qweave.worldquant_alpha101(
    {"close": "adj_close"},
    alphas=["alpha13", "alpha101"],
)
qweave.compute_alphas(df, "asset", "time", alphas)
```

`worldquant_alpha101(input_alias, alphas=None)` returns `PyExpr` objects:

- Omitting `alphas` returns all 101 expressions.
- Passing `alphas` returns the named subset in request order.
- `input_alias` maps canonical input names such as `close` to physical DataFrame
  columns such as `adj_close`; pass an empty dict for identity mapping.

`compute_alphas()` evaluates expressions over the full `(time, symbol)` panel.
`with_alphas()` appends outputs to the input DataFrame in original row order.
For custom expressions, see [Python Expression API](expression_api.en.md).

## Defaults

- `adv{d}` is implemented as `ts_mean(volume, d)`, using share volume rather
  than dollar volume.
- Non-integer lookback windows use `floor(d)`.
- Paper `min(x, d)` and `max(x, d)` are implemented as rolling `ts_min(x, d)`
  and `ts_max(x, d)`.
- Dynamic exponent formulas use expression-valued `power` and `signed_power`.
- `IndClass.sector`, `IndClass.industry`, and `IndClass.subindustry` map to
  numeric columns named `sector`, `industry`, and `subindustry`.

## Coverage Tiers

- Tier A: base OHLCV fields `open`, `high`, `low`, `close`, and `volume`;
  `returns` is derived from `close`.
- Tier B: Tier A plus any referenced `vwap` or `cap`; `adv{d}` is derived from
  `volume`.
- Tier C: non-null String or integer group classification columns: `sector`, `industry`, or
  `subindustry`.

If you only have basic OHLCV data, start with Tier A alphas. Add Tier B/Tier C
once your data includes `vwap`, `cap`, or industry classifications.

## Verification

- Rust tests assert the builder returns exactly `alpha1` through `alpha101`.
- Smoke tests compute all 101 alphas on a complete synthetic panel.
- Golden regression coverage compares all alpha outputs against a frozen
  synthetic Parquet fixture.
- Independent reference tests cover representative formulas.
