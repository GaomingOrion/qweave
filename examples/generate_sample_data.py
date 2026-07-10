"""Generate the deterministic synthetic panel used by the qweave quickstart."""

from __future__ import annotations

from datetime import date, timedelta
from pathlib import Path

import numpy as np
import polars as pl


OUTPUT = Path(__file__).resolve().parent / "data" / "sample_daily.parquet"


def business_days(start: date, count: int) -> list[date]:
    days: list[date] = []
    current = start
    while len(days) < count:
        if current.weekday() < 5:
            days.append(current)
        current += timedelta(days=1)
    return days


def build_sample_panel(n_assets: int = 80, n_days: int = 320, seed: int = 20260710) -> pl.DataFrame:
    """Build a synthetic OHLCV panel with mild, deterministic return persistence."""
    rng = np.random.default_rng(seed)
    market = rng.normal(0.0002, 0.006, size=n_days)
    shocks = rng.normal(0.0, 0.012, size=(n_assets, n_days))
    returns = np.empty_like(shocks)
    returns[:, 0] = market[0] + shocks[:, 0]
    for day in range(1, n_days):
        returns[:, day] = market[day] + 0.22 * returns[:, day - 1] + shocks[:, day]
    returns = np.clip(returns, -0.12, 0.12)

    close = 25.0 * np.exp(np.cumsum(returns, axis=1))
    close *= rng.uniform(0.7, 1.3, size=(n_assets, 1))
    open_ = close * (1.0 + rng.normal(0.0, 0.003, size=(n_assets, n_days)))
    high = np.maximum(open_, close) * (1.0 + rng.uniform(0.0, 0.012, size=(n_assets, n_days)))
    low = np.minimum(open_, close) * (1.0 - rng.uniform(0.0, 0.012, size=(n_assets, n_days)))
    volume = rng.lognormal(mean=13.0, sigma=0.4, size=(n_assets, n_days))
    vwap = (open_ + high + low + close) / 4.0
    amount = vwap * volume
    cap = close * rng.uniform(5e7, 2e8, size=(n_assets, 1))
    tradable = rng.random(size=(n_assets, n_days)) > 0.025

    dates = business_days(date(2024, 1, 2), n_days)
    symbols = np.repeat([f"S{i:03d}" for i in range(n_assets)], n_days)

    def flat(values: np.ndarray) -> np.ndarray:
        return values.reshape(-1)

    panel = pl.DataFrame(
        {
            "asset": symbols,
            "open": flat(open_),
            "high": flat(high),
            "low": flat(low),
            "close": flat(close),
            "volume": flat(volume),
            "vwap": flat(vwap),
            "amount": flat(amount),
            "returns": flat(returns),
            "cap": flat(cap),
            "sector": np.repeat(np.arange(n_assets) % 8, n_days),
            "industry": np.repeat(np.arange(n_assets) % 16, n_days),
            "subindustry": np.repeat(np.arange(n_assets) % 32, n_days),
            "tradable": flat(tradable),
        }
    )
    return panel.insert_column(1, pl.Series("date", dates * n_assets, dtype=pl.Date))


def main() -> None:
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    panel = build_sample_panel()
    panel.write_parquet(OUTPUT)
    print(f"wrote {panel.height:,} rows to {OUTPUT}")


if __name__ == "__main__":
    main()
