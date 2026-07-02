# Changelog

All notable changes will be documented in this file.

The project follows pre-1.0 semantic versioning: minor versions may change APIs,
and patch versions should remain backward compatible within a minor line.

## Unreleased

- None.

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
- Added `qfactors.with_alphas(...)` to append expression outputs to the input
  DataFrame while preserving original row order.
- Changed `qfactors.compute_alphas(...)` to accept aliased expressions and emit
  the full `(time, symbol)` history; `output_path` now writes that full frame to
  Parquet.
- Added `qfactors.worldquant101_alphas(...)` and `_worldquant101_alphas()` as
  the Python discovery/remapping surface for the built-in WorldQuant 101
  expressions.
- Added Python type stubs for the extension module.
- Updated README and docs for the expression API and WorldQuant 101 workflow.
- Added public project documentation for GitHub release readiness.
- Added CI documentation and workflow expectations.
- Breaking: removed the Python `alpha_catalog()` binding and removed
  `observation_times` from Python `compute_alphas(...)`.
