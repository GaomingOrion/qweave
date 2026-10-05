"""Regenerate synthetic moment fixtures with a pinned pandas oracle.

Run from the repository root in PowerShell:
uv run --no-project --with numpy==2.2.6 --with pandas==2.2.3 python scripts/generate_moment_reference.py

Writes tests/fixtures/moment_reference.json. pandas is only needed for fixture
generation, not for qweave or its normal test suite.
"""

import json
import math
from pathlib import Path

import numpy as np
import pandas as pd


def main():
    """Write deterministic full-window skew/kurt references.

    Takes no arguments and returns None. Raises RuntimeError if the oracle
    versions differ, or OSError if the fixture cannot be written.
    """
    if (np.__version__, pd.__version__) != ("2.2.6", "2.2.3"):
        raise RuntimeError("Use numpy==2.2.6 and pandas==2.2.3")
    inputs = {
        "asymmetric": [1, 5, 2, 7, 15, 6, -3, 0, 2, 9, 4, 1],
        "constant": [7] * 12,
        "missing": [None, 1, 5, 2, 7, None, 15, 6, -3, 0, 2, 9],
        "alternating": [-2, 2] * 6,
        "random": np.random.default_rng(20261005).normal(size=40).tolist(),
    }
    cases = []
    for name, values in inputs.items():
        for days in [3, 4, 5, 10]:
            rolling = pd.Series(values, dtype=float).rolling(days)
            expected = {}
            for op in ["skew", "kurt"]:
                expected[op] = [
                    None if math.isnan(x) else x for x in getattr(rolling, op)()
                ]
            cases.append(dict(name=name, values=values, days=days, **expected))
    path = Path(__file__).resolve().parents[1] / "tests/fixtures/moment_reference.json"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(dict(pandas=pd.__version__, numpy=np.__version__, cases=cases),
                   indent=2, allow_nan=False) + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
