import math
import random

import polars as pl
import pytest
import qfactors


def reference_labels(
    df,
    horizons,
    entry_lag=1,
    entry_col="close",
    exit_col="close",
    tradable_col=None,
    grid=None,
):
    """Pure-Polars reference: dense (grid x symbol) panel, shift within symbol."""
    if grid is None:
        grid = df.select(pl.col("time").unique().sort())
    else:
        grid = pl.DataFrame({"time": grid})
    symbols = df.select(pl.col("asset").unique().sort())
    dense = grid.join(symbols, how="cross").join(df, on=["time", "asset"], how="left")
    dense = dense.sort(["asset", "time"])

    exprs = []
    for h in horizons:
        ret = (
            pl.col(exit_col).shift(-(entry_lag + h)) / pl.col(entry_col).shift(-entry_lag)
            - 1.0
        ).over("asset", order_by="time")
        exprs.append(ret.fill_null(float("nan")).alias(f"ret_{h}"))
    if tradable_col is not None:
        tradable = (
            pl.col(tradable_col)
            .fill_null(False)
            .shift(-entry_lag, fill_value=False)
            .over("asset", order_by="time")
        )
        exprs.append(tradable.alias("tradable_entry"))
    dense = dense.with_columns(exprs)

    keep = ["time", "asset"] + [f"ret_{h}" for h in horizons]
    if tradable_col is not None:
        keep.append("tradable_entry")
    return df.join(dense.select(keep), on=["time", "asset"], how="left", maintain_order="left")


def random_panel(seed, n_assets=6, n_times=40, drop_rate=0.2, with_tradable=False):
    rng = random.Random(seed)
    rows = []
    for asset_idx in range(n_assets):
        asset = f"S{asset_idx:02d}"
        for t in range(1, n_times + 1):
            if rng.random() < drop_rate:
                continue
            close = 50.0 + asset_idx * 10.0 + rng.uniform(-5.0, 5.0)
            row = {
                "asset": asset,
                "time": t,
                "open": close * rng.uniform(0.97, 1.03),
                "close": float("nan") if rng.random() < 0.05 else close,
            }
            if with_tradable:
                roll = rng.random()
                row["tradable"] = None if roll < 0.1 else roll > 0.3
            rows.append(row)
    rng.shuffle(rows)
    return pl.DataFrame(rows)


def assert_frames_match(actual, expected, columns):
    assert actual.height == expected.height
    for column in columns:
        left = actual.get_column(column).to_list()
        right = expected.get_column(column).to_list()
        for row, (a, e) in enumerate(zip(left, right)):
            if isinstance(e, float) and math.isnan(e):
                assert a is None or math.isnan(a), f"{column} row {row}: {a} != NaN"
            else:
                assert a == pytest.approx(e), f"{column} row {row}: {a} != {e}"


def test_with_labels_matches_polars_reference_on_random_panels():
    for seed in range(5):
        df = random_panel(seed)
        horizons = [1, 5, 10]

        out = qfactors.with_labels(
            df, symbol_col="asset", time_col="time", horizons=horizons, entry_lag=1
        )
        expected = reference_labels(df, horizons, entry_lag=1)

        assert out.columns == df.columns + [f"ret_{h}" for h in horizons]
        assert out.select(df.columns).equals(df)
        assert_frames_match(out, expected, [f"ret_{h}" for h in horizons])


def test_with_labels_open_entry_and_lag_variants_match_reference():
    df = random_panel(97, drop_rate=0.1)
    for entry_lag, entry_col, exit_col in [(0, "close", "close"), (1, "open", "close"), (2, "close", "open")]:
        out = qfactors.with_labels(
            df,
            symbol_col="asset",
            time_col="time",
            horizons=[2],
            entry_lag=entry_lag,
            entry_col=entry_col,
            exit_col=exit_col,
        )
        expected = reference_labels(
            df, [2], entry_lag=entry_lag, entry_col=entry_col, exit_col=exit_col
        )
        assert_frames_match(out, expected, ["ret_2"])


def test_with_labels_tradable_entry_matches_reference():
    df = random_panel(7, with_tradable=True)

    out = qfactors.with_labels(
        df,
        symbol_col="asset",
        time_col="time",
        horizons=[1],
        tradable_col="tradable",
    )
    expected = reference_labels(df, [1], tradable_col="tradable")

    assert out.get_column("tradable_entry").dtype == pl.Boolean
    assert_frames_match(out, expected, ["ret_1", "tradable_entry"])


def test_with_labels_calendar_grid_and_warning():
    df = pl.DataFrame(
        {
            "asset": ["A", "A", "A"],
            "time": [1, 2, 4],
            "close": [10.0, 11.0, 13.0],
        }
    ).with_columns(pl.col("time").cast(pl.Int64))
    calendar = pl.Series("calendar", [1, 2, 3, 4, 5], dtype=pl.Int64)

    with pytest.warns(UserWarning, match="1 calendar day"):
        out = qfactors.with_labels(
            df, symbol_col="asset", time_col="time", horizons=[1], calendar=calendar
        )
    expected = reference_labels(df, [1], grid=calendar.rename("time"))

    assert_frames_match(out, expected, ["ret_1"])

    with pytest.raises(ValueError, match="not in the provided calendar"):
        qfactors.with_labels(
            df,
            symbol_col="asset",
            time_col="time",
            horizons=[1],
            calendar=pl.Series("calendar", [1, 2, 3], dtype=pl.Int64),
        )


def test_with_labels_date_time_column():
    from datetime import date

    dates = [date(2026, 1, 5), date(2026, 1, 6), date(2026, 1, 7)]
    df = pl.DataFrame(
        {
            "asset": ["A", "A", "A"],
            "time": dates,
            "close": [10.0, 11.0, 12.1],
        }
    )

    out = qfactors.with_labels(
        df, symbol_col="asset", time_col="time", horizons=[1], entry_lag=0
    )

    values = out.get_column("ret_1").to_list()
    assert values[0] == pytest.approx(0.1)
    assert values[1] == pytest.approx(0.1)
    assert math.isnan(values[2])


def test_with_labels_chains_after_with_alphas():
    df = random_panel(3, drop_rate=0.0).drop("open")
    alpha = (qfactors.col("close") / qfactors.col("close").delay(1)).alias("mom1")

    out = qfactors.with_alphas(df, symbol_col="asset", time_col="time", alphas=[alpha])
    out = qfactors.with_labels(out, symbol_col="asset", time_col="time", horizons=[1, 5])

    assert out.columns == df.columns + ["mom1", "ret_1", "ret_5"]
    assert out.select(df.columns).equals(df)


def test_with_labels_rejects_bad_inputs():
    df = pl.DataFrame({"asset": ["A"], "time": [1], "close": [1.0], "ret_1": [0.0]})

    with pytest.raises(ValueError, match="already exists"):
        qfactors.with_labels(df, symbol_col="asset", time_col="time", horizons=[1])
    with pytest.raises(ValueError, match="horizons"):
        qfactors.with_labels(df, symbol_col="asset", time_col="time", horizons=[])
    with pytest.raises(ValueError, match="horizons"):
        qfactors.with_labels(df, symbol_col="asset", time_col="time", horizons=[2, 2])
    with pytest.raises(ValueError, match="expected bool"):
        qfactors.with_labels(
            df, symbol_col="asset", time_col="time", horizons=[2], tradable_col="close"
        )
