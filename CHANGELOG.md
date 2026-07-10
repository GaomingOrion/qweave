# Changelog

All notable changes will be documented in this file.

The project follows pre-1.0 semantic versioning: minor versions may change APIs,
and patch versions should remain backward compatible within a minor line.

## Unreleased

- None.

## v0.4.0 - 2026-07-02

- Added `qweave.qlib_alpha158(input_alias, alphas=None)` — the Qlib Alpha158
  feature set (9 kbar + 4 price + 29 rolling groups × 5 windows = 158 factors)
  as alpha expressions. See `docs/qlib_alpha158.md` for caliber notes.
- Added `slope`, `rsquare`, `resi`, and `quantile` as expression kernels
  (`PyExpr` methods and Rust builders) backing the Alpha158 regression and
  quantile factors.
- Added `PyExpr.output_name()` to read an expression's alias, so `list[PyExpr]`
  results can be filtered by factor name.
- Breaking: renamed Python `worldquant101_alphas(...)` to
  `worldquant_alpha101(...)`.
- Breaking: removed the factor-kernel Python API (`compute_panel(...)` and
  `factor_catalog()`). All built-in factors are now alpha expressions computed
  through `compute_alphas` / `with_alphas`.
- Removed the `qweave-macros` crate and the `#[factor]`/`#[alpha]` registries;
  the WorldQuant 101 and Alpha158 sets are plain Rust builders returning
  `Vec<(String, Expr)>`.

## v0.3.1 - 2026-07-02

- Removed the undocumented `column_aliases` parameter from Python
  `compute_alphas(...)` and `with_alphas(...)`; alpha field remapping now only
  uses `PyExpr.replace_inputs(...)` or `worldquant101_alphas(input_alias=...)`.
- Documented the extra per-output scatter buffer used by `with_alphas` to
  preserve original row order.
- Changed `worldquant101_alphas(...)` to resolve names through the cached alpha
  registry and reject non-WorldQuant names such as `group_returns_rank` and
  `alpha102`.
- Refactored field collection to use a shared field visitor instead of carrying
  a second full AST traversal in `collect_fields`.

## v0.3.0 - 2026-07-01

- Added the Python expression API (`PyExpr`) with column/literal constructors,
  arithmetic and comparison operators, time-series windows, cross-sectional
  ranks, group operations, aliases, input collection, and input replacement.
- Added `qweave.with_alphas(...)` to append expression outputs to the input
  DataFrame while preserving original row order.
- Changed `qweave.compute_alphas(...)` to accept aliased expressions and emit
  the full `(time, symbol)` history; `output_path` now writes that full frame to
  Parquet.
- Added `qweave.worldquant101_alphas(...)` and `_worldquant101_alphas()` as
  the Python discovery/remapping surface for the built-in WorldQuant 101
  expressions.
- Added Python type stubs for the extension module.
- Updated README and docs for the expression API and WorldQuant 101 workflow.
- Added public project documentation for GitHub release readiness.
- Added CI documentation and workflow expectations.
- Breaking: removed the Python `alpha_catalog()` binding and removed
  `observation_times` from Python `compute_alphas(...)`.
