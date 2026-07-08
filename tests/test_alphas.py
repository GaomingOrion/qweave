import math
import threading
import time

import numpy as np
import polars as pl
import pytest
import qfactors


def test_roundtrip_dataframe():
    df = pl.DataFrame({"asset": ["A", "B"], "time": [1, 1], "close": [10.0, 20.0]})

    assert qfactors.roundtrip(df).equals(df)


def test_expression_collect_inputs_replace_inputs_alias_and_output_name():
    expr = ((qfactors.col("close") + qfactors.col("open")) / qfactors.lit(2.0)).alias("mid")

    assert expr.collect_inputs() == {"close", "open"}
    assert expr.output_name() == "mid"

    remapped = expr.replace_inputs({"close": "adj_close", "open": "adj_open"})
    df = pl.DataFrame(
        {
            "asset": ["A"],
            "time": [1],
            "adj_close": [12.0],
            "adj_open": [10.0],
        }
    )
    out = _with_alphas(df, [remapped])

    assert out.columns == ["asset", "time", "adj_close", "adj_open", "mid"]
    assert out.get_column("mid").to_list() == [11.0]


def test_alpha_executors_do_not_accept_column_aliases():
    df = _alpha_input_frame()
    expr = qfactors.col("close").alias("close_copy")

    with pytest.raises(TypeError, match="column_aliases"):
        qfactors.compute_alphas(
            df,
            symbol_col="asset",
            time_col="time",
            alphas=[expr],
            column_aliases={"close": "adj_close"},
        )
    with pytest.raises(TypeError, match="column_aliases"):
        qfactors.with_alphas(
            df,
            symbol_col="asset",
            time_col="time",
            alphas=[expr],
            column_aliases={"close": "adj_close"},
        )


def test_worldquant_alpha101_returns_expression_subset_with_aliases():
    selected = qfactors.worldquant_alpha101({}, alphas=["alpha13", "alpha101"])
    all_exprs = qfactors.worldquant_alpha101({})

    assert [expr.output_name() for expr in selected] == ["alpha13", "alpha101"]
    assert len(all_exprs) == 101
    assert {expr.output_name() for expr in all_exprs} == {f"alpha{idx}" for idx in range(1, 102)}
    assert selected[0].collect_inputs() == {"close", "volume"}
    assert "subindustry" in [
        expr for expr in all_exprs if expr.output_name() == "alpha100"
    ][0].collect_inputs()


def test_worldquant_alpha101_rejects_non_worldquant_names():
    with pytest.raises(ValueError, match="factor `group_returns_rank` is not known"):
        qfactors.worldquant_alpha101({}, alphas=["group_returns_rank"])
    with pytest.raises(ValueError, match="factor `alpha102` is not known"):
        qfactors.worldquant_alpha101({}, alphas=["alpha102"])


def test_qlib_alpha158_returns_subset_with_aliases_and_output_name_filtering():
    aliases = {
        "open": "o",
        "high": "h",
        "low": "l",
        "close": "c",
        "volume": "vol",
        "vwap": "vw",
    }
    selected = qfactors.qlib_alpha158(aliases, alphas=["KMID", "ROC5", "BETA5", "QTLU5"])
    all_exprs = qfactors.qlib_alpha158(aliases)
    wanted = {"KMID", "ROC5", "BETA5"}
    filtered = [expr for expr in all_exprs if expr.output_name() in wanted]

    assert [expr.output_name() for expr in selected] == ["KMID", "ROC5", "BETA5", "QTLU5"]
    assert len(all_exprs) == 158
    assert selected[0].collect_inputs() == {"c", "o"}
    assert [expr.output_name() for expr in filtered] == ["KMID", "ROC5", "BETA5"]

    df, _ = _qlib_input_frame(n_times=8)
    aliased_df = df.rename(
        {
            "open": "o",
            "high": "h",
            "low": "l",
            "close": "c",
            "volume": "vol",
            "vwap": "vw",
        }
    )
    out = _compute_alphas(aliased_df, alphas=filtered)
    appended = _with_alphas(aliased_df, alphas=filtered[:2])

    assert out.columns == ["time", "asset", "KMID", "ROC5", "BETA5"]
    assert appended.columns[-2:] == ["KMID", "ROC5"]
    assert out.height == aliased_df.height
    assert appended.height == aliased_df.height


def test_qlib_alpha158_rejects_unknown_names():
    with pytest.raises(ValueError, match="factor `alpha101` is not known"):
        qfactors.qlib_alpha158({}, alphas=["alpha101"])
    with pytest.raises(ValueError, match="factor `NOT_A_FACTOR` is not known"):
        qfactors.qlib_alpha158({}, alphas=["NOT_A_FACTOR"])


def test_compute_alphas_alpha101_matches_python_baseline():
    df = _alpha_input_frame()

    out = _compute_alphas(df, alphas=_alpha_exprs(["alpha101"]))

    assert out.columns == ["time", "asset", "alpha101"]
    assert out.select(["time", "asset"]).rows() == [
        (1, "A"),
        (1, "B"),
        (2, "A"),
        (2, "B"),
    ]
    expected = [
        _alpha101_baseline(df, 1, "A"),
        _alpha101_baseline(df, 1, "B"),
        _alpha101_baseline(df, 2, "A"),
        _alpha101_baseline(df, 2, "B"),
    ]
    for actual, expected_value in zip(out.get_column("alpha101").to_list(), expected):
        assert actual == pytest.approx(expected_value)


def test_compute_alphas_file_mode_matches_memory(tmp_path):
    df = _alpha_input_frame()
    output_path = tmp_path / "alpha_panel.parquet"

    alphas = _alpha_exprs(["alpha101"])
    memory = _compute_alphas(df, alphas=alphas)
    summary = _compute_alphas(
        df,
        alphas=alphas,
        output_path=str(output_path),
    )
    file_out = pl.read_parquet(output_path)

    assert summary == {
        "output_path": str(output_path),
        "n_observations": 1,
        "n_rows": memory.height,
    }
    assert file_out.equals(memory)


def test_compute_alphas_releases_gil_while_running():
    df = _worldquant_input_frame(n_times=50_000)
    alphas = qfactors.worldquant_alpha101({})
    started = threading.Event()
    done = threading.Event()
    errors = []

    def worker():
        try:
            started.set()
            _compute_alphas(df, alphas=alphas)
        except BaseException as exc:
            errors.append(exc)
        finally:
            done.set()

    thread = threading.Thread(target=worker, daemon=True)
    thread.start()

    assert started.wait(timeout=2.0)
    if done.wait(timeout=0.01):
        thread.join()
        if errors:
            raise errors[0]
        pytest.fail("compute_alphas finished before the GIL smoke test could observe it")

    deadline = time.perf_counter() + 0.05
    ticks = 0
    while time.perf_counter() < deadline:
        ticks += 1
    assert ticks > 0

    thread.join(timeout=30.0)
    assert not thread.is_alive()
    if errors:
        raise errors[0]


def test_with_alphas_mixes_custom_and_worldquant_exprs_in_original_order():
    df = _alpha_input_frame()
    custom = (
        (qfactors.col("close") - qfactors.col("open"))
        / (qfactors.col("high") - qfactors.col("low") + qfactors.lit(0.001))
    ).alias("custom")

    out = _with_alphas(df, [custom, *_alpha_exprs(["alpha101"])])

    assert out.select(["time", "asset"]).rows() == df.select(["time", "asset"]).rows()
    assert out.columns == [
        "asset",
        "time",
        "open",
        "close",
        "high",
        "low",
        "volume",
        "industry",
        "custom",
        "alpha101",
    ]
    for actual, expected in zip(out.get_column("custom").to_list(), out.get_column("alpha101").to_list()):
        assert actual == pytest.approx(expected)


def test_compute_alphas_worldquant_alpha101_representative_extra_fields_smoke():
    df = _worldquant_input_frame(n_times=40)

    out = _compute_alphas(
        df,
        alphas=_alpha_exprs(["alpha5", "alpha56", "alpha58", "alpha80"]),
    )

    assert out.columns == ["time", "asset", "alpha5", "alpha56", "alpha58", "alpha80"]
    assert out.filter(pl.col("time") == 40).select(["time", "asset"]).rows() == [
        (40, "A"),
        (40, "B"),
        (40, "C"),
        (40, "D"),
    ]


def test_new_rolling_methods_compute_expected_values():
    df = pl.DataFrame(
        {
            "asset": ["A"] * 5,
            "time": [1, 2, 3, 4, 5],
            "close": [1.0, 3.0, 5.0, 7.0, 8.0],
        }
    )
    alphas = [
        qfactors.col("close").slope(3).alias("slope"),
        qfactors.col("close").rsquare(3).alias("rsquare"),
        qfactors.col("close").resi(3).alias("resi"),
        qfactors.col("close").quantile(3, 0.8).alias("q80"),
    ]

    out = _compute_alphas(df, alphas=alphas)

    assert _nan_prefix(out.get_column("slope").to_list(), [2.0, 2.0, 1.5])
    assert _nan_prefix(out.get_column("rsquare").to_list(), [1.0, 1.0, 9.0 / (2.0 * (14.0 / 3.0))])
    assert _nan_prefix(out.get_column("resi").to_list(), [0.0, 0.0, -1.0 / 6.0])
    assert _nan_prefix(out.get_column("q80").to_list(), [4.2, 6.2, 7.6])


def test_qlib_alpha158_representatives_match_numpy_reference():
    df, arrays = _qlib_input_frame(n_times=12)
    names = [
        "KMID",
        "VWAP0",
        "ROC5",
        "MA5",
        "STD5",
        "BETA5",
        "RSQR5",
        "RESI5",
        "MAX5",
        "QTLU5",
        "RANK5",
        "RSV5",
        "IMAX5",
        "CORR5",
        "CNTP5",
        "SUMP5",
        "WVMA5",
        "VSUMD5",
    ]

    out = _compute_alphas(df, alphas=qfactors.qlib_alpha158({}, alphas=names))
    expected = _qlib_reference(arrays)
    asset_index = {asset: idx for idx, asset in enumerate(arrays["assets"])}
    rows = out.select(["time", "asset"]).rows()

    for name in names:
        for actual, (time, asset) in zip(out.get_column(name).to_list(), rows):
            expected_value = expected[name][asset_index[asset], time - 1]
            _assert_float_matches(actual, expected_value, name, asset, time)


def _compute_alphas(df, alphas, output_path=None):
    return qfactors.compute_alphas(
        df,
        symbol_col="asset",
        time_col="time",
        alphas=alphas,
        output_path=output_path,
    )


def _with_alphas(df, alphas):
    return qfactors.with_alphas(
        df,
        symbol_col="asset",
        time_col="time",
        alphas=alphas,
    )


def _alpha_exprs(names):
    return qfactors.worldquant_alpha101({}, alphas=names)


def _qlib_input_frame(n_times=12):
    assets = ["A", "B", "C"]
    arrays = {
        "assets": assets,
        "open": np.zeros((len(assets), n_times), dtype=float),
        "high": np.zeros((len(assets), n_times), dtype=float),
        "low": np.zeros((len(assets), n_times), dtype=float),
        "close": np.zeros((len(assets), n_times), dtype=float),
        "volume": np.zeros((len(assets), n_times), dtype=float),
        "vwap": np.zeros((len(assets), n_times), dtype=float),
    }
    rows = []
    for asset_idx, asset in enumerate(assets):
        for time_idx in range(n_times):
            day = time_idx + 1
            trend = 20.0 + asset_idx * 7.0 + day * 0.9
            close = trend + (((day * (asset_idx + 2)) % 7) - 3) * 0.25
            open_ = trend - 0.35 + ((day + asset_idx) % 4) * 0.18
            high = max(open_, close) + 0.55 + asset_idx * 0.03
            low = min(open_, close) - 0.65 - day * 0.01
            volume = (
                1000.0
                + asset_idx * 90.0
                + day * 7.0
                + (((day * (asset_idx + 3)) % 9) - 4) * 15.0
            )
            vwap = (open_ + high + low + close) / 4.0

            for field, value in [
                ("open", open_),
                ("high", high),
                ("low", low),
                ("close", close),
                ("volume", volume),
                ("vwap", vwap),
            ]:
                arrays[field][asset_idx, time_idx] = value

            rows.append(
                {
                    "asset": asset,
                    "time": day,
                    "open": open_,
                    "high": high,
                    "low": low,
                    "close": close,
                    "volume": volume,
                    "vwap": vwap,
                }
            )
    return pl.DataFrame(rows), arrays


def _alpha_input_frame():
    return pl.DataFrame(
        [
            {
                "asset": "B",
                "time": 2,
                "open": 21.0,
                "close": 24.0,
                "high": 25.0,
                "low": 20.0,
                "volume": 110.0,
                "industry": 1.0,
            },
            {
                "asset": "A",
                "time": 1,
                "open": 10.0,
                "close": 11.0,
                "high": 12.0,
                "low": 9.0,
                "volume": 100.0,
                "industry": 0.0,
            },
            {
                "asset": "A",
                "time": 2,
                "open": 12.0,
                "close": 15.0,
                "high": 16.0,
                "low": 11.0,
                "volume": 120.0,
                "industry": 0.0,
            },
            {
                "asset": "B",
                "time": 1,
                "open": 20.0,
                "close": 21.0,
                "high": 22.0,
                "low": 19.0,
                "volume": 90.0,
                "industry": 1.0,
            },
        ]
    )


def _worldquant_input_frame(n_times):
    rows = []
    for asset_idx, asset in enumerate(["A", "B", "C", "D"]):
        for time in range(1, n_times + 1):
            base = 10.0 * (asset_idx + 1) + time * 0.2
            close = base * (1.0 + ((time % 7) - 3) * 0.001)
            high = max(base, close) + 1.0 + asset_idx * 0.01
            low = min(base, close) - 1.0
            rows.append(
                {
                    "asset": asset,
                    "time": time,
                    "open": base,
                    "high": high,
                    "low": low,
                    "close": close,
                    "volume": 1_000.0 + asset_idx * 17.0 + time * 3.0,
                    "vwap": (high + low + close) / 3.0,
                    "cap": close * (1_000_000.0 + asset_idx * 100_000.0),
                    "sector": float(asset_idx % 2),
                    "industry": float(asset_idx % 2),
                    "subindustry": float(asset_idx % 2),
                }
            )
    return pl.DataFrame(rows)


def _alpha101_baseline(df, observation_time, asset):
    row = df.filter((pl.col("asset") == asset) & (pl.col("time") == observation_time)).row(
        0, named=True
    )
    return (row["close"] - row["open"]) / (row["high"] - row["low"] + 0.001)


def _nan_prefix(values, expected_tail):
    if not all(math.isnan(value) for value in values[:2]):
        return False
    for actual, expected in zip(values[2:], expected_tail):
        if actual != pytest.approx(expected):
            return False
    return True


def _qlib_reference(arrays):
    eps = 1e-12
    d = 5
    c = arrays["close"]
    o = arrays["open"]
    h = arrays["high"]
    l = arrays["low"]
    v = arrays["volume"]
    vw = arrays["vwap"]

    close_delta = c - _delay(c, 1)
    volume_delta = v - _delay(v, 1)
    sump_num = _rolling_apply(np.maximum(close_delta, 0.0), d, np.sum)
    sumn_num = _rolling_apply(np.maximum(-close_delta, 0.0), d, np.sum)
    sum_denom = _rolling_apply(np.abs(close_delta), d, np.sum) + eps
    vsump_num = _rolling_apply(np.maximum(volume_delta, 0.0), d, np.sum)
    vsumn_num = _rolling_apply(np.maximum(-volume_delta, 0.0), d, np.sum)
    vsum_denom = _rolling_apply(np.abs(volume_delta), d, np.sum) + eps
    wvma_base = np.abs(c / _delay(c, 1) - 1.0) * v

    with np.errstate(divide="ignore", invalid="ignore"):
        return {
            "KMID": (c - o) / o,
            "VWAP0": vw / c,
            "ROC5": _delay(c, d) / c,
            "MA5": _rolling_apply(c, d, np.mean) / c,
            "STD5": _rolling_apply(c, d, _std_window) / c,
            "BETA5": _rolling_apply(c, d, _slope_window) / c,
            "RSQR5": _rolling_apply(c, d, _rsquare_window),
            "RESI5": _rolling_apply(c, d, _resi_window) / c,
            "MAX5": _rolling_apply(h, d, np.max) / c,
            "QTLU5": _rolling_apply(c, d, lambda window: _quantile_window(window, 0.8)) / c,
            "RANK5": _rolling_apply(c, d, _rank_last),
            "RSV5": (c - _rolling_apply(l, d, np.min))
            / (_rolling_apply(h, d, np.max) - _rolling_apply(l, d, np.min) + eps),
            "IMAX5": (_rolling_apply(h, d, lambda window: float(np.argmax(window))) + 1.0) / d,
            "CORR5": _rolling_apply2(c, np.log(v + 1.0), d, _corr_window),
            "CNTP5": _rolling_apply(_cmp_gt(c, _delay(c, 1)), d, np.mean),
            "SUMP5": sump_num / sum_denom,
            "WVMA5": _rolling_apply(wvma_base, d, _std_window)
            / (_rolling_apply(wvma_base, d, np.mean) + eps),
            "VSUMD5": (vsump_num - vsumn_num) / vsum_denom,
        }


def _delay(values, days):
    out = np.full(values.shape, np.nan, dtype=float)
    if days == 0:
        return values.copy()
    out[:, days:] = values[:, :-days]
    return out


def _rolling_apply(values, days, reducer):
    out = np.full(values.shape, np.nan, dtype=float)
    for asset_idx in range(values.shape[0]):
        for time_idx in range(days - 1, values.shape[1]):
            window = values[asset_idx, time_idx + 1 - days : time_idx + 1]
            if not np.isnan(window).any():
                out[asset_idx, time_idx] = reducer(window)
    return out


def _rolling_apply2(lhs, rhs, days, reducer):
    out = np.full(lhs.shape, np.nan, dtype=float)
    for asset_idx in range(lhs.shape[0]):
        for time_idx in range(days - 1, lhs.shape[1]):
            lhs_window = lhs[asset_idx, time_idx + 1 - days : time_idx + 1]
            rhs_window = rhs[asset_idx, time_idx + 1 - days : time_idx + 1]
            if not np.isnan(lhs_window).any() and not np.isnan(rhs_window).any():
                out[asset_idx, time_idx] = reducer(lhs_window, rhs_window)
    return out


def _std_window(window):
    return float(np.std(window, ddof=1)) if len(window) >= 2 else math.nan


def _slope_window(window):
    parts = _regression_parts(window)
    if parts is None:
        return math.nan
    sxy, sxx, _, _ = parts
    return sxy / sxx


def _rsquare_window(window):
    parts = _regression_parts(window)
    if parts is None:
        return math.nan
    sxy, sxx, syy, _ = parts
    return math.nan if syy == 0.0 else sxy * sxy / (sxx * syy)


def _resi_window(window):
    parts = _regression_parts(window)
    if parts is None:
        return math.nan
    sxy, sxx, _, y_mean = parts
    n = float(len(window))
    x_mean = (n - 1.0) / 2.0
    slope = sxy / sxx
    return window[-1] - y_mean - slope * (n - 1.0 - x_mean)


def _regression_parts(window):
    n = len(window)
    if n < 2:
        return None
    n_f = float(n)
    x_mean = (n_f - 1.0) / 2.0
    y_mean = float(np.mean(window))
    sxx = n_f * (n_f * n_f - 1.0) / 12.0
    sxy = sum(idx * value for idx, value in enumerate(window)) - n_f * x_mean * y_mean
    syy = sum((value - y_mean) * (value - y_mean) for value in window)
    return sxy, sxx, syy, y_mean


def _quantile_window(window, q):
    sorted_window = np.sort(window)
    pos = q * (len(sorted_window) - 1)
    lo = int(math.floor(pos))
    hi = int(math.ceil(pos))
    if lo == hi:
        return float(sorted_window[lo])
    frac = pos - lo
    return float(sorted_window[lo] + frac * (sorted_window[hi] - sorted_window[lo]))


def _rank_last(window):
    target = window[-1]
    less = int(np.count_nonzero(window < target))
    equal = int(np.count_nonzero(window == target))
    return (less + 1 + less + equal) / 2.0 / len(window)


def _corr_window(lhs, rhs):
    if len(lhs) < 2:
        return math.nan
    lhs_centered = lhs - np.mean(lhs)
    rhs_centered = rhs - np.mean(rhs)
    lhs_var = float(np.dot(lhs_centered, lhs_centered))
    rhs_var = float(np.dot(rhs_centered, rhs_centered))
    if lhs_var == 0.0 or rhs_var == 0.0:
        return math.nan
    return float(np.dot(lhs_centered, rhs_centered) / math.sqrt(lhs_var * rhs_var))


def _cmp_gt(lhs, rhs):
    out = np.full(lhs.shape, np.nan, dtype=float)
    mask = ~np.isnan(lhs) & ~np.isnan(rhs)
    out[mask] = (lhs[mask] > rhs[mask]).astype(float)
    return out


def _assert_float_matches(actual, expected, name, asset, time):
    if actual is None:
        actual = math.nan
    expected = float(expected)
    if math.isnan(expected):
        assert math.isnan(actual), f"{name} {asset} time={time}: actual={actual}, expected=NaN"
    else:
        assert actual == pytest.approx(expected, rel=1e-10, abs=1e-10), (
            f"{name} {asset} time={time}: actual={actual}, expected={expected}"
        )
