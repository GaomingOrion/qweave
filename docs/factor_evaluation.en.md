# Factor Evaluation

[Chinese](factor_evaluation.md)

The evaluator answers: **does this factor carry information about forward
returns?** It focuses on predictive power (IC / RankIC), monotonicity (quantile
returns), and tradability (turnover, long-short diagnostics). It is not a
backtester: it does not simulate fills, slippage, matching, capital curves, or
exit-side liquidity.

## Pipeline

Evaluation is a single-DataFrame pipeline. Each step appends columns in original
row order; no join is required.

```python
import qweave as qf

df = qf.with_alphas(
    df,
    symbol_col="symbol",
    time_col="date",
    alphas=qf.worldquant_alpha101({}),
)

df = qf.with_labels(
    df,
    symbol_col="symbol",
    time_col="date",
    horizons=[1, 5, 10, 20],
    entry_lag=1,
    entry_col="close",
    exit_col="close",
    tradable_col="tradable",
)

result = qf.evaluate(
    df,
    symbol_col="symbol",
    time_col="date",
    factor_cols=[f"alpha{i}" for i in range(1, 102)],
    quantiles=10,
    binning="daily",
    tradable_col="tradable_entry",
    demean="none",
)

result.summary.sort("rank_ic_ir", descending=True).head(20)
result.save("runs/2026-07-04/")
```

## `with_labels`

```text
ret_h(t) = exit(t + entry_lag + h) / entry(t + entry_lag) - 1
```

Bar offsets use the **panel-wide date grid**, either the union of all panel dates
or an explicit `calendar`. A missing symbol-day yields NaN instead of silently
compressing the holding period.

| parameter | default | meaning |
| --- | --- | --- |
| `horizons` | required | positive, unique holding periods in panel bars |
| `entry_lag` | `1` | bars between signal day and entry day |
| `entry_col` / `exit_col` | `"close"` | entry and exit price columns |
| `tradable_col` | `None` | boolean tradability column for the entry day |
| `calendar` | `None` | explicit trading-day series for strict validation |

The default is a conservative T+1 close-to-close caliber: compute the signal on
T close, enter on T+1 close, and hold `h` bars.

When `tradable_col` is provided, `with_labels` shifts the entry-day tradability
flag back to the signal day and appends `tradable_entry` for `evaluate`.

## `evaluate`

```python
result = qf.evaluate(
    df,
    symbol_col,
    time_col,
    factor_cols,
    label_cols=None,        # None => auto-detect ret_{h}
    quantiles=10,
    binning="daily",        # "daily" | "global"
    group_col=None,
    tradable_col=None,
    demean="none",          # "none" | "universe" | "group"
    min_cs_count=30,
    cost_bps=0.0,
    weighting="quantile",   # "quantile" | "factor"
    output_dir=None,
)
```

`factor_cols` is required because the input frame can contain prices, factors,
and labels. When `label_cols=None`, all `ret_{h}` columns are detected and the
horizon is parsed from the name.

### Validity Layers

Per day, per factor:

- **factor-valid** = tradable and factor non-NaN.
- **pair-valid(h)** = factor-valid and `ret_h` non-NaN.
- Days with fewer than `min_cs_count` factor-valid samples are skipped.
- Horizons with fewer than `min_cs_count` pair-valid samples get NaN
  IC/RankIC/spread, while bucket means remain visible with their counts.

Not-tradable samples are removed from the whole cross-section and counted in
coverage. Exit-side liquidity and trade simulation belong to backtesting, not
factor evaluation.

### IC and RankIC

- **IC** = Pearson(factor value, ret_h).
- **RankIC** = Pearson(factor rank, label rank), equivalent to Spearman with
  averaged ties.

Summary statistics are reported per `(factor, horizon)`: mean, standard
deviation, IR, Newey-West t-statistic, and win rate. The Newey-West lag is
`h - 1` to handle overlapping h-step forward returns.

### Binning and Quantile Returns

`binning="daily"` ranks each day's factor-valid samples and cuts them into
roughly equal-sized buckets. This is the most direct portfolio interpretation:
Q10 means the top 10% by factor value each day.

`binning="global"` uses fixed type-7 quantile boundaries from the pooled
all-day distribution. This answers a distributional question about factor
values; the boundaries use full-sample information and should not be treated as
tradable rules.

`quantile_returns` has one row per `(date, factor, bin)`:

```text
date | factor | bin | bin_lo | bin_hi | count | mean_ret_1 | mean_ret_5 | ...
```

`spread` is top-bucket mean minus bottom-bucket mean. `monotonicity` is the
Kendall tau between bucket index and full-period bucket mean return.

### Demean

- `none`: raw returns.
- `universe`: subtract the day's tradable equal-weight universe mean.
- `group`: subtract the day's group mean; requires `group_col` and rejects null
  groups.

### Turnover, Portfolio, Autocorrelation

- `turnover`: top/bottom bucket member turnover.
- `portfolio`: requires `ret_1`; supports `weighting="quantile"` and
  `weighting="factor"`. For h>1, weights are averaged over the last h signal
  days.
- `rank_autocorr`: factor-rank Pearson correlation between day t and day t-lag
  over common symbols.

Summary adds annualized long-short gross/net return, IR, and turnover fields.

## Result Object and Streaming

Common attributes:

- `result.summary`
- `result.ic`
- `result.quantile_returns`
- `result.coverage`
- `result.turnover`
- `result.portfolio`
- `result.rank_autocorr`
- `result.ic_monthly`
- `result.meta`
- `result.save(dir)`

With `output_dir` set, large tables are streamed to parquet in factor batches
and returned as `polars.LazyFrame` scans. Small tables stay in memory. For
thousand-factor runs, `factor_source=<parquet>` can read factor columns from
disk in batches instead of holding a wide input frame in memory.

## Interactive report

`result.view()` starts the embedded `qweave-server` and opens a Vue + ECharts
interactive report (summary table + per-factor Returns/IC tearsheets) in the
browser, with no external files required. It is best used on a filtered
shortlist, not a thousand-factor full run.

`result.to_html(path, max_detail_factors=200)` writes a single-file HTML
report.

For streamed output:

```powershell
cargo run -p qweave-server -- --dir <output_dir> --open
```

## `factor_correlation`

```python
corr = qf.factor_correlation(
    df,
    symbol_col,
    time_col,
    factor_cols,
    tradable_col=None,
    min_cs_count=30,
)
```

Returns time-averaged daily cross-sectional rank correlation as a symmetric wide
frame with a leading `factor` column. It is intended for a filtered shortlist,
not thousands of raw factors.

## Validation

1. Rust unit tests lock exact values on small panels.
2. `tests/test_evaluate.py` and `tests/test_flows.py` rederive metrics with an
   independent NumPy reference.

Intentional divergences from alphalens include Newey-West t-stats,
deterministic rank bucketing, explicit `entry_lag`, a tradable mask, Pearson IC
alongside Spearman RankIC, and no look-ahead future-return clipping.

## Non-goals

The evaluator does not do portfolio optimization, Barra-style risk-model
neutralization, event studies, pyfolio integration, precise backtesting,
matching, slippage, exit-side liquidity, or capital-curve simulation. It
measures whether a factor has information, not how much a strategy earns.
