import importlib.util
import sys
from pathlib import Path

import polars as pl
import pytest


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "bench_factor_engines.py"
SPEC = importlib.util.spec_from_file_location("bench_factor_engines", SCRIPT)
bench = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
sys.modules[SPEC.name] = bench
SPEC.loader.exec_module(bench)


def test_build_ohlcv_panel_has_dense_factor_inputs():
    df = bench.build_ohlcv_panel(3, 7, seed=11)

    assert df.height == 21
    assert set(
        [
            "asset",
            "time",
            "open",
            "high",
            "low",
            "close",
            "volume",
            "vwap",
            "amount",
            "returns",
            "cap",
            "sector",
            "industry",
            "subindustry",
        ]
    ).issubset(df.columns)
    assert df.select(pl.col("asset").n_unique()).item() == 3
    assert df.select(pl.col("time").n_unique()).item() == 7


def test_panel_to_kunquant_inputs_are_time_stock_matrices():
    df = bench.build_ohlcv_panel(5, 6, seed=17)

    inputs = bench.panel_to_kunquant_inputs(df)

    assert sorted(inputs) == ["amount", "close", "high", "low", "open", "volume"]
    assert inputs["close"].shape == (6, 5)
    first_asset = df.filter(pl.col("asset") == "S00000").sort("time")
    assert inputs["close"][:, 0].tolist() == pytest.approx(first_asset.get_column("close").to_list())


def test_qlib_provider_writer_creates_minimal_binary_layout(tmp_path):
    df = bench.build_ohlcv_panel(2, 4, seed=19)

    instruments, start, end = bench.write_qlib_provider(df, tmp_path)

    assert instruments == ["S00000", "S00001"]
    assert start == "2000-01-01"
    assert end == "2000-01-04"
    assert (tmp_path / "calendars" / "day.txt").read_text(encoding="utf-8").splitlines() == [
        "2000-01-01",
        "2000-01-02",
        "2000-01-03",
        "2000-01-04",
    ]
    assert (tmp_path / "instruments" / "all.txt").exists()
    assert (tmp_path / "features" / "s00000" / "close.day.bin").stat().st_size == 5 * 4


def test_default_engines_follow_workload():
    assert bench.default_engines("alpha158") == ["qweave", "qlib"]
    assert bench.default_engines("worldquant101") == ["qweave", "kunquant"]


def test_benchmark_defaults_use_large_panel():
    args = bench.build_parser().parse_args([])

    assert args.symbols == 5000
    assert args.days == 1000


def test_qweave_sequential_runs_one_factor_at_a_time():
    df = bench.build_ohlcv_panel(4, 30, seed=23)

    out = bench.run_qweave_sequential(
        df,
        "worldquant101",
        names=["alpha13", "alpha101"],
    )

    assert out.columns == ["time", "asset", "alpha13", "alpha101"]
    assert out.height == df.height
    assert bench.process_peak_rss_mib() > 0
