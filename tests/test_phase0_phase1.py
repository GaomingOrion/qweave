import math

import polars as pl
import pytest
import qfactors


def test_roundtrip_dataframe():
    df = pl.DataFrame({"asset": ["A", "B"], "time": [1, 1], "close": [10.0, 20.0]})

    assert qfactors.roundtrip(df).equals(df)


def test_prepare_panel_sorts_and_adds_internal_columns():
    df = pl.DataFrame({"asset": ["B", "A", "A"], "time": [1, 2, 1], "close": [20.0, 11.0, 10.0]})

    panel = qfactors.prepare_panel(df, group_col="asset", time_col="time")
    out = panel.to_frame()

    assert panel.height == 3
    assert panel.group_count == 2
    assert out.get_column("asset").to_list() == ["A", "A", "B"]
    assert "__qfactors_group_id" in out.columns
    assert "__qfactors_time_ord" in out.columns


def test_float_null_to_nan():
    df = pl.DataFrame({"asset": ["A", "A"], "time": [1, 2], "close": [10.0, None]})

    panel = qfactors.prepare_panel(
        df,
        group_col="asset",
        time_col="time",
        null_policy="float_null_to_nan",
    )

    assert math.isnan(panel.to_frame().get_column("close").to_list()[1])


def test_compute_panel_ret_matches_python_baseline():
    df = _phase2_input_frame()
    panel = qfactors.prepare_panel(df, group_col="asset", time_col="time")

    out = panel.compute_panel(observation_times=[61, 60], factors=["ret"])

    assert out.columns == ["time", "asset", "ret"]
    assert out.select(["time", "asset"]).rows() == [
        (61, "A"),
        (61, "B"),
        (61, "C"),
        (60, "A"),
        (60, "B"),
        (60, "C"),
    ]

    values = out.get_column("ret").to_list()
    expected = [
        _ret_baseline(df, 61, "A", "open", "close"),
        _ret_baseline(df, 61, "B", "open", "close"),
        math.nan,
        _ret_baseline(df, 60, "A", "open", "close"),
        _ret_baseline(df, 60, "B", "open", "close"),
        math.nan,
    ]

    for actual, expected_value in zip(values, expected):
        if math.isnan(expected_value):
            assert math.isnan(actual)
        else:
            assert actual == pytest.approx(expected_value)


def test_compute_panel_ret_uses_column_aliases():
    df = _phase2_input_frame().rename({"open": "adj_open", "close": "adj_close"})
    panel = qfactors.prepare_panel(
        df,
        group_col="asset",
        time_col="time",
        column_aliases={"open": "adj_open", "close": "adj_close"},
    )

    out = panel.compute_panel(observation_times=pl.Series([60]), factors=["ret"])

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


def test_compute_panel_param_factor_matches_python_baseline():
    df = _phase2_input_frame()
    panel = qfactors.prepare_panel(df, group_col="asset", time_col="time")

    out = panel.compute_panel(
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
    panel = qfactors.prepare_panel(_phase2_input_frame(), group_col="asset", time_col="time")

    with pytest.raises(ValueError, match="factor `missing` is not known"):
        panel.compute_panel(observation_times=[60], factors=["missing"])


def test_compute_panel_file_mode_matches_memory(tmp_path):
    panel = qfactors.prepare_panel(_phase2_input_frame(), group_col="asset", time_col="time")
    output_path = tmp_path / "factor_panel.parquet"

    memory = panel.compute_panel(observation_times=[61, 60], factors=["ret"])
    summary = panel.compute_panel(
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
