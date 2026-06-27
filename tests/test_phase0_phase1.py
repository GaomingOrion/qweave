import math

import polars as pl
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
