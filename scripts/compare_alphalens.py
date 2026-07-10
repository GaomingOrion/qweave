"""Cross-check qweave.evaluate against alphalens-reloaded (dev-only, not CI).

Run with an ephemeral alphalens install:

    uv run --with alphalens-reloaded python scripts/compare_alphalens.py

Configuration is chosen so the two libraries' definitions coincide exactly:
continuous factor values without ties, a complete panel (no missing days),
entry_lag=0 close-to-close labels, universe demeaning for quantile returns,
and a per-period alphalens run (its dropna is all-periods-joint, ours is
per-horizon pairwise).

Intentional divergences that are NOT compared (documented in the plan):
  - IC: we also report Pearson on raw values; alphalens only has Spearman.
  - t-stats: ours are Newey-West corrected; alphalens assumes iid.
  - Binning with ties: we rank-bucket deterministically; pd.qcut raises.
"""

import sys

import numpy as np
import polars as pl

import qweave

try:
    import pandas as pd
    from alphalens.performance import (
        factor_information_coefficient,
        mean_return_by_quantile,
    )
    from alphalens.utils import get_clean_factor_and_forward_returns
except ImportError as exc:  # pragma: no cover
    sys.exit(f"alphalens-reloaded not installed ({exc}); see module docstring")

N_ASSETS = 50
N_DAYS = 250
QUANTILES = 5
HORIZONS = [1, 5]


def build_data(seed=0):
    rng = np.random.default_rng(seed)
    dates = pd.bdate_range("2022-01-03", periods=N_DAYS)
    assets = [f"S{i:03d}" for i in range(N_ASSETS)]
    prices = pd.DataFrame(
        np.exp(rng.normal(0, 0.02, (N_DAYS, N_ASSETS)).cumsum(axis=0)) * 100.0,
        index=dates,
        columns=assets,
    )
    factor_values = rng.normal(size=(N_DAYS, N_ASSETS))
    factor = pd.DataFrame(factor_values, index=dates, columns=assets).stack()
    factor.index = factor.index.set_names(["date", "asset"])
    return prices, factor


def qweave_results(prices, factor):
    rows = prices.stack().rename("close").reset_index()
    rows.columns = ["date", "asset", "close"]
    factor_rows = factor.rename("f").reset_index()
    merged = rows.merge(factor_rows, on=["date", "asset"])
    df = pl.from_pandas(merged)

    df = qweave.with_labels(
        df,
        symbol_col="asset",
        time_col="date",
        horizons=HORIZONS,
        entry_lag=0,
    )
    return qweave.evaluate(
        df,
        symbol_col="asset",
        time_col="date",
        factor_cols=["f"],
        quantiles=QUANTILES,
        min_cs_count=2,
        demean="universe",
    )


def main():
    prices, factor = build_data()
    result = qweave_results(prices, factor)
    ours_ic = result.ic.to_pandas()
    ours_q = result.quantile_returns.to_pandas()

    worst_ic = 0.0
    worst_q = 0.0
    for h in HORIZONS:
        # The only loss is the last h days whose forward return is undefined.
        factor_data = get_clean_factor_and_forward_returns(
            factor, prices, quantiles=QUANTILES, periods=(h,), max_loss=0.05
        )
        period = f"{h}D"

        al_ic = factor_information_coefficient(factor_data)[period]
        ours = (
            ours_ic[ours_ic["horizon"] == h]
            .set_index("date")["rank_ic"]
            .reindex(al_ic.index)
        )
        diff = (al_ic - ours).abs().max()
        worst_ic = max(worst_ic, diff)
        print(f"h={h}: RankIC vs alphalens IC   max|diff| = {diff:.3e}  ({len(al_ic)} days)")

        al_q, _ = mean_return_by_quantile(factor_data, by_date=True, demeaned=True)
        al_q = al_q[period].unstack(level="factor_quantile")
        ours_bins = (
            ours_q.pivot_table(index="date", columns="bin", values=f"mean_ret_{h}")
            .reindex(al_q.index)
        )
        ours_bins.columns = list(ours_bins.columns)
        al_q.columns = list(al_q.columns)
        diff = (al_q - ours_bins).abs().max().max()
        worst_q = max(worst_q, diff)
        print(f"h={h}: quantile mean returns    max|diff| = {diff:.3e}")

    ok = worst_ic < 1e-10 and worst_q < 1e-10
    print(f"\n{'OK' if ok else 'MISMATCH'}: worst IC diff {worst_ic:.3e}, worst quantile diff {worst_q:.3e}")
    sys.exit(0 if ok else 1)


if __name__ == "__main__":
    main()
