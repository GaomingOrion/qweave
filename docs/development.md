# Development

## Environment

Install Python dependencies and build the extension:

```bash
uv sync --dev
uv run maturin develop
```

The Rust toolchain is pinned by `rust-toolchain.toml`. Cargo will install/use
the configured nightly toolchain when needed.

## Checks

Run these before committing:

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
uv run maturin develop
uv run pytest
```

`cargo test --workspace` includes Rust unit tests and `trybuild` macro UI tests.
The first run can take several minutes because Polars and PyO3 dependencies are
compiled from source.

## Python Extension

`uv run maturin develop` builds and installs the local extension module into the
project environment. Re-run it after changing Rust code used by `qfactors-py`.

## Benchmarks

The synthetic alpha benchmark is an ignored Rust test:

```bash
cargo test -p qfactors-factors synthetic_alpha_benchmark -- --ignored --nocapture
```

Benchmark dimensions can be adjusted with:

- `QFACTORS_BENCH_SYMBOLS`
- `QFACTORS_BENCH_TIMES`
- `QFACTORS_BENCH_REPEATS`

To compare alpha engines locally, set `QF_ENGINE=tree` or `QF_ENGINE=dag`.

## Golden Fixtures

The checked-in Parquet fixture for all-alphas regression coverage is synthetic.
Only update it when an intentional implementation change alters expected output.
Review the diff and mention the reason in the commit or pull request.
