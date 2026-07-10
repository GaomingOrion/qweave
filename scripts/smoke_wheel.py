"""Minimal smoke test for an installed qweave release wheel."""

import polars as pl
import qweave


frame = pl.DataFrame({"value": [1.0, 2.0]})
roundtripped = qweave.roundtrip(frame)

assert roundtripped.equals(frame)
assert qweave.col("value").collect_inputs() == {"value"}
print("qweave wheel smoke test passed")
