# Comparison

[Chinese](comparison.md)

qweave is a **lightweight Rust/Polars kernel for factor research**. It does not
try to own data platforms, model training, portfolio optimization, and live
execution all at once. Instead, it focuses on the part that often gets slow and
messy: factor expressions, batch computation, labels, evaluation, and reports.
On the same 4.8M-row synthetic panel, qweave is 23.85× faster than Qlib
Alpha158DL and 2.56× faster end to end than KunQuant's JIT C++ path
([measured data and calibers](benchmark.en.md)).

## Where qweave Helps

**Less glue code.** Researchers can append alphas, labels, and evaluation outputs
to the same DataFrame instead of manually maintaining multiple wide tables, long
tables, and index joins.

**Batch factors feel natural.** Built-in factor libraries return normal
expression objects, so you can select subsets, remap inputs, add custom
expressions, and submit everything to the evaluator together.

**Performance work lives in the library.** Users write Python expressions, while
execution enters a Rust DAG with common-subexpression reuse, slot reuse,
node-level parallelism, and fused elementwise chains.

**Evaluation calibers are explicit.** `with_labels` uses a panel-wide date grid,
so missing symbol-days do not silently compress holding periods. `evaluate`
separates factor-valid samples, pair-valid samples, tradability filters, and
minimum cross-section counts.

## One-Line Differences

- **Qlib** is a full AI quant platform; qweave is a factor research kernel that
  can be embedded into existing data pipelines.
- **KunQuant** is an expression compiler and JIT runner; qweave is a
  Python-friendly Rust/Polars workflow that does not ask researchers to manage
  C++ build artifacts.
- **Alphalens-style tools** focus on analysis reports; qweave keeps computation,
  labels, and evaluation in one pipeline.

## Comparison Table

| Dimension | qweave | Qlib | KunQuant |
| --- | --- | --- | --- |
| Primary goal | Factor processing and evaluation, expanding toward modeling/strategy/backtesting | AI quant research platform covering a broader investment workflow | Batch expression optimization, code generation, and JIT execution |
| Input experience | Polars DataFrame directly through Python API | Usually organized around Qlib data providers and handlers | Build expression graphs, then compile and run |
| Execution core | Rust kernels with Polars/PyO3 bindings | Mostly Python ecosystem with pandas/NumPy-oriented paths | Generates optimized C++ and runs through a runner |
| Factor libraries | WorldQuant 101 and Qlib Alpha158 as composable expressions | Alpha158/Alpha360-style handlers | Predefined Alpha101 path |
| Evaluation loop | `with_labels`, `evaluate`, IC/RankIC, quantile returns, turnover, reports | Full experiment and backtesting capabilities inside the platform | Mainly expression computation, not an evaluation framework |
| Best fit | Existing DataFrame/Parquet data where you want fast factor computation and evaluation | Full research platform with model and experiment management | Compiling expression bundles into high-performance execution paths |
| Current boundary | pre-1.0; modeling/strategy/backtesting are planned | Broader platform, with higher adoption surface | Strong JIT/compiler path, but requires workflow assembly |

Beyond these qualitative boundaries, [Benchmarks](benchmark.en.md) publishes
measured head-to-head numbers against Qlib Alpha158DL and KunQuant on the same
synthetic panel, with machine specs, commit, and full commands.

## Where It Is Not The Best Fit

- Ready-made data ingestion, model templates, portfolio optimization, and
  experiment-managed backtests: choose Qlib.
- Compiling a fixed expression bundle into maximally optimized C++ code:
  choose KunQuant.
- A small one-off notebook analysis: pandas/Alphalens-style tools may be
  lighter.

qweave fits the space where you already have your own data and research process,
but want the factor computation and evaluation layer to be faster, more unified,
and easier to reproduce.

## References

- [Microsoft Qlib](https://github.com/microsoft/qlib)
- [KunQuant](https://github.com/Menooker/KunQuant)
