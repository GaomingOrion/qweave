# Contributing

Thanks for taking the time to improve qfactors. Keep changes focused and
verifiable.

## Setup

```bash
uv sync --dev
uv run maturin develop
```

Rust uses the pinned nightly toolchain from `rust-toolchain.toml`.

## Required Checks

Before opening a pull request, run:

```bash
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
uv run maturin develop
uv run pytest
```

## Test Data

Do not commit private market data, credentials, local paths, or generated build
artifacts. The checked-in golden fixture under `crates/qfactors-factors/tests`
is synthetic test data.

If an intentional alpha implementation change requires updating the golden
fixture, run the existing bless flow documented in test failures, review the
resulting diff carefully, and explain why the output changed in the pull
request.

## Style

- Match the surrounding Rust and Python style.
- Keep API changes explicit in docs and tests.
- Prefer small, reviewable changes over broad refactors.
- Add focused tests for behavior changes.
