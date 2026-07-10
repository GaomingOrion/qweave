# Architecture And Design Tradeoffs

[Chinese](architecture.md)

qweave's architecture follows a simple goal: **researchers write Python
expressions, while performance-sensitive paths stay in Rust.** The Python API is
for ergonomics; the Rust workspace handles panel layout, expression evaluation,
evaluation statistics, and report output.

## Design Principles

- **DataFrame in, DataFrame out.** qweave does not require users to migrate into
  a dedicated data provider. If the data already lives in a Polars DataFrame or
  Parquet workflow, it can be used directly.
- **Expressions are the research interface.** Built-in and custom factors use
  the same `PyExpr` representation, so they can be selected, composed, remapped,
  and batch-executed together.
- **Batch computation first.** Alphas are rarely isolated formulas; they are
  usually batches with overlapping windows and inputs. The default evaluator
  lowers the whole batch into a shared DAG.
- **Explicit calibers.** Panel sorting, duplicate symbol-time checks, NaN
  propagation, window warmup, label calendars, and tradability filters are
  documented and tested.

## Data Flow

```text
Polars DataFrame
  -> Python API validates request shape
  -> Rust panel layout sorts and checks (symbol, time)
  -> expression DAG evaluates alphas over the full panel
  -> labels/evaluation/reporting reuse the same aligned frame
  -> Polars DataFrame, Parquet, HTML, or interactive report
```

`with_alphas` fits workflows that preserve the original input shape: it computes
the full panel and scatters results back into original row order. `compute_alphas`
fits large factor outputs: it returns a tidy `(time, symbol)` result or writes
Parquet.

## Alpha Evaluator

The DAG evaluator is the default alpha engine. It lowers requested expressions
into a shared DAG to:

- reuse common subexpressions, such as the same rolling mean across many alphas;
- reuse intermediate slots and reduce temporary allocations;
- execute fusible elementwise chains continuously;
- use node-level parallelism where it has been validated.

The tree evaluator remains as an independent reference implementation for
debugging DAG optimizations:

```powershell
$env:QWEAVE_ENGINE = "tree"
uv run python -m pytest
Remove-Item Env:\QWEAVE_ENGINE
```

Valid values are `dag` and `tree`.

## Workspace Structure

- `qweave-core`: panel layout, column validation, alpha expression evaluation,
  and result sinks.
- `qweave-factors`: built-in alpha builders for WorldQuant 101 and Qlib
  Alpha158.
- `qweave-eval`: forward-return labels, factor evaluation, correlation, report
  tables, and HTML output.
- `qweave-server`: Axum server for the interactive evaluation report.
- `qweave-py`: PyO3 extension module exposed to Python as `qweave`.

## Current Boundaries

- qweave is not a full backtester yet; it does not simulate matching, slippage,
  capital curves, or portfolio constraints.
- The evaluation API is still marked experimental and may change before 1.0.
- The benchmark documentation provides reproducible methods; the repository does
  not keep performance numbers from the pre-Windows environment.
