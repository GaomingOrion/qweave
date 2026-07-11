# qweave Documentation

[中文](index.md)

qweave is a Polars-native, Rust-accelerated factor research engine: batch
factor computation, leakage-safe labels, IC/quantile/turnover evaluation, and
an interactive report in one DataFrame pipeline. All 158 Qlib Alpha158 factors
run in about 2.2 s on a 4.8M-row panel ([measured data](benchmark.en.md)).

The documentation follows the order of a real factor-research workflow. New
users should start with the repository
[quickstart](../README.en.md#from-market-data-to-a-factor-report) and the
runnable [example](../examples/README.en.md).

## Learning Path

1. **Install and run the complete example:** [README quickstart](../README.en.md#from-market-data-to-a-factor-report).
2. **Build and compose factors:** [Python Expression API](expression_api.en.md).
3. **Use classic factor libraries:** [WorldQuant 101](worldquant_alpha101.en.md)
   and [Qlib Alpha158](qlib_alpha158.en.md).
4. **Create labels and evaluate factors:** [Factor Evaluation](factor_evaluation.en.md).
5. **Understand execution:** [Architecture and Design Tradeoffs](architecture.en.md).
6. **Run large workloads:** [Benchmarks](benchmark.en.md).

## Positioning and Comparison

- [Comparison](comparison.en.md) explains where qweave, Qlib, KunQuant, and
  pandas/Alphalens-style tools each fit, with measured head-to-head numbers on
  the same panel.
- qweave focuses on the factor computation and diagnostics layer. It runs
  standalone or embeds into a larger research platform.
- Maintainers and contributors should read the
  [Development Guide](development.en.md) and
  [CONTRIBUTING](../CONTRIBUTING.en.md).
