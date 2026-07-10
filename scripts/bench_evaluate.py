"""Synthetic benchmark for qweave.evaluate / with_labels.

Usage (build the extension in release mode first):

    uv run maturin develop --release
    uv run python scripts/bench_evaluate.py                 # default tiers
    uv run python scripts/bench_evaluate.py --symbols 1000 --days 600 --factors 158

Default tiers:
  - full panel (5000 x 2400 ~= 12M rows): 1 and 8 factors, memory mode
  - reduced panel (1000 x 600): 158 and 1024 factors, memory + streaming

The 158/1024-factor tiers use a reduced panel because the *input* frame with
that many f64 columns on 12M rows would not fit in RAM; streaming factor
sources land in Phase 3.
"""

import argparse
import resource
import sys
import tempfile
import time

import numpy as np
import polars as pl

import qweave

HORIZONS = [1, 5, 10, 20]


def peak_rss_gib():
    rss = resource.getrusage(resource.RUSAGE_SELF).ru_maxrss
    # ru_maxrss is bytes on macOS, KiB on Linux.
    if sys.platform == "darwin":
        return rss / 2**30
    return rss / 2**20


def build_panel(n_symbols, n_days, n_factors, seed=0):
    rng = np.random.default_rng(seed)
    n = n_symbols * n_days
    symbols = np.repeat([f"S{i:05d}" for i in range(n_symbols)], n_days)
    times = np.tile(np.arange(1, n_days + 1, dtype=np.int64), n_symbols)
    data = {"asset": symbols, "time": times}
    close = 20.0 + rng.standard_normal(n).cumsum() * 0.01
    data["close"] = np.abs(close) + 1.0
    for i in range(n_factors):
        data[f"f{i}"] = rng.standard_normal(n)
    for h in HORIZONS:
        data[f"ret_{h}"] = rng.standard_normal(n) * 0.02
    data["tradable"] = rng.random(n) > 0.05
    return pl.DataFrame(data)


def run_tier(name, df, factor_cols, output_dir=None):
    start = time.perf_counter()
    result = qweave.evaluate(
        df,
        symbol_col="asset",
        time_col="time",
        factor_cols=factor_cols,
        quantiles=10,
        tradable_col="tradable",
        output_dir=output_dir,
    )
    elapsed = time.perf_counter() - start
    mode = "streamed" if output_dir else "memory"
    print(
        f"{name:<28} {len(factor_cols):>5} factors  {df.height:>10,} rows  "
        f"{elapsed:>8.2f}s  {mode:<8} peak_rss={peak_rss_gib():.2f}GiB"
    )
    return elapsed, result


def bench_with_labels(df):
    df = df.drop([c for c in df.columns if c.startswith("ret_")])
    start = time.perf_counter()
    out = qweave.with_labels(
        df,
        symbol_col="asset",
        time_col="time",
        horizons=HORIZONS,
        entry_lag=1,
        tradable_col="tradable",
    )
    elapsed = time.perf_counter() - start
    print(f"{'with_labels (4 horizons)':<28} {'-':>5}          {df.height:>10,} rows  {elapsed:>8.2f}s")
    return out, elapsed


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--symbols", type=int)
    parser.add_argument("--days", type=int)
    parser.add_argument("--factors", type=int)
    parser.add_argument("--streaming", action="store_true")
    args = parser.parse_args()

    if args.symbols or args.days or args.factors:
        n_symbols = args.symbols or 5000
        n_days = args.days or 2400
        n_factors = args.factors or 1
        print(f"building panel {n_symbols} x {n_days}, {n_factors} factors ...")
        df = build_panel(n_symbols, n_days, n_factors)
        output_dir = tempfile.mkdtemp(prefix="qweave-bench-") if args.streaming else None
        run_tier("custom", df, [f"f{i}" for i in range(n_factors)], output_dir)
        return

    print("building full panel (5000 x 2400, 8 factors) ...")
    df = build_panel(5000, 2400, 8)
    bench_with_labels(df.select(["asset", "time", "close", "tradable"]))
    elapsed, _ = run_tier("full panel", df, ["f0"])
    if elapsed > 2.0:
        print("  !! single-factor acceptance line (<= 2s) missed")
    run_tier("full panel", df, [f"f{i}" for i in range(8)])
    del df

    print("building reduced panel (1000 x 600, 1024 factors) ...")
    df = build_panel(1000, 600, 1024)
    run_tier("reduced panel", df, [f"f{i}" for i in range(158)])
    with tempfile.TemporaryDirectory(prefix="qweave-bench-") as tmp:
        run_tier("reduced panel", df, [f"f{i}" for i in range(1024)], output_dir=tmp)


if __name__ == "__main__":
    main()
