# Factor Evaluation

**Status: experimental.** The evaluation API (`with_labels`, `evaluate`,
`factor_correlation`) is new and not yet covered by a frozen golden fixture the
way the alpha engine is. Calibers are validated against an independent pure-numpy
reference and against alphalens-reloaded on a matched configuration (see
[Validation](#validation)), but the surface may still change before 1.0.

The evaluator answers *"does this factor carry information about forward
returns?"* — predictive power (IC), monotonicity (quantile returns), and
tradability (turnover, a long-short portfolio). It is not a backtester: there is
no fill simulation, slippage, or holding-period extension from exit-side
illiquidity.

## Pipeline

Evaluation is a single-DataFrame pipeline. Each stage appends columns in the
original row order; nothing is joined:

```python
import qfactors as qf

# 1) factor columns (existing API)
df = qf.with_alphas(df, symbol_col="symbol", time_col="date",
                    alphas=qf.worldquant_alpha101({}))

# 2) forward-return label columns
df = qf.with_labels(df, symbol_col="symbol", time_col="date",
                    horizons=[1, 5, 10, 20],
                    entry_lag=1, entry_col="close", exit_col="close",
                    tradable_col="tradable")   # -> ret_1..ret_20, tradable_entry

# 3) evaluate (single frame in, no join; factor_cols required)
result = qf.evaluate(df, symbol_col="symbol", time_col="date",
                     factor_cols=[f"alpha{i}" for i in range(1, 102)],
                     quantiles=10, binning="daily",
                     tradable_col="tradable_entry", demean="none")

result.summary.sort("rank_ic_ir", descending=True).head(20)
result.save("runs/2026-07-04/")
```

## `with_labels`

```
ret_h(t) = exit(t + entry_lag + h) / entry(t + entry_lag) - 1
```

Bar offsets are taken on the **panel-wide date grid** — the union of all panel
dates (or an explicit `calendar`), not each symbol's own observed bars. A
symbol missing a day yields NaN there instead of silently compressing its
holding period.

| parameter | default | meaning |
|---|---|---|
| `horizons` | — | holding periods in panel bars (integers, positive, unique) |
| `entry_lag` | `1` | bars between the signal (T) and entry (T+1 = T+1 close by default) |
| `entry_col` / `exit_col` | `"close"` | entry/exit price columns; either can be `"open"` or any price column (vwap, …) |
| `tradable_col` | `None` | boolean "can trade at the entry price on this day"; when given, `tradable_entry` is appended |
| `calendar` | `None` | explicit trading-day series for strict validation |

The default (`entry_lag=1`, close→close) is the conservative A-share T+1 caliber:
compute the factor on T's close, enter on T+1's close, hold `h` bars — no
look-ahead.

**`tradable_entry`.** The signal is on T but the trade happens on
`T + entry_lag`, so tradability must be checked on the entry day. `with_labels`
does that shift once and appends `tradable_entry` aligned back to the signal
day, for `evaluate` to consume. Missing rows and null flags on the entry day
count as *not tradable*.

**Trading-day grid.** Without a calendar the grid is the panel's own date
union, and its completeness is the caller's responsibility — a whole missing
trading day means the data source dropped that day for everyone, which belongs
in the data layer, not in a calendar-inference heuristic (weekends and holidays
are indistinguishable from dropped days from inside the data). With a
`calendar`, panel dates must be a subset of it; calendar days inside the panel's
range with no panel rows occupy a grid slot (labels crossing them are NaN) and
are reported via a `UserWarning`.

## `evaluate`

```python
result = qf.evaluate(
    df, symbol_col, time_col, factor_cols,
    label_cols=None,        # None => auto-detect ret_{h} columns
    quantiles=10,
    binning="daily",        # "daily" | "global"
    group_col=None,
    tradable_col=None,
    demean="none",          # "none" | "universe" | "group"
    min_cs_count=30,
    cost_bps=0.0,
    weighting="factor",     # "factor" | "quantile"
    output_dir=None,        # set => stream large tables to parquet
)
```

`factor_cols` is required (the frame mixes price, factor, and label columns, all
f64). `label_cols` defaults to every `ret_{h}`-named column; the integer horizon
is parsed from the name (it drives the Newey–West lag).

### Validity layers

Per day, per factor:

- **factor-valid** = tradable ∧ factor non-NaN (horizon independent).
- **pair-valid(h)** = factor-valid ∧ label(h) non-NaN.
- A day with fewer than `min_cs_count` factor-valid samples is skipped entirely
  (IC/RankIC NaN, no quantile rows); coverage still records it.
- A horizon with fewer than `min_cs_count` pair-valid samples gets NaN
  IC/RankIC/spread, but its bucket means remain visible — the `count` column
  tells you how thin the day was.

Not-tradable samples are dropped from the whole cross-section (IC, binning,
quantile returns) and counted in `coverage.n_masked`. Exit-side illiquidity and
buy/sell direction are **not** modeled — that is backtest simulation.

### IC and RankIC

Both are computed per day, pairwise-dropping NaN and not-tradable samples:

- **IC** = Pearson(factor value, ret_h).
- **RankIC** = Pearson(factor rank, label rank) = Spearman, ties averaged.

We report Pearson IC on raw values *in addition to* RankIC; alphalens reports
only Spearman.

Summary statistics per (factor, horizon): mean, std, IR = mean/std,
`t_nw`, `win_rate`. `t_nw` is a Newey–West t-statistic with a Bartlett kernel at
lag = h−1 — overlapping h>1 forward returns autocorrelate the daily IC series,
and an iid t-stat would systematically overstate significance. h=1 recovers the
plain t-statistic.

### Binning and quantile returns

`binning="daily"` (default): each day's factor-valid samples are stably ranked
(ties broken by symbol order, recorded), then bucketed `⌊pos·q/m⌋`. Equal-sized,
deterministic, never raises on ties (unlike `pd.qcut`). This is the
combination-interpretable caliber — "Q10 = long the top 10% by factor value each
day" — whose return series composes into a net-value curve.

`binning="global"`: cut points come from the pooled all-day distribution
(type-7 quantiles); bucket boundaries are fixed. This answers the distributional
question "is factor *value* monotone in forward return" (a weight-of-evidence
view). Two caveats, documented because they bite: a non-stationary factor's
per-bucket samples are unevenly distributed in time (time effects contaminate
the cross-sectional read), and the boundaries embed full-sample information (any
portfolio built by bucket has look-ahead).

The `quantile_returns` table is one row per (date, factor, non-empty bucket):

```
date | factor | bin | bin_lo | bin_hi | count | mean_ret_1 | mean_ret_5 | ...
```

`bin_lo`/`bin_hi` are the bucket's actual factor-value range that day (daily
mode — also lets you watch factor drift; global mode shows the fixed cut
points). `count` is the factor-valid sample count; each `mean_ret_h`'s
denominator is that horizon's pair-valid count, so a bucket can have `count=40`
but average a horizon over fewer names.

`spread` (in the summary) is top-bucket mean − bottom-bucket mean, with its own
NW t-statistic. `monotonicity` is the Kendall τ of (bucket index, full-period
bucket mean return).

### demean

`none` (default): raw returns. `universe`: subtract the day's tradable
equal-weight mean (quantile returns become excess-over-market; the top−bottom
spread is unchanged, and IC is unchanged because subtracting a per-day constant
does not move a correlation). `group`: subtract the day's group mean — this is
industry-neutral, and makes IC an in-industry IC. `group` requires `group_col`
and rejects null groups.

### Turnover, portfolio, autocorrelation

Cross-day metrics, computed in a separate sequential pass per factor
(parallelized across factors):

- **Quantile turnover** (`turnover` table): `1 − |top_t ∩ top_{t−h}| / |top_t|`,
  and likewise for the bottom bucket, per horizon.
- **Long-short portfolio** (`portfolio` table): needs a `ret_1` column. Weights
  are `weighting="factor"` (default; `(f − mean)/Σ|·|`, gross leverage 1) or
  `"quantile"` (top +0.5/n, bottom −0.5/n). For h>1, the portfolio is the
  average of the last h signal days' weights (the staggered-substrategy caliber
  that alphalens-reloaded dropped — restored here); close-to-close it is
  equivalent to averaging h overlapping substrategies. Turnover =
  `0.5·Σ|wbar_t − wbar_{t−1}|`; `net = gross − turnover · cost_bps/1e4`. A
  position in a name with no `ret_1` today contributes zero (a suspension
  approximation).
- **Rank autocorrelation** (`rank_autocorr` table): factor-rank Pearson between
  day t and t−lag on common symbols, averaged over time, for lag ∈ {1,5,10,20}.

Summary adds `ls_gross_ann` / `ls_net_ann` / `ls_ir` (annualized at 252),
`ls_turnover`, `top_turnover`, `bottom_turnover`.

### Result object and streaming

`result.summary` / `result.ic` / `result.quantile_returns` / `result.coverage`
/ `result.turnover` / `result.portfolio` / `result.rank_autocorr` /
`result.ic_monthly` (present only for Date/Datetime time columns) /
`result.meta` (the parameter snapshot) / `result.save(dir)`.

With `output_dir` set, the large tables (`ic`, `quantile_returns`, `coverage`,
`turnover`, `portfolio`) are streamed to parquet in factor batches and returned
as `polars.LazyFrame` scans; the small tables stay in memory. This bounds peak
memory for thousand-factor runs — the dominant cost is `quantile_returns`
(T×F×Q rows). `save()` writes the same contract to a directory (one parquet per
table plus `meta.json`) and is memory-mode only.

For thousand-factor runs the wide *input* frame is itself the memory ceiling.
Pass `factor_source=<parquet>` (e.g. a `compute_alphas(output_path=...)` panel
covering the same `(symbol, time)` panel) to read factor columns from disk in
batches instead of from `df`; only the label / tradable / group columns need to
be materialized.

### HTML report

`result.to_html(path, max_detail_factors=200)` writes a single self-contained
HTML file — a sortable, filterable multi-factor summary table plus a per-factor
drill-down (mean return by quantile across horizons, monthly IC) drawn with
inline SVG. No external assets, no server; it opens straight from disk. Memory
mode only; the drill-down bundle is capped at `max_detail_factors` factors (the
summary table always covers every factor) to bound file size, so sort/filter the
result to the shortlist you care about first.

### Interactive report (Vue + Axum + ECharts)

For a richer, interactive view, `result.view()` opens a browser report with a
sortable summary table and per-factor tearsheets drawn with ECharts (tooltips,
zoom, legend toggles). The Returns tab shows mean return by quantile, cumulative
return by quantile, the long-short cumulative net-value with drawdown, and the
top−bottom spread; the IC tab shows daily IC/RankIC with a rolling mean, the IC
distribution, and the monthly IC heatmap.

```python
result = qf.evaluate(df, "symbol", "date", factor_cols)
result.view()  # opens the browser; blocks until Ctrl-C
```

The server (`qfactors-server`) and the compiled frontend are embedded in the
extension module — `view()` needs no ports, paths, or external binary. It is
read-only and filters the in-memory tables per factor, so use it on a filtered
shortlist rather than a thousand-factor run. `view()` is memory-mode only; for a
streamed `output_dir`, `save(dir)` then serve it from the CLI:

```powershell
cargo run -p qfactors-server -- --dir <output_dir> --open
```

Building the Python wheel embeds the frontend, so build the SPA first (one-time,
requires Node/npm):

```powershell
cd frontend; npm install; npm run build; cd ..
```

## `factor_correlation`

```python
corr = qf.factor_correlation(df, symbol_col, time_col, factor_cols,
                             tradable_col=None, min_cs_count=30)
```

Time-averaged daily cross-sectional rank correlation, returned as a symmetric
F×F wide frame with a leading `factor` column. Intended for the *filtered*
shortlist after `evaluate` (it holds every factor column densely in memory), not
thousands of raw factors.

## Validation

Three layers, mirroring the alpha engine's approach:

1. **Hand-computed unit tests** in Rust lock exact values on small panels.
2. **Independent numpy reference** (`tests/test_evaluate.py`,
   `tests/test_flows.py`) re-derives every metric with per-day Python loops and
   matches the Rust kernel to `1e-10` across `daily`/`global` × demean modes,
   with NaN / ties / tradable masks / streaming vs memory.
3. **alphalens-reloaded cross-check** (`scripts/compare_alphalens.py`, dev-only,
   not in CI): on a tie-free, complete panel with `entry_lag=0`, universe
   demeaning, and matched quantiles, RankIC and quantile returns agree to
   machine precision (~1e-17).

Intentional divergences from alphalens (each a deliberate correction, not a
match target): Newey–West t-stats vs iid; deterministic rank-bucketing vs
`pd.qcut` raising on ties; explicit `entry_lag` vs prices carrying the execution
lag implicitly; a tradable mask; reporting Pearson IC alongside Spearman; and no
`filter_zscore` future-return clipping (alphalens's default has a documented
look-ahead).

## Non-goals

Portfolio optimization / Barra-style risk-model neutralization (only group
demeaning is offered); event studies; pyfolio integration; intraday frequency;
and precise backtesting (matching, slippage, exit-side illiquidity, capital
curves). The evaluator answers whether a factor has information, not how much a
strategy earns.
