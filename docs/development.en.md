# Development Guide

[Chinese](development.md)

This is the maintainer entry point for people changing qweave source code. For
user-facing capabilities, positioning, and benchmark interpretation, see the
[README](../README.en.md), [Comparison](comparison.en.md), and
[Performance And Benchmarks](benchmark.en.md).

## Environment

Install Python dependencies and build the extension:

```powershell
uv sync --dev
uv run maturin develop
```

The Rust toolchain is pinned by `rust-toolchain.toml`. Cargo will install or use
the configured nightly toolchain when needed.

## Pre-Commit Checks

```powershell
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
uv run maturin develop
uv run python -m pytest
```

`cargo test --workspace` runs Rust unit and integration tests. The first run can
take several minutes because Polars and PyO3 dependencies are compiled from
source.

## Python Extension

`uv run maturin develop` builds and installs the local extension module into the
project environment. Re-run it after changing Rust code used by `qweave-py`.

## Local Benchmarks

The synthetic alpha benchmark is an ignored Rust test:

```powershell
cargo test -p qweave-factors synthetic_alpha_benchmark -- --ignored --nocapture
```

Benchmark dimensions can be adjusted with:

- `QWEAVE_BENCH_SYMBOLS`
- `QWEAVE_BENCH_TIMES`
- `QWEAVE_BENCH_REPEATS`

To compare alpha engines locally:

```powershell
$env:QWEAVE_ENGINE = "tree"  # or "dag"
cargo test -p qweave-factors all_alphas_golden_matches_frozen_baseline
Remove-Item Env:\QWEAVE_ENGINE
```

For public cross-engine reproduction commands, see
[Performance And Benchmarks](benchmark.en.md).

## Golden Fixtures

The checked-in Parquet golden fixtures use synthetic data. Only update them when
an intentional implementation change alters expected output. Review the diff and
mention the reason in the commit or pull request.
