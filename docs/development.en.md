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

## Releases And PyPI

`.github/workflows/release.yml` builds CPython 3.10+ stable-ABI wheels for
Windows, Linux, and macOS on version tags. Every native runner executes
`scripts/smoke_wheel.py`; the Linux aarch64 wheel is cross-compiled and cannot
run on the x86_64 runner.

PyPI uses Trusted Publishing, so no publishing token is stored in the
repository. Before the first publication, a maintainer must configure this
publisher for `qweave` on PyPI:

```text
Owner: GaomingOrion
Repository: qweave
Workflow: release.yml
Environment: pypi
```

After that configuration, a new version tag builds the same distributions for
GitHub Release and the `publish-pypi` job uploads them to PyPI. Before tagging,
follow the repository release rules for version, Changelog, and release notes.

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
