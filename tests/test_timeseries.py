"""Independent numerical and public API checks for standard time-series ops."""

import json
import math
import sys
from decimal import Decimal, localcontext
from pathlib import Path

import numpy as np
import polars as pl
import pytest
import qweave as qw


OPS = ("ts_skew", "ts_kurt", "ema", "wma")
REFERENCE = json.loads(
    (Path(__file__).parent / "fixtures/moment_reference.json").read_text(encoding="utf-8")
)


def compute(values, op, days):
    df = pl.DataFrame({"asset": pl.Series(["A"] * len(values), dtype=pl.String),
                       "time": pl.Series(range(len(values)), dtype=pl.Int64),
                       "x": pl.Series(values, dtype=pl.Float64)})
    expr = getattr(qw.col("x"), op)(days).alias("out")
    return qw.compute_alphas(df, "asset", "time", [expr])["out"].to_numpy()


@pytest.fixture(params=["tree", "dag"], autouse=True)
def engine(request, monkeypatch):
    monkeypatch.setenv("QWEAVE_ENGINE", request.param)


@pytest.mark.parametrize("case", REFERENCE["cases"],
                         ids=lambda c: f'{c["name"]}-{c["days"]}')
@pytest.mark.parametrize("op", ["skew", "kurt"])
def test_moments_match_frozen_pandas(case, op):
    expected = np.array(case[op], dtype=float)
    np.testing.assert_allclose(compute(case["values"], "ts_" + op, case["days"]),
                               expected, rtol=1e-10, atol=1e-12, equal_nan=True)


def decimal_moment(values, kurtosis):
    """High-precision reference independent of the scaled Rust implementation."""
    with localcontext() as ctx:
        ctx.prec = 80
        xs = [Decimal.from_float(x) for x in values]
        n = Decimal(len(xs))
        mean = sum(xs) / n
        m2 = sum((x - mean) ** 2 for x in xs) / n
        if not m2:
            return -3.0 if kurtosis else 0.0
        if kurtosis:
            m4 = sum((x - mean) ** 4 for x in xs) / n
            return float((n - 1) * ((n + 1) * m4 / m2**2 - 3 * (n - 1))
                         / ((n - 2) * (n - 3)))
        m3 = sum((x - mean) ** 3 for x in xs) / n
        return float((n * (n - 1)).sqrt() * m3 / ((n - 2) * m2 * m2.sqrt()))


@pytest.mark.parametrize("values", [
    [1e12 + x * 0.001 for x in [1, 5, 2, 7, 15, 6]],
    [x * 1e-200 for x in [1, 5, 2, 7, 15, 6]],
    [-1e308, 1e308, 0.0, 5e307, -2e307, 1e307],
])
@pytest.mark.parametrize("op", ["ts_skew", "ts_kurt"])
def test_moments_stable_at_extreme_scales(values, op):
    actual = compute(values, op, len(values))[-1]
    assert actual == pytest.approx(decimal_moment(values, op == "ts_kurt"),
                                   rel=1e-12, abs=1e-12)


@pytest.mark.parametrize("op", OPS)
@pytest.mark.parametrize("days", [0, 1, 2, 3, 4, 20])
def test_window_boundaries_and_invalid_parameters(op, days):
    actual = compute([1.0, 2.0, 3.0, 4.0, 5.0], op, days)
    minimum = {"ts_skew": 3, "ts_kurt": 4, "ema": 1, "wma": 1}[op]
    if days < minimum or days > 5:
        assert np.isnan(actual).all()
    else:
        assert np.isnan(actual[:days - 1]).all()
        assert np.isfinite(actual[days - 1:]).all()
    method = getattr(qw.col("x"), op)
    with pytest.raises(OverflowError):
        method(-1)
    with pytest.raises(TypeError):
        method(2.5)


@pytest.mark.parametrize("days", [4, 20, 127])
@pytest.mark.parametrize("op", ["ts_skew", "ts_kurt"])
@pytest.mark.parametrize("scenario", ["random", "offset", "scale_change", "outlier", "constant", "missing", "decay"])
def test_incremental_moments_match_decimal_after_window_updates(days, op, scenario):
    rng = np.random.default_rng(42)
    values = rng.normal(size=2048)
    if scenario == "offset":
        values = 1e12 + values * 0.01
    elif scenario == "scale_change":
        values[:512] *= 1e-150
        values[512:1024] *= 1e150
        values[1024:] *= 1e-150
    elif scenario == "outlier":
        values[512] = 1e100
    elif scenario == "constant":
        values[512:1024] = 7.0
    elif scenario == "missing":
        values[512:514] = math.nan
        values[1024] = math.inf
        values[1536] = -math.inf
    elif scenario == "decay":
        values *= np.exp(-np.arange(len(values)) * 30 / days)
    actual = compute(values, op, days)
    # Cover repeated rebuild boundaries and the exact bars where exceptional
    # observations enter and leave the window, plus the end of a long stream.
    indices = set(range(days - 1, len(values), 113))
    for boundary in [days, 2 * days, 512, 513, 514, 1024, 1536, 2047]:
        indices.update(boundary + shift for shift in [-1, 0, 1, days - 1, days, days + 1])
    for i in sorted(indices):
        if i < days - 1 or i >= len(values):
            continue
        window = values[i-days+1:i+1]
        if not np.isfinite(window).all():
            assert math.isnan(actual[i]), (scenario, days, i)
        else:
            expected = decimal_moment(window, op == "ts_kurt")
            assert actual[i] == pytest.approx(expected, rel=1e-9, abs=1e-10), (scenario, days, i)


@pytest.mark.parametrize("days", [512, 2048])
@pytest.mark.parametrize("op", ["ts_skew", "ts_kurt"])
@pytest.mark.parametrize("scenario", ["offset", "trend", "decay"])
def test_incremental_moments_long_windows(days, op, scenario):
    values = np.random.default_rng(81).normal(size=5 * days)
    if scenario == "offset":
        values = 1e12 + values * 0.01
    elif scenario == "trend":
        values = np.arange(len(values)) * 0.01 + values * 0.001
    else:
        values *= np.exp(-np.arange(len(values)) * 30 / days)
    actual = compute(values, op, days)
    for i in np.linspace(days - 1, len(values) - 1, 35, dtype=int):
        expected = decimal_moment(values[i-days+1:i+1], op == "ts_kurt")
        assert actual[i] == pytest.approx(expected, rel=1e-9, abs=1e-10), (scenario, days, i)


@pytest.mark.parametrize("missing", [None, math.nan, math.inf, -math.inf])
def test_ema_initialization_and_restart(missing):
    values = [1.0, 2.0, 6.0, 4.0, missing, 8.0, 4.0, 9.0, 5.0]
    expected = [math.nan, math.nan, 3.0, 3.5, math.nan,
                math.nan, math.nan, 7.0, 6.0]
    np.testing.assert_allclose(compute(values, "ema", 3), expected, atol=1e-12)
    expected_identity = [math.nan if x is None or not math.isfinite(x) else x
                         for x in values]
    np.testing.assert_allclose(compute(values, "ema", 1), expected_identity)


def test_ema_one_preserves_finite_values_across_scale_changes():
    values = [1e16, 1.0, 2.0, -1e16, 1.0, 100.0, 1e-15,
              math.nan, math.inf, -math.inf, None, -0.0, 3.0]
    expected = np.array([math.nan if x is None or not math.isfinite(x) else x
                         for x in values])
    actual = compute(values, "ema", 1)
    np.testing.assert_array_equal(actual, expected)
    np.testing.assert_array_equal(np.signbit(actual[~np.isnan(expected)]),
                                  np.signbit(expected[~np.isnan(expected)]))


@pytest.mark.parametrize("days", [1, 2, 7, 20])
def test_ema_matches_independent_recurrence(days):
    values = np.random.default_rng(19).normal(size=100)
    values[[0, 24, 25, 60]] = math.nan
    expected = np.full(len(values), math.nan)
    start = 0
    for i, value in enumerate(values):
        if not math.isfinite(value):
            start = i + 1
        elif i - start + 1 == days:
            expected[i] = math.fsum(values[start:i+1]) / days
        elif i - start + 1 > days:
            expected[i] = expected[i-1] + 2 / (days+1) * (value - expected[i-1])
    np.testing.assert_allclose(compute(values, "ema", days), expected, atol=1e-12)


@pytest.mark.parametrize("days", [1, 3, 7, 20])
def test_ema_extreme_finite_inputs_do_not_overflow(days):
    largest = sys.float_info.max
    for value in [largest, -largest]:
        actual = compute([value] * 25, "ema", days)
        np.testing.assert_array_equal(actual[days-1:], value)
    values = [largest, -largest, largest, -largest]
    actual = compute(values, "ema", 2)
    assert np.isfinite(actual[1:]).all()
    assert actual[1] == 0.0
    assert actual[2] == pytest.approx(largest * (2/3))


@pytest.mark.parametrize("op", OPS)
def test_empty_input(op):
    assert compute([], op, 5).size == 0


@pytest.mark.parametrize("missing", [None, math.nan, math.inf, -math.inf])
@pytest.mark.parametrize("op", ["ts_skew", "ts_kurt"])
def test_moment_window_recovers_after_nonfinite(op, missing):
    values = [1.0, 2.0, missing, 3.0, 4.0, 5.0, 6.0]
    actual = compute(values, op, 4)
    assert np.isnan(actual[:-1]).all()
    assert actual[-1] == pytest.approx(0.0 if op == "ts_skew" else -1.2)


@pytest.mark.parametrize("days", [1, 3, 7])
def test_wma_matches_direct_weights_and_decay_linear(days):
    values = np.random.default_rng(7).normal(size=30)
    values[10] = math.nan
    expected = np.full(len(values), math.nan)
    weights = np.arange(1, days + 1)
    for i in range(days - 1, len(values)):
        expected[i] = np.dot(values[i-days+1:i+1], weights) / weights.sum()
    actual = compute(values, "wma", days)
    np.testing.assert_allclose(actual, expected, atol=1e-12)
    np.testing.assert_array_equal(actual, compute(values, "decay_linear", days))
    np.testing.assert_allclose(compute([1.0] * 6, "wma", 3)[2:], 1.0)


@pytest.mark.parametrize("op", OPS)
def test_symbols_sorting_metadata_nested_expressions_and_file_output(op, tmp_path):
    values = [1.0, 5.0, 2.0, 7.0, 15.0, 6.0]
    a = pl.DataFrame({"asset": ["A"] * 6, "time": range(6), "price": values})
    b = pl.DataFrame({"asset": ["B"] * 6, "time": range(6),
                      "price": list(reversed(values))})
    df = pl.concat([a, b]).sample(fraction=1, shuffle=True, seed=8)
    expr = getattr(qw.col("x"), op)(4).alias("result")
    assert expr.collect_inputs() == {"x"}
    expr = expr.replace_inputs({"x": "price"})
    assert expr.collect_inputs() == {"price"}
    assert expr.output_name() == "result"
    assert (op if op != "wma" else "decay_linear") in repr(expr)
    expected = np.concatenate([compute(values, op, 4), compute(values[::-1], op, 4)])
    out = qw.compute_alphas(df, "asset", "time", [expr])
    assert out.select("asset", "time").equals(df.select("asset", "time").sort("asset", "time"))
    np.testing.assert_allclose(out["result"].to_numpy(), expected)
    attached = qw.with_alphas(df, "asset", "time", [expr])
    assert attached.select(df.columns).equals(df)
    np.testing.assert_allclose(attached.sort("asset", "time")["result"].to_numpy(), expected)
    nested = (expr.rank().ts_mean(2) + qw.lit(1.0)).alias("nested")
    combined = qw.compute_alphas(df, "asset", "time", [expr, expr.alias("copy"), nested])
    np.testing.assert_array_equal(combined["result"].to_numpy(), combined["copy"].to_numpy())
    assert combined["nested"].is_finite().sum() > 0
    path = tmp_path / "alphas.parquet"
    qw.compute_alphas(df, "asset", "time", [expr], output_path=str(path))
    saved = pl.read_parquet(path).sort("asset", "time")
    np.testing.assert_allclose(saved["result"].to_numpy(), expected)
