import math
import random

import numpy as np
import polars as pl
import pytest
import qweave


# ---------------------------------------------------------------------------
# Independent reference implementation (numpy, per-day loops)
# ---------------------------------------------------------------------------


def avg_rank(values):
    """1-based average ranks with ties sharing the mean of their positions."""
    order = np.argsort(values, kind="stable")
    ranks = np.empty(len(values), dtype=float)
    sorted_values = values[order]
    i = 0
    while i < len(values):
        j = i
        while j < len(values) and sorted_values[j] == sorted_values[i]:
            j += 1
        ranks[order[i:j]] = (i + 1 + j) / 2
        i = j
    return ranks


def nw_t(series, lag):
    x = series[~np.isnan(series)]
    n = len(x)
    if n < 2:
        return float("nan")
    mean = x.mean()
    d = x - mean
    max_lag = min(lag, n - 1)
    s = (d * d).sum() / n
    for l in range(1, max_lag + 1):
        gamma = (d[l:] * d[: n - l]).sum() / n
        s += 2.0 * (1.0 - l / (max_lag + 1)) * gamma
    if s <= 0:
        return float("nan")
    return mean / math.sqrt(s / n)


def corr(x, y):
    if len(x) < 2:
        return float("nan")
    dx = x - x.mean()
    dy = y - y.mean()
    vx = (dx * dx).sum()
    vy = (dy * dy).sum()
    if vx <= 0 or vy <= 0:
        return float("nan")
    return (dx * dy).sum() / math.sqrt(vx * vy)


def reference_evaluate(
    df,
    factor,
    horizons,
    quantiles,
    min_cs_count,
    tradable_col=None,
    binning="daily",
    demean="none",
    group_col=None,
):
    """Per-factor reference: dicts keyed by day (and bin) with expected values."""
    q = quantiles
    days = sorted(df.get_column("time").unique().to_list())
    per_day = {
        day: df.filter(pl.col("time") == day).sort("asset") for day in days
    }

    def arrays(sub):
        f = sub.get_column(factor).fill_null(float("nan")).to_numpy().astype(float)
        tr = (
            sub.get_column(tradable_col).fill_null(False).to_numpy().astype(bool)
            if tradable_col
            else np.ones(len(f), dtype=bool)
        )
        ys = []
        for h in horizons:
            y = sub.get_column(f"ret_{h}").fill_null(float("nan")).to_numpy().astype(float)
            y = y.copy()
            valid = tr & ~np.isnan(y)
            if demean == "universe":
                if valid.any():
                    y[~np.isnan(y)] -= y[valid].mean()
            elif demean == "group":
                groups = sub.get_column(group_col).to_numpy()
                for g in set(groups):
                    in_g = groups == g
                    if (valid & in_g).any():
                        mean = y[valid & in_g].mean()
                        y[in_g & ~np.isnan(y)] -= mean
            ys.append(y)
        return f, tr, ys

    cuts = None
    pooled_lo = pooled_hi = None
    if binning == "global":
        pooled = []
        for day in days:
            f, tr, _ = arrays(per_day[day])
            pooled.append(f[tr & ~np.isnan(f)])
        pooled = np.sort(np.concatenate(pooled))
        cuts = np.quantile(pooled, [k / q for k in range(1, q)]) if len(pooled) else None
        pooled_lo, pooled_hi = (pooled[0], pooled[-1]) if len(pooled) else (None, None)

    ic = {}
    quantile_rows = {}
    coverage = {}
    for day in days:
        sub = per_day[day]
        f, tr, ys = arrays(sub)
        fv = tr & ~np.isnan(f)
        m = int(fv.sum())
        coverage[day] = (m, int((~tr & ~np.isnan(f)).sum()))
        if m < max(min_cs_count, 1):
            for h in horizons:
                ic[(day, h)] = (float("nan"), float("nan"))
            continue

        fv_idx = np.flatnonzero(fv)
        order = fv_idx[np.argsort(f[fv_idx], kind="stable")]
        if binning == "daily":
            buckets = np.arange(m) * q // m
        else:
            buckets = np.searchsorted(cuts, f[order], side="left")

        for h_pos, h in enumerate(horizons):
            y = ys[h_pos]
            pair = order[~np.isnan(y[order])]
            n = len(pair)
            if n >= max(min_cs_count, 2):
                day_ic = corr(f[pair], y[pair])
                day_rank_ic = corr(avg_rank(f[pair]), avg_rank(y[pair]))
            else:
                day_ic = day_rank_ic = float("nan")
            ic[(day, h)] = (day_ic, day_rank_ic)

        for b in range(q):
            members = order[buckets == b]
            if len(members) == 0:
                continue
            if binning == "daily":
                lo, hi = f[members].min(), f[members].max()
            else:
                lo = pooled_lo if b == 0 else cuts[b - 1]
                hi = pooled_hi if b == q - 1 else cuts[b]
            means = []
            for h_pos, h in enumerate(horizons):
                y = ys[h_pos][members]
                y = y[~np.isnan(y)]
                means.append(y.mean() if len(y) else float("nan"))
            quantile_rows[(day, b + 1)] = (lo, hi, len(members), means)

    return {"ic": ic, "quantile_rows": quantile_rows, "coverage": coverage}


# ---------------------------------------------------------------------------
# Panel fixtures
# ---------------------------------------------------------------------------


def make_panel(seed, n_assets=8, n_days=30, nan_rate=0.10):
    rng = np.random.default_rng(seed)
    rows = []
    for a in range(n_assets):
        for t in range(1, n_days + 1):
            rows.append(
                {
                    "asset": f"S{a:02d}",
                    "time": t,
                    # f1: continuous with NaN holes; f2: heavily tied values.
                    "f1": float(rng.normal()) if rng.random() > nan_rate else float("nan"),
                    "f2": float(round(rng.normal(), 1)),
                    "ret_1": float(rng.normal() * 0.02)
                    if rng.random() > nan_rate
                    else float("nan"),
                    "ret_5": float(rng.normal() * 0.05),
                    "tradable": bool(rng.random() > 0.15),
                    "industry": ["tech", "fin", "cons"][a % 3],
                }
            )
    random.Random(seed).shuffle(rows)
    return pl.DataFrame(rows)


def run_evaluate(df, factor_cols, **kwargs):
    return qweave.evaluate(
        df,
        symbol_col="asset",
        time_col="time",
        factor_cols=factor_cols,
        **kwargs,
    )


def assert_factor_matches_reference(result, df, factor, horizons, q, min_cs, **ref_kwargs):
    ref = reference_evaluate(df, factor, horizons, q, min_cs, **ref_kwargs)

    ic = result.ic
    if isinstance(ic, pl.LazyFrame):
        ic = ic.collect()
    for row in ic.filter(pl.col("factor") == factor).iter_rows(named=True):
        expected_ic, expected_rank = ref["ic"][(row["date"], row["horizon"])]
        for actual, expected, name in [
            (row["ic"], expected_ic, "ic"),
            (row["rank_ic"], expected_rank, "rank_ic"),
        ]:
            if math.isnan(expected):
                assert actual is None or math.isnan(actual), (
                    f"{factor} {name} day={row['date']} h={row['horizon']}: "
                    f"{actual} != NaN"
                )
            else:
                assert actual == pytest.approx(expected, abs=1e-10), (
                    f"{factor} {name} day={row['date']} h={row['horizon']}"
                )

    qr = result.quantile_returns
    if isinstance(qr, pl.LazyFrame):
        qr = qr.collect()
    qr = qr.filter(pl.col("factor") == factor)
    seen = set()
    for row in qr.iter_rows(named=True):
        key = (row["date"], row["bin"])
        seen.add(key)
        lo, hi, count, means = ref["quantile_rows"][key]
        assert row["bin_lo"] == pytest.approx(lo, abs=1e-10), f"{factor} lo {key}"
        assert row["bin_hi"] == pytest.approx(hi, abs=1e-10), f"{factor} hi {key}"
        assert row["count"] == count, f"{factor} count {key}"
        for h, expected in zip(horizons, means):
            actual = row[f"mean_ret_{h}"]
            if math.isnan(expected):
                assert actual is None or math.isnan(actual), f"{factor} mean {key} h={h}"
            else:
                assert actual == pytest.approx(expected, abs=1e-10), (
                    f"{factor} mean {key} h={h}"
                )
    assert seen == set(ref["quantile_rows"].keys()), f"{factor}: bin row mismatch"

    cov = result.coverage
    if isinstance(cov, pl.LazyFrame):
        cov = cov.collect()
    for row in cov.filter(pl.col("factor") == factor).iter_rows(named=True):
        n_valid, n_masked = ref["coverage"][row["date"]]
        assert row["n_valid"] == n_valid
        assert row["n_masked"] == n_masked

    # Summary IC stats recomputed from the reference IC series.
    days = sorted({d for d, _ in ref["ic"]})
    summary = result.summary.filter(pl.col("factor") == factor)
    for row in summary.iter_rows(named=True):
        h = row["horizon"]
        series = np.array([ref["ic"][(d, h)][0] for d in days])
        rank_series = np.array([ref["ic"][(d, h)][1] for d in days])
        clean = series[~np.isnan(series)]
        assert row["n_days"] == len(clean)
        if len(clean):
            assert row["ic_mean"] == pytest.approx(clean.mean(), abs=1e-10)
            assert row["ic_t_nw"] == pytest.approx(nw_t(series, h - 1), abs=1e-10)
            assert row["rank_ic_t_nw"] == pytest.approx(nw_t(rank_series, h - 1), abs=1e-10)
            assert row["ic_win_rate"] == pytest.approx((clean > 0).mean(), abs=1e-10)


# ---------------------------------------------------------------------------
# Reference comparisons across configurations
# ---------------------------------------------------------------------------


@pytest.mark.parametrize("seed", [0, 1])
@pytest.mark.parametrize(
    "config",
    [
        {"binning": "daily", "demean": "none"},
        {"binning": "daily", "demean": "universe"},
        {"binning": "global", "demean": "none"},
        {"binning": "daily", "demean": "group"},
    ],
    ids=["daily-none", "daily-universe", "global-none", "daily-group"],
)
def test_evaluate_matches_reference(seed, config):
    df = make_panel(seed)
    kwargs = dict(config)
    if kwargs["demean"] == "group":
        kwargs["group_col"] = "industry"
    result = run_evaluate(
        df,
        ["f1", "f2"],
        quantiles=4,
        min_cs_count=4,
        tradable_col="tradable",
        **kwargs,
    )
    for factor in ["f1", "f2"]:
        assert_factor_matches_reference(
            result,
            df,
            factor,
            [1, 5],
            4,
            4,
            tradable_col="tradable",
            binning=config["binning"],
            demean=config["demean"],
            group_col="industry" if config["demean"] == "group" else None,
        )


# ---------------------------------------------------------------------------
# Property tests
# ---------------------------------------------------------------------------


def test_monotone_transform_preserves_rank_ic_and_negation_flips_ic():
    df = make_panel(7).with_columns(
        pl.col("f2").exp().alias("f2_exp"),
        (-pl.col("f2")).alias("f2_neg"),
    )
    result = run_evaluate(df, ["f2", "f2_exp", "f2_neg"], quantiles=4, min_cs_count=4)

    ic = result.ic
    base = ic.filter(pl.col("factor") == "f2").sort(["horizon", "date"])
    exp = ic.filter(pl.col("factor") == "f2_exp").sort(["horizon", "date"])
    neg = ic.filter(pl.col("factor") == "f2_neg").sort(["horizon", "date"])
    for a, b in zip(base.get_column("rank_ic"), exp.get_column("rank_ic")):
        if a is not None and not math.isnan(a):
            assert b == pytest.approx(a, abs=1e-10)
    for a, b in zip(base.get_column("ic"), neg.get_column("ic")):
        if a is not None and not math.isnan(a):
            assert b == pytest.approx(-a, abs=1e-10)

    # Bucket means reverse under negation — exact only for distinct values and
    # q dividing the daily valid count, so use a NaN-free panel (8 assets, q=4).
    clean = make_panel(7, nan_rate=0.0).with_columns((-pl.col("f1")).alias("f1_neg"))
    clean_result = run_evaluate(clean, ["f1", "f1_neg"], quantiles=4, min_cs_count=4)
    qr = clean_result.quantile_returns
    top = qr.filter((pl.col("factor") == "f1") & (pl.col("bin") == 4)).sort("date")
    bottom_neg = qr.filter((pl.col("factor") == "f1_neg") & (pl.col("bin") == 1)).sort("date")
    assert top.height == bottom_neg.height > 0
    for a, b in zip(top.get_column("mean_ret_1"), bottom_neg.get_column("mean_ret_1")):
        both_nan = (a is None or math.isnan(a)) and (b is None or math.isnan(b))
        assert both_nan or b == pytest.approx(a, abs=1e-10)


def test_symbol_permutation_invariance():
    df = make_panel(11)
    shuffled = df.sample(fraction=1.0, shuffle=True, seed=99)

    a = run_evaluate(df, ["f1"], quantiles=4, min_cs_count=4)
    b = run_evaluate(shuffled, ["f1"], quantiles=4, min_cs_count=4)

    assert a.summary.equals(b.summary)
    assert a.ic.sort(["horizon", "date"]).equals(b.ic.sort(["horizon", "date"]))


def test_self_factor_has_unit_rank_ic():
    df = make_panel(3).with_columns(pl.col("ret_1").alias("fcopy"))
    result = run_evaluate(df, ["fcopy"], label_cols=["ret_1"], quantiles=4, min_cs_count=4)

    ic = result.ic.get_column("rank_ic").drop_nans().drop_nulls()
    assert len(ic) > 0
    for value in ic:
        assert value == pytest.approx(1.0, abs=1e-12)


def test_daily_bin_counts_differ_by_at_most_one():
    df = make_panel(5)
    result = run_evaluate(df, ["f1", "f2"], quantiles=3, min_cs_count=3)

    spread = (
        result.quantile_returns.group_by(["date", "factor"])
        .agg((pl.col("count").max() - pl.col("count").min()).alias("spread"))
        .get_column("spread")
    )
    assert spread.max() <= 1


def test_universe_demean_keeps_ic_but_shifts_bucket_means():
    df = make_panel(13)
    plain = run_evaluate(df, ["f2"], quantiles=4, min_cs_count=4, tradable_col="tradable")
    demeaned = run_evaluate(
        df, ["f2"], quantiles=4, min_cs_count=4, tradable_col="tradable", demean="universe"
    )

    for a, b in zip(plain.ic.get_column("ic"), demeaned.ic.get_column("ic")):
        both_nan = (a is None or math.isnan(a)) and (b is None or math.isnan(b))
        assert both_nan or b == pytest.approx(a, abs=1e-10)
    # Bucket means differ (the day mean has been removed).
    a_means = plain.quantile_returns.get_column("mean_ret_1").drop_nans()
    b_means = demeaned.quantile_returns.get_column("mean_ret_1").drop_nans()
    assert any(
        abs(x - y) > 1e-12 for x, y in zip(a_means, b_means) if x is not None and y is not None
    )


def test_all_nan_day_yields_nan_ic_and_zero_coverage():
    df = make_panel(17, n_days=6).with_columns(
        pl.when(pl.col("time") == 3)
        .then(float("nan"))
        .otherwise(pl.col("f1"))
        .alias("f1")
    )
    result = run_evaluate(df, ["f1"], quantiles=2, min_cs_count=2)

    day3 = result.ic.filter(pl.col("date") == 3)
    assert all(math.isnan(v) for v in day3.get_column("ic"))
    cov = result.coverage.filter(pl.col("date") == 3)
    assert cov.get_column("n_valid").to_list() == [0]
    assert result.quantile_returns.filter(pl.col("date") == 3).height == 0


def test_min_cs_count_gates_small_cross_sections():
    df = make_panel(19, n_assets=4)
    result = run_evaluate(df, ["f2"], quantiles=2, min_cs_count=10)

    assert all(
        v is None or math.isnan(v) for v in result.ic.get_column("ic")
    )
    assert result.quantile_returns.height == 0
    row = result.summary.row(0, named=True)
    assert row["n_days"] == 0


# ---------------------------------------------------------------------------
# Result object: streaming, save, meta
# ---------------------------------------------------------------------------


def test_output_dir_streaming_matches_memory(tmp_path):
    df = make_panel(23)
    memory = run_evaluate(df, ["f1", "f2"], quantiles=4, min_cs_count=4)
    streamed = run_evaluate(
        df, ["f1", "f2"], quantiles=4, min_cs_count=4, output_dir=str(tmp_path / "run")
    )

    assert isinstance(streamed.ic, pl.LazyFrame)
    assert streamed.summary.equals(memory.summary)
    assert streamed.ic.collect().equals(memory.ic)
    assert streamed.quantile_returns.collect().equals(memory.quantile_returns)
    assert streamed.coverage.collect().equals(memory.coverage)
    assert (tmp_path / "run" / "meta.json").exists()

    with pytest.raises(ValueError, match="save\\(\\) is only available in memory mode"):
        streamed.save(str(tmp_path / "other"))


def test_save_roundtrip_and_meta(tmp_path):
    df = make_panel(29)
    result = run_evaluate(
        df, ["f1"], quantiles=4, min_cs_count=4, tradable_col="tradable"
    )
    result.save(str(tmp_path / "run"))

    reloaded = pl.read_parquet(tmp_path / "run" / "ic.parquet")
    assert reloaded.equals(result.ic)
    assert pl.read_parquet(tmp_path / "run" / "summary.parquet").equals(result.summary)

    meta = result.meta
    assert meta["quantiles"] == 4
    assert meta["horizons"] == [1, 5]
    assert meta["tradable_col"] == "tradable"
    assert meta["binning"] == "daily"
    assert "memory" in repr(result)


def test_monthly_table_for_date_column():
    from datetime import date, timedelta

    start = date(2026, 1, 20)
    rows = []
    rng = np.random.default_rng(31)
    for a in range(4):
        for t in range(20):
            rows.append(
                {
                    "asset": f"S{a}",
                    "time": start + timedelta(days=t),
                    "f1": float(rng.normal()),
                    "ret_1": float(rng.normal()),
                }
            )
    df = pl.DataFrame(rows)

    result = run_evaluate(df, ["f1"], quantiles=2, min_cs_count=3)

    monthly = result.ic_monthly
    assert monthly is not None
    assert set(zip(monthly.get_column("year"), monthly.get_column("month"))) == {
        (2026, 1),
        (2026, 2),
    }
    # Integer time column: no monthly table.
    assert run_evaluate(make_panel(1), ["f1"], min_cs_count=4).ic_monthly is None


def test_evaluate_rejects_bad_arguments():
    df = make_panel(37)
    with pytest.raises(ValueError, match="factor_cols"):
        run_evaluate(df, [])
    with pytest.raises(ValueError, match="binning"):
        run_evaluate(df, ["f1"], binning="weekly")
    with pytest.raises(ValueError, match="demean"):
        run_evaluate(df, ["f1"], demean="zscore")
    with pytest.raises(ValueError, match="group_col"):
        run_evaluate(df, ["f1"], demean="group")
    with pytest.raises(ValueError, match="quantiles"):
        run_evaluate(df, ["f1"], quantiles=1)
    with pytest.raises(ValueError, match="no label columns"):
        run_evaluate(df.drop(["ret_1", "ret_5"]), ["f1"])


# ---------------------------------------------------------------------------
# End-to-end: alphas -> labels -> evaluate
# ---------------------------------------------------------------------------


def test_alpha101_end_to_end():
    rows = []
    for asset_idx, asset in enumerate(["A", "B", "C", "D", "E"]):
        for t in range(1, 61):
            base = 10.0 * (asset_idx + 1) + t * 0.2
            close = base * (1.0 + ((t * (asset_idx + 3)) % 11 - 5) * 0.003)
            high = max(base, close) + 1.0
            low = min(base, close) - 1.0
            rows.append(
                {
                    "asset": asset,
                    "time": t,
                    "open": base,
                    "high": high,
                    "low": low,
                    "close": close,
                    "volume": 1_000.0 + asset_idx * 17.0 + t * 3.0,
                    "vwap": (high + low + close) / 3.0,
                    "cap": close * 1e6,
                    "sector": float(asset_idx % 2),
                    "industry": float(asset_idx % 2),
                    "subindustry": float(asset_idx % 2),
                }
            )
    df = pl.DataFrame(rows)

    df = qweave.with_alphas(
        df, symbol_col="asset", time_col="time", alphas=qweave.worldquant_alpha101({})
    )
    df = qweave.with_labels(df, symbol_col="asset", time_col="time", horizons=[1, 5])
    result = run_evaluate(
        df,
        [f"alpha{i}" for i in range(1, 102)],
        quantiles=4,
        min_cs_count=4,
    )

    assert result.summary.height == 101 * 2
    assert result.summary.get_column("factor").n_unique() == 101
    # At least some alphas produce usable ICs on this synthetic panel.
    assert result.summary.filter(pl.col("n_days") > 0).height > 0


def test_factor_source_matches_in_frame(tmp_path):
    # compute_alphas writes a factor panel; evaluate reads factors from it
    # instead of from the (label-only) frame.
    df = make_panel(71, nan_rate=0.0)
    alpha_out = tmp_path / "alphas.parquet"
    qweave.compute_alphas(
        df.select(["asset", "time", "f1", "f2"]).rename({"f1": "close", "f2": "open"}),
        symbol_col="asset",
        time_col="time",
        alphas=[
            qweave.col("close").alias("a1"),
            (qweave.col("close") - qweave.col("open")).alias("a2"),
        ],
        output_path=str(alpha_out),
    )

    # Build the same factor columns in-frame for the baseline.
    in_frame_df = df.with_columns(
        pl.col("f1").alias("a1"),
        (pl.col("f1") - pl.col("f2")).alias("a2"),
    )
    baseline = run_evaluate(in_frame_df, ["a1", "a2"], quantiles=4, min_cs_count=4)

    labels_only = df.drop(["f1", "f2"])
    sourced = qweave.evaluate(
        labels_only,
        symbol_col="asset",
        time_col="time",
        factor_cols=["a1", "a2"],
        quantiles=4,
        min_cs_count=4,
        factor_source=str(alpha_out),
    )

    assert sourced.summary.equals(baseline.summary)
    assert sourced.quantile_returns.equals(baseline.quantile_returns)
    assert sourced.meta["factor_source"] == str(alpha_out)

    # A mismatched panel is rejected.
    with pytest.raises(ValueError, match="does not match"):
        qweave.evaluate(
            labels_only.head(labels_only.height - 4),
            symbol_col="asset",
            time_col="time",
            factor_cols=["a1"],
            quantiles=4,
            min_cs_count=4,
            factor_source=str(alpha_out),
        )


def test_to_html_report(tmp_path):
    from datetime import date, timedelta

    start = date(2026, 1, 1)
    rows = []
    rng = np.random.default_rng(83)
    for a in range(6):
        for t in range(40):
            rows.append({
                "asset": f"S{a}", "time": start + timedelta(days=t),
                "f1": float(rng.normal()), "f2": float(rng.normal()),
                "ret_1": float(rng.normal() * 0.02),
            })
    df = pl.DataFrame(rows)
    result = run_evaluate(df, ["f1", "f2"], quantiles=4, min_cs_count=4)

    path = tmp_path / "report.html"
    result.to_html(str(path))
    html = path.read_text(encoding="utf-8")

    assert html.lstrip().lower().startswith("<!doctype html>")
    assert '"f1"' in html and '"f2"' in html
    # Self-contained: no external script/style/link fetches.
    assert 'src="http' not in html and "<link" not in html and "cdn" not in html
    # Monthly detail present because the time column is a date.
    assert '"monthly"' in html

    # Streamed results have no in-memory tables to render.
    streamed = run_evaluate(df, ["f1"], quantiles=4, min_cs_count=4,
                            output_dir=str(tmp_path / "run"))
    with pytest.raises(ValueError):
        streamed.to_html(str(tmp_path / "x.html"))
