# Changelog

All notable changes will be documented in this file.

The project follows pre-1.0 semantic versioning: minor versions may change APIs,
and patch versions should remain backward compatible within a minor line.

## v0.6.0 - 2026-07-17

- **Breaking:** `compute_alphas` now returns rows sorted by `(symbol, time)`
  instead of `(time, symbol)`, matching the common instrument-major panel
  layout. `with_alphas` still returns results in the original input row order.
- Rewrote the DAG alpha scheduler to a memory-frugal postorder-priority batched
  order, lowering peak memory on large multi-factor runs (e.g. WorldQuant101 +
  Qlib158 over ~15M rows) without giving up parallelism.
- Added the built-in `gtja_alpha191()` collection with padded output names
  `gtja_alpha001` through `gtja_alpha191`.
- Corrected the Alpha191 `MAX(x, 0)` and DBM formula transcriptions.
- Built-in factor builders now print the resolved canonical-input-to-DataFrame
  mapping for the requested factors, making aliases such as
  `close -> close_adj` visible at construction time.

## v0.5.0 - 2026-07-11

- Added non-null String and integer group columns backed by validated `i32`
  storage for `group_rank`, `group_neutralize`, and WorldQuant 101 groups.
- Removed the legacy static `to_html` report API in favor of `view()`.
- Added an in-report button that gracefully stops the local report server.

- Repositioned the bilingual README around qweave's Polars-native factor
  research workflow, with a reproducible end-to-end example and real report
  screenshot.
- Added a deterministic synthetic sample panel, documentation learning path,
  and GitHub social-preview assets.
- Added a 5,000-symbol × 1,000-day batch-DAG benchmark against per-factor DAG
  calls, including process peak RSS.
- Added native release-wheel smoke tests and PyPI Trusted Publishing to the
  release workflow.

## v0.4.1 - 2026-07-10

- Renamed the public project surface to qweave in documentation and repository
  metadata.
- Reworked the README and docs into Chinese-first bilingual public
  documentation, with clearer positioning against Qlib and KunQuant.
- Added release benchmark notes for the Windows/PowerShell environment.
- Updated the GitHub Release workflow to use checked-in release notes when a
  tag-specific notes file is present.

## v0.4.0 - 2026-07-02

- Added `qweave.qlib_alpha158(input_alias, alphas=None)` - the Qlib Alpha158
  feature set (9 kbar + 4 price + 29 rolling groups x 5 windows = 158 factors)
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
