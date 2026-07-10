# 开发者手册

[English](development.en.md)

本文是维护者入口，面向要修改 qweave 源码的人。用户能力、对比和 benchmark 解释见
[README](../README.md)、[项目对比](comparison.md) 和 [性能与基准测试](benchmark.md)。

## 环境

安装 Python 依赖并构建本地扩展：

```powershell
uv sync --dev
uv run maturin develop
```

Rust toolchain 由 `rust-toolchain.toml` 固定。Cargo 会在需要时自动安装或使用配置好
的 nightly toolchain。

## 提交前检查

```powershell
cargo fmt --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
uv run maturin develop
uv run python -m pytest
```

`cargo test --workspace` 会运行 Rust 单测和集成测试。首次运行可能需要几分钟，因为
Polars 和 PyO3 依赖需要从源码编译。

在当前 Windows 环境里，如果 workspace 测试二进制需要加载 Python DLL，可以把 uv
管理的 Python 安装目录临时加入 `PATH` 后再运行测试。

## Python 扩展

`uv run maturin develop` 会构建并安装本地扩展模块到项目环境。修改 `qweave-py`
使用到的 Rust 代码后，需要重新运行该命令。

## 发布与 PyPI

`.github/workflows/release.yml` 在版本 tag 上构建 Windows、Linux 和 macOS 的
CPython 3.10+ stable ABI wheels，并对所有原生 runner 执行
`scripts/smoke_wheel.py`。Linux aarch64 wheel 是交叉编译产物，不在 x86_64 runner
上执行。

PyPI 使用 Trusted Publishing，不在仓库保存 token。首次发布前，维护者需要在 PyPI
为 `qweave` 配置以下 publisher：

```text
Owner: GaomingOrion
Repository: qweave
Workflow: release.yml
Environment: pypi
```

配置完成后，新的版本 tag 会在 GitHub Release 成功构建同一组分发文件，并由
`publish-pypi` job 上传到 PyPI。发布前仍需按仓库规则更新版本、Changelog 和 release
notes。

## 本地 benchmark

合成 alpha benchmark 是 ignored Rust test：

```powershell
cargo test -p qweave-factors synthetic_alpha_benchmark -- --ignored --nocapture
```

benchmark 维度可通过环境变量调整：

- `QWEAVE_BENCH_SYMBOLS`
- `QWEAVE_BENCH_TIMES`
- `QWEAVE_BENCH_REPEATS`

比较 alpha 引擎时使用：

```powershell
$env:QWEAVE_ENGINE = "tree"  # 或 "dag"
cargo test -p qweave-factors all_alphas_golden_matches_frozen_baseline
Remove-Item Env:\QWEAVE_ENGINE
```

跨引擎 benchmark 的公开复现命令见 [性能与基准测试](benchmark.md)。

## Golden fixtures

仓库中的 Parquet golden fixtures 使用合成数据。只有当实现变更确实改变预期输出时
才更新 fixture；更新时必须 review diff，并在提交或 PR 中说明原因。
