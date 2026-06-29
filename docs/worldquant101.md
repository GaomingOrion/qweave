# WorldQuant 101 Alphas

qfactors registers `alpha1` through `alpha101` as built-in alpha expressions.
The implementation follows the formula set from Kakushadze, "101 Formulaic
Alphas", Appendix A, with project-specific defaults documented here.

## Public Surface

Use:

```python
qfactors.alpha_catalog()
qfactors.compute_alphas(...)
```

`alpha_catalog()` returns the registered alpha names, expressions, required
input fields, input count, and lookback metadata.

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
