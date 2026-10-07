"""Float64 mask truth tables and independent rolling-statistic oracles."""

import itertools
import math
import sys
from decimal import Decimal, localcontext

import numpy as np
import polars as pl
import pytest
import qweave as qw


@pytest.fixture(params=["tree", "dag"], autouse=True)
def engine(request, monkeypatch):
    monkeypatch.setenv("QWEAVE_ENGINE", request.param)


def evaluate(xs, exprs, ys=None):
    df = pl.DataFrame({"asset": pl.Series(["A"] * len(xs), dtype=pl.String),
                       "time": pl.Series(range(len(xs)), dtype=pl.Int64),
                       "x": pl.Series(xs, dtype=pl.Float64),
                       "y": pl.Series(xs if ys is None else ys, dtype=pl.Float64)})
    return qw.compute_alphas(df, "asset", "time", exprs)


# An unknown mask represents either Boolean value. Enumerating its possible
# outcomes gives an oracle independent of the implementation's branch order.
def possibilities(x):
    return [False, True] if x is None or math.isnan(x) else [x > 0]


def logic_reference(x, y, op):
    outcomes = {op(a, b) for a in possibilities(x) for b in possibilities(y)}
    return float(outcomes.pop()) if len(outcomes) == 1 else math.nan


def test_logic_truth_tables_and_fused_paths():
    masks = [0.0, -0.0, -3.0, -math.inf, 1.0, 2.5, 1e-300, math.inf, math.nan, None]
    pairs = list(itertools.product(masks, repeat=2))
    xs, ys = zip(*pairs)
    x, y = qw.col("x"), qw.col("y")
    out = evaluate(xs, [(x & y).alias("and"), (x | y).alias("or"),
                        (~x).alias("not"), (x != y).alias("ne"),
                        (~(x & y) | (x != y)).alias("fused")], ys)
    expected_and = [logic_reference(a, b, lambda p, q: p and q) for a, b in pairs]
    expected_or = [logic_reference(a, b, lambda p, q: p or q) for a, b in pairs]
    expected_not = [math.nan if len(possibilities(a)) == 2 else float(not (a > 0)) for a in xs]
    expected_ne = [math.nan if a is None or b is None or math.isnan(a) or math.isnan(b)
                   else float(a != b) for a, b in pairs]
    expected_fused = [logic_reference(math.nan if math.isnan(a) else 1-a, b,
                                      lambda p, q: p or q)
                      for a, b in zip(expected_and, expected_ne)]
    for name, expected in zip(["and", "or", "not", "ne", "fused"],
                              [expected_and, expected_or, expected_not, expected_ne, expected_fused]):
        assert out[name].dtype == pl.Float64
        np.testing.assert_array_equal(out[name].to_numpy(), expected)


@pytest.mark.parametrize("left,right", list(itertools.product([0.0, 2.0, math.nan], repeat=2)))
def test_constant_folding_and_scalar_vector_logic(left, right):
    x, y = qw.lit(left), qw.lit(right)
    exprs = [(x & y).alias("and"), (x | y).alias("or"), (~x).alias("not"),
             (x != y).alias("ne"), (qw.col("x") & y).alias("vector_and"),
             (x | qw.col("x")).alias("vector_or")]
    out = evaluate([left] * 3, exprs)
    expected = [logic_reference(left, right, lambda a, b: a and b),
                logic_reference(left, right, lambda a, b: a or b),
                math.nan if math.isnan(left) else float(left <= 0),
                math.nan if math.isnan(left) or math.isnan(right) else float(left != right)]
    for name, value in zip(["and", "or", "not", "ne", "vector_and", "vector_or"],
                           expected + [expected[0], logic_reference(left, left, lambda a, b: a or b)]):
        np.testing.assert_array_equal(out[name].to_numpy(), [value] * 3)


@pytest.mark.parametrize("source", ["bool(x)", "x and y", "x or y", "not x",
                                    "1 if x else 0", "x < y < x"])
def test_implicit_python_truth_is_rejected(source):
    with pytest.raises(TypeError, match="use &, \\|, and ~"):
        eval(source, {"x": qw.col("x"), "y": qw.col("y")})


def rolling_reference(values, days, statistic):
    result = [math.nan] * len(values)
    if days:
        for end in range(days, len(values) + 1):
            result[end - 1] = statistic(values[end-days:end])
    return result


def decimal_mad(window):
    if any(x is None or not math.isfinite(x) for x in window):
        return math.nan
    with localcontext() as ctx:
        ctx.prec = 1100
        xs = [Decimal.from_float(float(x)) for x in window]
        mean = sum(xs) / len(xs)
        return float(sum(abs(x - mean) for x in xs) / len(xs))


@pytest.mark.parametrize("days", [0, 1, 2, 7, 31, 301])
def test_count_matches_independent_windows(days):
    values = np.random.default_rng(5).normal(size=300).tolist()
    values[30:70] = [None] * 40
    values[110:115] = [math.nan, math.inf, -math.inf, 0.0, -0.0]
    expected = rolling_reference(values, days, lambda xs: float(sum(
        x is not None and not math.isnan(x) for x in xs)))
    actual = evaluate(values, [qw.col("x").ts_count(days).alias("out")])["out"]
    assert actual.dtype == pl.Float64
    np.testing.assert_array_equal(actual.to_numpy(), expected)


@pytest.mark.parametrize("values", [
    [7.0] * 30,
    [sys.float_info.max] * 30,
    [1e12 + x * 0.001 for x in [1, 5, 2, 7, 15, 6]] * 5,
    [x * 1e-200 for x in [1, 5, 2, 7, 15, 6]] * 5,
    [-sys.float_info.max, sys.float_info.max, 0.0, 5e307, -2e307, 1e307] * 5,
    [0.0, 5e-324, 1e-323, 0.0, -5e-324, -1e-323] * 5,
    [1.0, 2.0, None, 3.0, math.nan, 4.0, math.inf, 5.0, -math.inf] + list(map(float, range(21))),
    np.random.default_rng(19).normal(size=300).tolist(),
])
@pytest.mark.parametrize("days", [0, 1, 2, 5, 20, 301])
def test_mad_matches_high_precision_windows(values, days):
    expected = rolling_reference(values, days, decimal_mad)
    actual = evaluate(values, [qw.col("x").ts_mad(days).alias("out")])["out"]
    assert actual.dtype == pl.Float64
    np.testing.assert_allclose(actual.to_numpy(), expected, rtol=2e-14, atol=5e-324)


@pytest.mark.parametrize("op", ["ts_count", "ts_mad"])
def test_window_parameters_and_empty_input(op):
    method = getattr(qw.col("x"), op)
    with pytest.raises(OverflowError):
        method(-1)
    with pytest.raises(TypeError):
        method(1.5)
    assert evaluate([], [method(3).alias("out")]).height == 0


def test_logic_metadata_layouts_and_file_output(tmp_path):
    df = pl.DataFrame({"asset": ["A"] * 4 + ["B"] * 4, "time": [0, 1, 2, 3] * 2,
                       "price": [1.0, -1.0, None, 3.0, -2.0, 2.0, 4.0, None]})
    df = df.sample(fraction=1, shuffle=True, seed=13)
    x = qw.col("x")
    mask = (~(x <= qw.lit(0.0)) & (x != qw.lit(2.0))) | qw.lit(0.0)
    expr = mask.alias("mask")
    assert expr.collect_inputs() == {"x"}
    expr = expr.replace_inputs({"x": "price"})
    assert expr.collect_inputs() == {"price"}
    assert expr.output_name() == "mask"
    for name in ["and", "or", "not", "ne"]:
        assert name in repr(expr)
    # Cross-sectional -> elementwise -> rolling -> cross-sectional traverses
    # both layouts. Two copies also exercise shared DAG nodes.
    price = qw.col("price")
    ranked = price.rank()
    nested = ((ranked > qw.lit(0.5)) & ~(price < qw.lit(0.0))).ts_count(2).rank()
    exprs = [expr, (expr * price).alias("masked"), expr.ts_sum(2).alias("sum"),
             nested.alias("nested"), nested.alias("copy")]
    out = qw.compute_alphas(df, "asset", "time", exprs)
    expected = [1.0, 0.0, math.nan, 1.0, 0.0, 0.0, 1.0, math.nan]
    np.testing.assert_array_equal(out["mask"].to_numpy(), expected)
    np.testing.assert_array_equal(out["masked"].to_numpy(), [1, -0.0, math.nan, 3, -0.0, 0, 4, math.nan])
    np.testing.assert_array_equal(out["sum"].to_numpy(), [math.nan, 1, math.nan, math.nan, math.nan, 0, 1, math.nan])
    np.testing.assert_array_equal(out["nested"].to_numpy(), [math.nan, .75, .5, .75, math.nan, .75, 1, .75])
    np.testing.assert_array_equal(out["nested"].to_numpy(), out["copy"].to_numpy())
    attached = qw.with_alphas(df, "asset", "time", exprs)
    assert attached.select(df.columns).equals(df)
    assert attached.sort("asset", "time").select(out.columns).equals(out)
    path = tmp_path / "logic.parquet"
    qw.compute_alphas(df, "asset", "time", exprs, output_path=str(path))
    assert pl.read_parquet(path).sort("asset", "time").equals(out)
