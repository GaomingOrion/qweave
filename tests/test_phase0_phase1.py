import math

import polars as pl
import pytest
import qfactors


def test_roundtrip_dataframe():
    df = pl.DataFrame({"asset": ["A", "B"], "time": [1, 1], "close": [10.0, 20.0]})

    assert qfactors.roundtrip(df).equals(df)


def test_compute_panel_ignores_unused_null_columns():
    df = _phase2_input_frame()
    df = df.with_columns(pl.Series("unused", [None] * df.height))

    out = _compute_panel(df, observation_times=[60], factors=["ret"])

    assert out.select(["time", "asset"]).rows() == [(60, "A"), (60, "B")]


def test_float_null_to_nan():
    rows = []
    for time in range(1, 61):
        rows.append(
            {
                "asset": "A",
                "time": time,
                "open": None if time == 1 else float(time),
                "close": float(time + 1),
                "volume": 10.0,
            }
        )
    df = pl.DataFrame(rows)

    out = _compute_panel(df, observation_times=[60], factors=["ret"])

    assert math.isnan(out.get_column("ret").to_list()[0])


def test_compute_panel_ret_matches_python_baseline():
    df = _phase2_input_frame()

    out = _compute_panel(df, observation_times=[61, 60], factors=["ret"])

    assert out.columns == ["time", "asset", "ret"]
    assert out.select(["time", "asset"]).rows() == [
        (61, "A"),
        (61, "B"),
        (61, "C"),
        (60, "A"),
        (60, "B"),
    ]

    values = out.get_column("ret").to_list()
    expected = [
        _ret_baseline(df, 61, "A", "open", "close"),
        _ret_baseline(df, 61, "B", "open", "close"),
        math.nan,
        _ret_baseline(df, 60, "A", "open", "close"),
        _ret_baseline(df, 60, "B", "open", "close"),
    ]

    for actual, expected_value in zip(values, expected):
        if math.isnan(expected_value):
            assert math.isnan(actual)
        else:
            assert actual == pytest.approx(expected_value)


def test_compute_panel_ret_uses_column_aliases():
    df = _phase2_input_frame().rename({"open": "adj_open", "close": "adj_close"})

    out = _compute_panel(
        df,
        observation_times=pl.Series([60]),
        factors=["ret"],
        column_aliases={"open": "adj_open", "close": "adj_close"},
    )

    assert out.get_column("ret").to_list()[0] == pytest.approx(
        _ret_baseline(df, 60, "A", "adj_open", "adj_close")
    )


def test_factor_catalog_contains_registered_ret_and_is_filterable():
    catalog = qfactors.factor_catalog()

    row = catalog.filter(pl.col("factor_name") == "ret").row(0, named=True)
    assert row["kernel_name"] == "ret"
    assert row["window"] == 60
    assert row["input_names"] == ["open", "close"]
    assert row["output_columns"] == ["ret"]

    selected = (
        catalog.filter(pl.col("kernel_name") == "ret")
        .filter(pl.col("window") == 60)
        .get_column("factor_name")
        .to_list()
    )
    assert selected == ["ret"]


def test_factor_catalog_contains_param_factor_and_is_filterable():
    catalog = qfactors.factor_catalog()

    row = catalog.filter(pl.col("factor_name") == "volume_breakout_20_k15").row(0, named=True)
    assert row["kernel_name"] == "volume_breakout"
    assert row["window"] == 20
    assert row["input_names"] == ["volume"]
    assert row["param_set"] == "k15"
    assert row["param_k"] == pytest.approx(1.5)

    selected = (
        catalog.filter(pl.col("kernel_name") == "volume_breakout")
        .filter(pl.col("window") == 20)
        .filter(pl.col("param_k") == 1.5)
        .get_column("factor_name")
        .to_list()
    )
    assert selected == ["volume_breakout_20_k15"]


def test_expression_collect_inputs_replace_inputs_and_alias():
    expr = ((qfactors.col("close") + qfactors.col("open")) / qfactors.lit(2.0)).alias("mid")

    assert expr.collect_inputs() == {"close", "open"}

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


def test_worldquant101_alphas_returns_expression_subset_with_aliases():
    selected = qfactors.worldquant101_alphas({}, alphas=["alpha13", "alpha101"])
    catalog = qfactors._worldquant101_alphas()

    assert len(selected) == 2
    assert set(catalog) == {f"alpha{idx}" for idx in range(1, 102)}
    assert selected[0].collect_inputs() == {"close", "volume"}
    assert "subindustry" in catalog["alpha100"].collect_inputs()


def test_worldquant101_alphas_rejects_non_worldquant_names():
    with pytest.raises(ValueError, match="factor `group_returns_rank` is not known"):
        qfactors.worldquant101_alphas({}, alphas=["group_returns_rank"])
    with pytest.raises(ValueError, match="factor `alpha102` is not known"):
        qfactors.worldquant101_alphas({}, alphas=["alpha102"])


def test_compute_panel_param_factor_matches_python_baseline():
    df = _phase2_input_frame()

    out = _compute_panel(
        df,
        observation_times=[61],
        factors=["volume_breakout_20_k15", "volume_breakout_60_k15"],
    )

    for factor_name, window, k in [
        ("volume_breakout_20_k15", 20, 1.5),
        ("volume_breakout_60_k15", 60, 1.5),
    ]:
        values = out.get_column(factor_name).to_list()
        expected = [
            _volume_breakout_baseline(df, 61, "A", window, k),
            _volume_breakout_baseline(df, 61, "B", window, k),
            math.nan,
        ]
        for actual, expected_value in zip(values, expected):
            if math.isnan(expected_value):
                assert math.isnan(actual)
            else:
                assert actual == pytest.approx(expected_value)


def test_compute_panel_rejects_unknown_factor():
    with pytest.raises(ValueError, match="factor `missing` is not known"):
        _compute_panel(_phase2_input_frame(), observation_times=[60], factors=["missing"])


def test_compute_panel_file_mode_matches_memory(tmp_path):
    df = _phase2_input_frame()
    output_path = tmp_path / "factor_panel.parquet"

    memory = _compute_panel(df, observation_times=[61, 60], factors=["ret"])
    summary = _compute_panel(
        df,
        observation_times=[61, 60],
        factors=["ret"],
        output_path=str(output_path),
    )
    file_out = pl.read_parquet(output_path)

    assert summary == {
        "output_path": str(output_path),
        "n_observations": 2,
        "n_rows": memory.height,
    }
    assert file_out.columns == memory.columns
    assert file_out.select(["time", "asset"]).rows() == memory.select(["time", "asset"]).rows()
    for actual, expected in zip(file_out.get_column("ret").to_list(), memory.get_column("ret").to_list()):
        if math.isnan(expected):
            assert math.isnan(actual)
        else:
            assert actual == pytest.approx(expected)


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


def test_compute_alphas_worldquant101_representative_extra_fields_smoke():
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


def test_compute_panel_missing_observation_time_outputs_empty_frame(tmp_path):
    df = _phase2_input_frame()
    output_path = tmp_path / "empty_factor_panel.parquet"

    memory = _compute_panel(df, observation_times=[999], factors=["ret"])
    summary = _compute_panel(
        df,
        observation_times=[999],
        factors=["ret"],
        output_path=str(output_path),
    )
    file_out = pl.read_parquet(output_path)

    assert memory.columns == ["time", "asset", "ret"]
    assert memory.height == 0
    assert summary == {
        "output_path": str(output_path),
        "n_observations": 1,
        "n_rows": 0,
    }
    assert file_out.columns == memory.columns
    assert file_out.height == 0


def _compute_panel(df, observation_times, factors, column_aliases=None, output_path=None):
    return qfactors.compute_panel(
        df,
        symbol_col="asset",
        time_col="time",
        factors=factors,
        observation_times=observation_times,
        column_aliases=column_aliases,
        output_path=output_path,
    )


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
    return qfactors.worldquant101_alphas({}, alphas=names)


def _phase2_input_frame():
    rows = []
    for asset, multiplier in [("A", 1.0), ("B", 2.0)]:
        for time in range(1, 62):
            rows.append(
                {
                    "asset": asset,
                    "time": time,
                    "open": multiplier * time,
                    "close": multiplier * (time + 1),
                    "volume": 100.0 if asset == "A" and time == 61 else 10.0,
                }
            )
    rows.append({"asset": "C", "time": 61, "open": 100.0, "close": 110.0, "volume": 100.0})
    return pl.DataFrame(rows)


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


def _ret_baseline(df, observation_time, asset, open_col, close_col):
    window = (
        df.filter((pl.col("asset") == asset) & (pl.col("time") <= observation_time))
        .sort("time")
        .tail(60)
    )
    if window.height < 60:
        return math.nan
    return window.get_column(close_col)[-1] / window.get_column(open_col)[0] - 1.0


def _volume_breakout_baseline(df, observation_time, asset, window, k):
    window_df = (
        df.filter((pl.col("asset") == asset) & (pl.col("time") <= observation_time))
        .sort("time")
        .tail(window)
    )
    if window_df.height < window:
        return math.nan

    volume = window_df.get_column("volume").to_list()
    return 1.0 if volume[-1] > k * (sum(volume) / len(volume)) else 0.0


def _alpha101_baseline(df, observation_time, asset):
    row = df.filter((pl.col("asset") == asset) & (pl.col("time") == observation_time)).row(
        0, named=True
    )
    return (row["close"] - row["open"]) / (row["high"] - row["low"] + 0.001)
