from examples.quickstart import FACTOR_NAMES, evaluate_sample


def test_bundled_quickstart_runs_end_to_end():
    result = evaluate_sample()

    assert result.summary.height == len(FACTOR_NAMES) * 3
    assert set(result.summary.get_column("factor")) == set(FACTOR_NAMES)
    assert set(result.summary.get_column("horizon")) == {1, 5, 20}
