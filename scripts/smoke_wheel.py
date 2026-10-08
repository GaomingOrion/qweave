"""Minimal smoke test for an installed qweave release wheel."""

import math
import os

import polars as pl
import qweave


frame = pl.DataFrame({"value": [1.0, 2.0]})
roundtripped = qweave.roundtrip(frame)

assert roundtripped.equals(frame)
assert qweave.col("value").collect_inputs() == {"value"}

panel = pl.DataFrame({"asset": ["A"] * 4, "time": range(4),
                      "value": [None, 0.0, 2.0, 4.0]})
x = qweave.col("value")
expressions = [(x != qweave.lit(2.0)).alias("ne"),
               (x & qweave.lit(0.0)).alias("and"),
               (x | qweave.lit(1.0)).alias("or"), (~x).alias("not"),
               x.ts_mad(2).alias("mad"), x.ts_count(2).alias("count")]
expected = {"ne": [math.nan, 1.0, 0.0, 1.0], "and": [0.0] * 4,
            "or": [1.0] * 4, "not": [math.nan, 1.0, 0.0, 0.0],
            "mad": [math.nan, math.nan, 1.0, 1.0],
            "count": [math.nan, 1.0, 2.0, 2.0]}
for engine in ["tree", "dag"]:
    os.environ["QWEAVE_ENGINE"] = engine
    result = qweave.compute_alphas(panel, "asset", "time", expressions)
    for name, values in expected.items():
        assert result[name].dtype == pl.Float64
        for actual, target in zip(result[name], values):
            assert math.isnan(actual) if math.isnan(target) else actual == target
try:
    bool(x)
except TypeError:
    pass
else:
    raise AssertionError("Expressions must reject implicit Python truth testing")

print("qweave wheel smoke test passed")
