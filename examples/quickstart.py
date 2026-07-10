"""Run qweave's factor-to-report workflow on the bundled synthetic panel."""

from __future__ import annotations

from pathlib import Path

import polars as pl
import qweave as qf


EXAMPLE_DIR = Path(__file__).resolve().parent
DATA_PATH = EXAMPLE_DIR / "data" / "sample_daily.parquet"
REPORT_PATH = EXAMPLE_DIR / "output" / "qweave-report.html"
FACTOR_NAMES = ["alpha13", "alpha101", "mean_reversion_20"]


def evaluate_sample(data_path: Path = DATA_PATH):
    df = pl.read_parquet(data_path)

    alphas = qf.worldquant_alpha101({}, alphas=["alpha13", "alpha101"])
    alphas.append(
        (
            -(
                qf.col("close") / qf.col("close").delay(20) - qf.lit(1.0)
            )
        ).alias("mean_reversion_20")
    )

    df = qf.with_alphas(df, "asset", "date", alphas)
    df = qf.with_labels(
        df,
        symbol_col="asset",
        time_col="date",
        horizons=[1, 5, 20],
        entry_lag=1,
        tradable_col="tradable",
    )
    return qf.evaluate(
        df,
        symbol_col="asset",
        time_col="date",
        factor_cols=FACTOR_NAMES,
        quantiles=5,
        min_cs_count=30,
        tradable_col="tradable_entry",
    )


def main() -> None:
    result = evaluate_sample()
    REPORT_PATH.parent.mkdir(parents=True, exist_ok=True)
    result.to_html(str(REPORT_PATH))
    print(
        result.summary.select(
            "factor",
            "horizon",
            "rank_ic_mean",
            "rank_ic_ir",
            "spread_mean",
        )
    )
    print(f"\nreport: {REPORT_PATH}")


if __name__ == "__main__":
    main()
