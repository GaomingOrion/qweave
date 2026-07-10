import math

import numpy as np
import polars as pl
import pytest
import qweave

from test_evaluate import avg_rank, make_panel, run_evaluate


# ---------------------------------------------------------------------------
# Reference implementations
# ---------------------------------------------------------------------------


def day_arrays(df, factor, day, tradable_col=None):
    sub = df.filter(pl.col("time") == day).sort("asset")
    f = sub.get_column(factor).fill_null(float("nan")).to_numpy().astype(float)
    tr = (
        sub.get_column(tradable_col).fill_null(False).to_numpy().astype(bool)
        if tradable_col
        else np.ones(len(f), dtype=bool)
    )
    assets = sub.get_column("asset").to_list()
    return f, tr, assets


def reference_flows(df, factor, horizons, q, min_cs, weighting, cost_bps, tradable_col=None):
    """Turnover / portfolio / autocorr reference on the signal-day axis."""
    days = sorted(df.get_column("time").unique().to_list())
    states = {}
    for day in days:
        f, tr, assets = day_arrays(df, factor, day, tradable_col)
        valid = tr & ~np.isnan(f)
        if valid.sum() < max(min_cs, 1):
            states[day] = None
            continue
        idx = np.flatnonzero(valid)
        order = idx[np.argsort(f[idx], kind="stable")]
        m = len(order)
        buckets = np.arange(m) * q // m
        top = {assets[i] for i in order[buckets == q - 1]}
        bottom = {assets[i] for i in order[buckets == 0]}
        ranks = {assets[i]: r for i, r in zip(idx, avg_rank(f[idx]))}
        if weighting == "factor":
            dev = f[idx] - f[idx].mean()
            denom = np.abs(dev).sum()
            weights = {assets[i]: (d / denom if denom > 0 else 0.0) for i, d in zip(idx, dev)}
        else:
            weights = {}
            for name in top:
                weights[name] = 0.5 / len(top)
            for name in bottom:
                weights[name] = -0.5 / len(bottom)
        states[day] = {"top": top, "bottom": bottom, "ranks": ranks, "weights": weights}

    # ret_1 by (day, asset)
    ret1 = {}
    for row in df.iter_rows(named=True):
        ret1[(row["time"], row["asset"])] = row.get("ret_1")

    out = {"turnover": {}, "portfolio": {}, "autocorr": {}}
    for h_pos, h in enumerate(horizons):
        for t_pos, day in enumerate(days):
            state = states[day]
            if t_pos >= h:
                past = states[days[t_pos - h]]
                if state and past:
                    top_t = 1.0 - len(state["top"] & past["top"]) / len(state["top"])
                    bot_t = 1.0 - len(state["bottom"] & past["bottom"]) / len(state["bottom"])
                    out["turnover"][(day, h)] = (top_t, bot_t)

        # portfolio: wbar over last h days
        prev_wbar = None
        for t_pos, day in enumerate(days):
            window = [states[d] for d in days[max(0, t_pos - h + 1) : t_pos + 1]]
            window = [w["weights"] if w else {} for w in window]
            h_avail = len(window)
            wbar = {}
            for w in window:
                for name, weight in w.items():
                    wbar[name] = wbar.get(name, 0.0) + weight / h_avail
            gross = 0.0
            for name, weight in wbar.items():
                r = ret1.get((day, name))
                if r is not None and not math.isnan(r):
                    gross += weight * r
            if prev_wbar is None:
                turnover = float("nan")
            else:
                names = set(wbar) | set(prev_wbar)
                turnover = 0.5 * sum(
                    abs(wbar.get(n, 0.0) - prev_wbar.get(n, 0.0)) for n in names
                )
            net = gross - (0.0 if math.isnan(turnover) else turnover * cost_bps * 1e-4)
            out["portfolio"][(day, h)] = (gross, net, turnover)
            prev_wbar = wbar

    for lag in [1, 5, 10, 20]:
        if lag >= len(days):
            continue
        values = []
        for t_pos in range(lag, len(days)):
            state, past = states[days[t_pos]], states[days[t_pos - lag]]
            if not state or not past:
                continue
            common = sorted(set(state["ranks"]) & set(past["ranks"]))
            if len(common) < 2:
                continue
            x = np.array([state["ranks"][n] for n in common])
            y = np.array([past["ranks"][n] for n in common])
            dx, dy = x - x.mean(), y - y.mean()
            vx, vy = (dx * dx).sum(), (dy * dy).sum()
            if vx > 0 and vy > 0:
                values.append((dx * dy).sum() / math.sqrt(vx * vy))
        out["autocorr"][lag] = float(np.mean(values)) if values else float("nan")
    return out


def assert_close(actual, expected, label):
    if expected is None or (isinstance(expected, float) and math.isnan(expected)):
        assert actual is None or math.isnan(actual), f"{label}: {actual} != NaN"
    else:
        assert actual == pytest.approx(expected, abs=1e-10), label


# ---------------------------------------------------------------------------
# Reference comparisons
# ---------------------------------------------------------------------------


@pytest.mark.parametrize("weighting", ["factor", "quantile"])
def test_flows_match_reference(weighting):
    df = make_panel(41)
    cost_bps = 25.0
    result = run_evaluate(
        df,
        ["f1"],
        quantiles=4,
        min_cs_count=4,
        tradable_col="tradable",
        cost_bps=cost_bps,
        weighting=weighting,
    )
    ref = reference_flows(
        df, "f1", [1, 5], 4, 4, weighting, cost_bps, tradable_col="tradable"
    )

    for row in result.turnover.iter_rows(named=True):
        key = (row["date"], row["horizon"])
        expected = ref["turnover"].get(key, (float("nan"), float("nan")))
        assert_close(row["top_turnover"], expected[0], f"top {key}")
        assert_close(row["bottom_turnover"], expected[1], f"bottom {key}")

    for row in result.portfolio.iter_rows(named=True):
        key = (row["date"], row["horizon"])
        gross, net, turnover = ref["portfolio"][key]
        assert_close(row["gross"], gross, f"gross {key}")
        assert_close(row["net"], net, f"net {key}")
        assert_close(row["turnover"], turnover, f"turnover {key}")

    for row in result.rank_autocorr.iter_rows(named=True):
        assert row["factor"] == "f1"
        assert_close(row["rank_autocorr"], ref["autocorr"][row["lag"]], f"lag {row['lag']}")

    # Summary aggregates match the reference series.
    for row in result.summary.iter_rows(named=True):
        h = row["horizon"]
        gross_series = np.array([ref["portfolio"][(d, h)][0] for d in sorted(set(k[0] for k in ref["portfolio"] if k[1] == h))])
        assert row["ls_gross_ann"] == pytest.approx(np.nanmean(gross_series) * 252, abs=1e-8)


# ---------------------------------------------------------------------------
# Properties
# ---------------------------------------------------------------------------


def test_static_factor_has_zero_turnover():
    df = make_panel(43, nan_rate=0.0).with_columns(
        # Constant per asset over time.
        pl.col("asset").str.slice(1).cast(pl.Float64).alias("static")
    )
    result = run_evaluate(df, ["static"], quantiles=4, min_cs_count=4)

    turnover = result.turnover.filter(pl.col("top_turnover").is_not_nan())
    assert turnover.height > 0
    assert turnover.get_column("top_turnover").abs().max() == 0.0
    autocorr = result.rank_autocorr.get_column("rank_autocorr")
    for value in autocorr:
        assert value == pytest.approx(1.0, abs=1e-12)
    # Static weights: portfolio turnover ~0 after day 1.
    port = result.portfolio.filter(pl.col("horizon") == 1).sort("date")
    tail = port.get_column("turnover").to_list()[1:]
    assert all(abs(v) < 1e-12 for v in tail)


def test_cost_reduces_net_by_turnover():
    df = make_panel(47)
    free = run_evaluate(df, ["f1"], quantiles=4, min_cs_count=4)
    costly = run_evaluate(df, ["f1"], quantiles=4, min_cs_count=4, cost_bps=50.0)

    a = free.portfolio.sort(["horizon", "date"])
    b = costly.portfolio.sort(["horizon", "date"])
    for gross_a, net_b, to in zip(
        a.get_column("gross"), b.get_column("net"), b.get_column("turnover")
    ):
        if any(v is None or math.isnan(v) for v in (gross_a, net_b, to)):
            continue
        assert net_b == pytest.approx(gross_a - to * 50.0 * 1e-4, abs=1e-12)


def test_portfolio_requires_ret1():
    df = make_panel(53).drop("ret_1")
    result = run_evaluate(df, ["f1"], quantiles=4, min_cs_count=4)

    values = result.portfolio.get_column("gross")
    assert all(v is None or math.isnan(v) for v in values)
    row = result.summary.row(0, named=True)
    assert math.isnan(row["ls_gross_ann"])
    # Turnover is label-independent and still present.
    assert result.turnover.get_column("top_turnover").is_not_nan().any()


def test_flows_save_and_streaming(tmp_path):
    df = make_panel(59)
    memory = run_evaluate(df, ["f1", "f2"], quantiles=4, min_cs_count=4)
    streamed = run_evaluate(
        df, ["f1", "f2"], quantiles=4, min_cs_count=4, output_dir=str(tmp_path / "run")
    )

    assert streamed.turnover.collect().equals(memory.turnover)
    assert streamed.portfolio.collect().equals(memory.portfolio)
    assert streamed.rank_autocorr.equals(memory.rank_autocorr)

    memory.save(str(tmp_path / "saved"))
    assert pl.read_parquet(tmp_path / "saved" / "turnover.parquet").equals(memory.turnover)
    assert pl.read_parquet(tmp_path / "saved" / "portfolio.parquet").equals(memory.portfolio)
    assert pl.read_parquet(tmp_path / "saved" / "rank_autocorr.parquet").equals(
        memory.rank_autocorr
    )
    assert memory.meta["cost_bps"] == 0.0
    assert memory.meta["weighting"] == "quantile"


# ---------------------------------------------------------------------------
# factor_correlation
# ---------------------------------------------------------------------------


def test_factor_correlation_matches_reference():
    df = make_panel(61)
    out = qweave.factor_correlation(
        df,
        symbol_col="asset",
        time_col="time",
        factor_cols=["f1", "f2"],
        tradable_col="tradable",
        min_cs_count=4,
    )

    # Reference: per-day spearman over common valid samples, averaged.
    days = sorted(df.get_column("time").unique().to_list())
    values = []
    for day in days:
        f1, tr, _ = day_arrays(df, "f1", day, "tradable")
        f2, _, _ = day_arrays(df, "f2", day, "tradable")
        v1 = tr & ~np.isnan(f1)
        v2 = tr & ~np.isnan(f2)
        if v1.sum() < 4 or v2.sum() < 4:
            continue
        r1 = np.full(len(f1), np.nan)
        r2 = np.full(len(f2), np.nan)
        r1[np.flatnonzero(v1)] = avg_rank(f1[v1])
        r2[np.flatnonzero(v2)] = avg_rank(f2[v2])
        both = v1 & v2
        if both.sum() < 4:
            continue
        x, y = r1[both], r2[both]
        dx, dy = x - x.mean(), y - y.mean()
        vx, vy = (dx * dx).sum(), (dy * dy).sum()
        if vx > 0 and vy > 0:
            values.append((dx * dy).sum() / math.sqrt(vx * vy))
    expected = float(np.mean(values))

    assert out.columns == ["factor", "f1", "f2"]
    assert out.get_column("f1")[0] == pytest.approx(1.0, abs=1e-12)
    assert out.get_column("f2")[1] == pytest.approx(1.0, abs=1e-12)
    assert out.get_column("f2")[0] == pytest.approx(expected, abs=1e-10)
    assert out.get_column("f1")[1] == pytest.approx(expected, abs=1e-10)


def test_factor_correlation_monotone_transform_is_one():
    df = make_panel(67, nan_rate=0.0).with_columns(pl.col("f1").exp().alias("f1_exp"))
    out = qweave.factor_correlation(
        df, symbol_col="asset", time_col="time", factor_cols=["f1", "f1_exp"], min_cs_count=4
    )
    assert out.get_column("f1_exp")[0] == pytest.approx(1.0, abs=1e-12)
