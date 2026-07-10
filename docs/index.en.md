# qweave Documentation

[中文](index.md)

The documentation follows the order of a real factor-research workflow. New
users should start with the repository [quickstart](../README.en.md#from-market-data-to-a-factor-report)
and the runnable [example](../examples/README.en.md).

## Learning Path

1. **Install and run the complete example:** [README quickstart](../README.en.md#from-market-data-to-a-factor-report).
2. **Build and compose factors:** [Python Expression API](expression_api.en.md).
3. **Use classic factor libraries:** [WorldQuant 101](worldquant_alpha101.en.md)
   and [Qlib Alpha158](qlib_alpha158.en.md).
4. **Create labels and evaluate factors:** [Factor Evaluation](factor_evaluation.en.md).
5. **Understand execution:** [Architecture and Design Tradeoffs](architecture.en.md).
6. **Run large workloads:** [Performance and Benchmarks](benchmark.en.md).

## Before Choosing qweave

- [Comparison](comparison.en.md) explains the boundary between qweave, Qlib,
  KunQuant, and pandas/Alphalens-style tools.
- qweave focuses on factor computation and diagnostics. It does not provide
  market data, order matching, slippage models, or full strategy backtesting.
- Maintainers and contributors should read the [Development Guide](development.en.md)
  and [CONTRIBUTING](../CONTRIBUTING.en.md).
