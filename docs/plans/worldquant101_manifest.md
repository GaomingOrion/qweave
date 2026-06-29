# WorldQuant 101 Implementation Manifest

This manifest records the implementation defaults for the built-in `alpha1` through
`alpha101` formulas.

## Status

- Status: implemented and registered.
- Source formula set: Kakushadze, "101 Formulaic Alphas", Appendix A.
- Rust location: `crates/qfactors-factors/src/worldquant101.rs`, plus the previously
  existing `alpha6`, `alpha8`, `alpha12`, `alpha13`, and `alpha101` in `alphas.rs`.
- Public Python surface: unchanged; use `qfactors.alpha_catalog()` and
  `qfactors.compute_alphas(...)`.

## Defaults

- `adv{d}` uses the project default `adv(d) = ts_mean(volume(), d)`, i.e. share
  volume, not dollar volume.
- Non-integer lookback windows use `floor(d)`, matching Appendix A's operator note.
- Paper `min(x, d)` and `max(x, d)` are implemented as `ts_min(x, d)` and
  `ts_max(x, d)`.
- Dynamic exponent formulas such as `rank(x)^rank(y)` and
  `SignedPower(x, delta(...))` use expression-valued `power` / `signedpower`.
- `IndClass.sector`, `IndClass.industry`, and `IndClass.subindustry` map to numeric
  input columns named `sector`, `industry`, and `subindustry`.

## Coverage Tiers

- A: base OHLCV formulas. Required columns are `open`, `high`, `low`, `close`,
  and `volume`; `returns` is derived from `close`.
  `alpha1`, `alpha2`, `alpha3`, `alpha4`, `alpha6`, `alpha8`, `alpha9`,
  `alpha10`, `alpha12`, `alpha13`, `alpha14`, `alpha15`, `alpha16`, `alpha18`,
  `alpha19`, `alpha20`, `alpha22`, `alpha23`, `alpha24`, `alpha26`, `alpha29`,
  `alpha30`, `alpha33`, `alpha34`, `alpha35`, `alpha37`, `alpha38`, `alpha40`,
  `alpha44`, `alpha45`, `alpha46`, `alpha49`, `alpha51`, `alpha52`, `alpha53`,
  `alpha54`, `alpha55`, `alpha60`, `alpha92`, `alpha101`.

- B: formulas requiring extra numeric fields or `adv{d}`. Required columns are the
  A-tier columns plus any referenced `vwap` or `cap`; `adv{d}` is derived from
  `volume`.
  `alpha5`, `alpha7`, `alpha11`, `alpha17`, `alpha21`, `alpha25`, `alpha27`,
  `alpha28`, `alpha31`, `alpha32`, `alpha36`, `alpha39`, `alpha41`, `alpha42`,
  `alpha43`, `alpha47`, `alpha50`, `alpha56`, `alpha57`, `alpha61`, `alpha62`,
  `alpha64`, `alpha65`, `alpha66`, `alpha68`, `alpha71`, `alpha72`, `alpha73`,
  `alpha74`, `alpha75`, `alpha77`, `alpha78`, `alpha81`, `alpha83`, `alpha84`,
  `alpha85`, `alpha86`, `alpha88`, `alpha89`, `alpha94`, `alpha95`, `alpha96`,
  `alpha98`, `alpha99`.

- C: formulas requiring numeric group classification columns.
  `alpha48` requires `subindustry`.
  `alpha58` requires `sector`.
  `alpha59`, `alpha63`, `alpha69`, `alpha70`, `alpha80`, `alpha87`, `alpha91`,
  `alpha93`, and `alpha97` require `industry`.
  `alpha67` requires `sector` and `subindustry`.
  `alpha76` and `alpha82` require `sector`.
  `alpha79` requires `sector`.
  `alpha90` and `alpha100` require `subindustry`.

## Verification

- Rust unit coverage asserts all `alpha1..alpha101` names are registered.
- Rust smoke coverage computes all 101 alphas on a complete 260-day synthetic panel.
- Existing independent reference tests remain for representative formulas
  `alpha6`, `alpha8`, `alpha12`, `alpha13`, and `alpha101`.
