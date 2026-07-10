# WorldQuant 101 Implementation Manifest

This manifest records the implementation defaults for the built-in `alpha1` through
`alpha101` formulas.

## Status

- Status: implemented and registered.
- Source formula set: Kakushadze, "101 Formulaic Alphas", Appendix A.
- Rust location: `crates/qweave-factors/src/worldquant101.rs`, plus the previously
  existing `alpha6`, `alpha8`, `alpha12`, `alpha13`, and `alpha101` in `alphas.rs`.
- Public Python surface: use `qweave.worldquant101_alphas(...)` to obtain
  expressions, then `qweave.compute_alphas(...)` or `qweave.with_alphas(...)`.

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

## Operator calibers

The paper leaves several operators under-specified, and reference implementations
disagree. These are the concrete calibers qweave uses. Where relevant the
DolphinDB `wq101alpha.dos` reference is noted, since it is a common cross-check.

- **`rank(x)` (cross-sectional):** percentile. Within each cross-section over the
  non-NaN cells, `rank = (average 1-based ascending rank) / N`, ties averaged.
  Range `(0, 1]` (minimum `1/N`, maximum `1`). NaN inputs are excluded from the
  cross-section and remain NaN. DolphinDB `rowRank(percent=true)` agrees for
  distinct values but takes the *minimum* rank on ties; the two differ only when
  ties occur (e.g. ranking booleans or `sign(...)`).
- **`ts_rank(x, d)` (time-series): percentile by default.** `(average 1-based rank
  of the current value within the window) / d`, ties averaged, range `(0, 1]`.
  This is the convention most quant pipelines use (pandas `rank(pct=true)`).
  `ts_rank_raw(x, d)` exposes the DolphinDB `mrank(x, true, d)` caliber instead:
  0-based ascending rank in `[0, d-1]`, minimum on ties. The default alphas use
  the percentile `ts_rank`.
- **`ts_argmax(x, d)` / `ts_argmin(x, d)`:** 0-based position of the extreme value
  within the window, counted from the oldest day (`0`) to the current day (`d-1`);
  the earliest occurrence wins on ties. Matches DolphinDB `mimax` / `mimin`.
- **Comparisons (`<, >, <=, >=, ==`) and `where(cond, a, b)`:** NaN propagates. Any
  NaN operand yields NaN, and a NaN condition yields NaN — qweave never coerces
  NaN to a boolean. (DolphinDB instead treats NULL as −∞ in comparisons, so its
  boolean results are always defined; this is why DolphinDB reports more finite
  cells during warmup for comparison/boolean alphas.)
- **Rolling windows** (`ts_sum`, `ts_mean`, `ts_std`, `ts_min`, `ts_max`,
  `ts_argmax`, `ts_argmin`, `ts_rank`, `decay_linear`, `correlation`,
  `covariance`, `product`): require a full window with no interior NaN, otherwise
  the output is NaN. (DolphinDB skips NULLs inside the window and emits a value
  once `d` positions exist, so it again reports more finite cells.)
- **Misc:** `signed_power(x, p) = sign(x) * |x|^p`; `log(x)` is the natural log for
  `x > 0` and NaN otherwise; `ts_std` and `covariance` use the sample denominator
  `n - 1`; `scale(x, a)` normalizes each cross-section so `sum(|x|) = a`
  (default `a = 1`).

## Caliber notes and cross-reference differences

Places where a formula's result depends on a caliber choice above, or where a
common reference diverges. Useful when reconciling qweave against another engine.

- **`ts_rank` scale-sensitive alphas:** `alpha4`, `alpha35`, `alpha43`, `alpha52`,
  `alpha84` consume `ts_rank` in an arithmetic context, so their magnitude depends
  on the percentile-vs-raw caliber. They match DolphinDB only under `ts_rank_raw`.
- **`ts_argmax` / `ts_argmin` alphas:** `alpha1`, `alpha57`, `alpha60`, `alpha96`,
  `alpha98`, `alpha100` depend on the position caliber (from oldest, earliest tie).
- **`alpha14`:** qweave follows the paper's `correlation(open, volume, 10)`.
  DolphinDB's reference uses covariance (`mcovar`) here, which differs in scale by
  orders of magnitude.
- **`alpha83`:** qweave follows the paper grouping
  `... / ((high - low) / (sum(close, 5) / 5) / (vwap - close))`. DolphinDB's
  reference evaluates the trailing chained division left-to-right, a different
  grouping.
- **`alpha42`:** `rank(vwap - close) / rank(vwap + close)` is sensitive to the
  `vwap` definition and to small denominator ranks; confirm the `vwap` input
  matches before comparing engines.

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
