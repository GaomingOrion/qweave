"""Compare qweave factor computation against representative factor engines.

Usage:

    uv run maturin develop --release
    uv run python scripts/bench_factor_engines.py --workload alpha158
    uv run python scripts/bench_factor_engines.py --workload worldquant101 --engines qweave,kunquant

The default benchmark compares qweave with, when pyqlib is installed, Qlib
Alpha158DL over a generated local provider. KunQuant is optional; when the
package and a C++ toolchain are available, the same CLI runs the package's
supported WorldQuant Alpha101 subset.
"""

from __future__ import annotations

import argparse
import gc
import importlib
import json
import math
import os
import statistics
import sys
import tempfile
import time
from dataclasses import asdict, dataclass
from datetime import date, timedelta
from pathlib import Path
from typing import Callable, Iterable

import numpy as np
import polars as pl


KUNQUANT_WORLDQUANT101 = [
    "alpha1",
    "alpha2",
    "alpha3",
    "alpha4",
    "alpha5",
    "alpha6",
    "alpha7",
    "alpha8",
    "alpha9",
    "alpha10",
    "alpha11",
    "alpha12",
    "alpha13",
    "alpha14",
    "alpha15",
    "alpha16",
    "alpha17",
    "alpha18",
    "alpha19",
    "alpha20",
    "alpha21",
    "alpha22",
    "alpha23",
    "alpha24",
    "alpha25",
    "alpha26",
    "alpha27",
    "alpha28",
    "alpha29",
    "alpha30",
    "alpha31",
    "alpha32",
    "alpha33",
    "alpha34",
    "alpha35",
    "alpha36",
    "alpha37",
    "alpha38",
    "alpha39",
    "alpha40",
    "alpha41",
    "alpha42",
    "alpha43",
    "alpha44",
    "alpha45",
    "alpha46",
    "alpha47",
    "alpha49",
    "alpha50",
    "alpha51",
    "alpha52",
    "alpha53",
    "alpha54",
    "alpha55",
    "alpha57",
    "alpha60",
    "alpha61",
    "alpha62",
    "alpha64",
    "alpha65",
    "alpha66",
    "alpha68",
    "alpha71",
    "alpha72",
    "alpha73",
    "alpha74",
    "alpha75",
    "alpha77",
    "alpha78",
    "alpha81",
    "alpha83",
    "alpha84",
    "alpha85",
    "alpha86",
    "alpha88",
    "alpha92",
    "alpha94",
    "alpha95",
    "alpha96",
    "alpha98",
    "alpha99",
    "alpha101",
]
DEFAULT_WORLDQUANT101 = KUNQUANT_WORLDQUANT101


@dataclass
class BenchResult:
    engine: str
    workload: str
    status: str
    rows: int
    factors: int
    best_seconds: float | None = None
    mean_seconds: float | None = None
    stdev_seconds: float | None = None
    rows_per_second: float | None = None
    cells_per_second: float | None = None
    compile_seconds: float | None = None
    output_rows: int | None = None
    output_columns: int | None = None
    note: str = ""


def build_ohlcv_panel(n_symbols: int, n_days: int, seed: int = 0) -> pl.DataFrame:
    """Build a deterministic OHLCV panel with fields used by Alpha158/101."""
    rng = np.random.default_rng(seed)
    returns = rng.normal(0.0004, 0.02, size=(n_symbols, n_days))
    close = 20.0 * np.exp(np.cumsum(returns, axis=1))
    close *= rng.uniform(0.7, 1.3, size=(n_symbols, 1))
    open_ = close * (1.0 + rng.normal(0.0, 0.004, size=(n_symbols, n_days)))
    high = np.maximum(open_, close) * (1.0 + rng.uniform(0.0, 0.01, size=(n_symbols, n_days)))
    low = np.minimum(open_, close) * (1.0 - rng.uniform(0.0, 0.01, size=(n_symbols, n_days)))
    volume = rng.lognormal(mean=13.0, sigma=0.35, size=(n_symbols, n_days))
    vwap = (open_ + high + low + close) / 4.0
    amount = vwap * volume
    cap = close * rng.uniform(5e7, 2e8, size=(n_symbols, 1))

    ret = np.full_like(close, np.nan)
    ret[:, 1:] = close[:, 1:] / close[:, :-1] - 1.0
    sectors = np.arange(n_symbols, dtype=float) % 8
    industries = np.arange(n_symbols, dtype=float) % 16
    subindustries = np.arange(n_symbols, dtype=float) % 32

    symbols = np.repeat([f"S{i:05d}" for i in range(n_symbols)], n_days)
    times = np.tile(np.arange(1, n_days + 1, dtype=np.int64), n_symbols)

    def flat(values: np.ndarray) -> np.ndarray:
        return values.reshape(-1)

    return pl.DataFrame(
        {
            "asset": symbols,
            "time": times,
            "open": flat(open_),
            "high": flat(high),
            "low": flat(low),
            "close": flat(close),
            "volume": flat(volume),
            "vwap": flat(vwap),
            "amount": flat(amount),
            "returns": flat(ret),
            "cap": flat(cap),
            "sector": np.repeat(sectors, n_days),
            "industry": np.repeat(industries, n_days),
            "subindustry": np.repeat(subindustries, n_days),
        }
    )


def parse_csv(value: str | None) -> list[str] | None:
    if value is None:
        return None
    out = [part.strip() for part in value.split(",") if part.strip()]
    return out or None


def worldquant_names(names: Iterable[str] | None = None) -> list[str]:
    selected = list(names) if names else list(DEFAULT_WORLDQUANT101)
    known = {f"alpha{idx}" for idx in range(1, 102)}
    unknown = [name for name in selected if name not in known]
    if unknown:
        raise ValueError(f"unknown WorldQuant alpha name(s): {', '.join(unknown)}")
    return selected


def qlib_alpha158_config(workload: str, names: Iterable[str] | None = None) -> tuple[list[str], list[str]]:
    from qlib.contrib.data.loader import Alpha158DL

    if workload == "alpha158":
        selected = list(names) if names else None
        config = {
            "kbar": {},
            "price": {
                "windows": [0],
                "feature": ["OPEN", "HIGH", "LOW", "VWAP"],
            },
            "rolling": {},
        }
    else:
        raise ValueError(f"Qlib Alpha158DL does not support workload: {workload}")

    fields, qlib_names = Alpha158DL.get_feature_config(config)
    selected = selected or qlib_names
    by_name = dict(zip(qlib_names, fields))
    missing = [name for name in selected if name not in by_name]
    if missing:
        raise ValueError(f"Qlib Alpha158DL did not produce: {', '.join(missing)}")
    return [by_name[name] for name in selected], selected


def write_qlib_provider(df: pl.DataFrame, provider_dir: Path) -> tuple[list[str], str, str]:
    sorted_df = df.sort(["asset", "time"])
    assets = sorted_df.get_column("asset").unique(maintain_order=True).to_list()
    times = sorted_df.get_column("time").unique(maintain_order=True).to_list()
    n_assets = len(assets)
    n_times = len(times)
    if sorted_df.height != n_assets * n_times:
        raise ValueError("Qlib benchmark requires a dense asset x time panel")

    dates = [date(2000, 1, 1) + timedelta(days=idx) for idx in range(n_times)]
    start = dates[0].isoformat()
    end = dates[-1].isoformat()

    calendars = provider_dir / "calendars"
    instruments = provider_dir / "instruments"
    features = provider_dir / "features"
    calendars.mkdir(parents=True, exist_ok=True)
    instruments.mkdir(parents=True, exist_ok=True)
    features.mkdir(parents=True, exist_ok=True)
    (calendars / "day.txt").write_text(
        "".join(f"{day.isoformat()}\n" for day in dates),
        encoding="utf-8",
    )
    (instruments / "all.txt").write_text(
        "".join(f"{asset}\t{start}\t{end}\n" for asset in assets),
        encoding="utf-8",
    )

    field_cols = ["open", "high", "low", "close", "volume", "vwap"]
    for asset in assets:
        asset_df = sorted_df.filter(pl.col("asset") == asset).sort("time")
        asset_dir = features / asset.lower()
        asset_dir.mkdir(parents=True, exist_ok=True)
        for column in field_cols:
            values = asset_df.get_column(column).to_numpy().astype(np.float32)
            payload = np.concatenate([np.array([0.0], dtype=np.float32), values]).astype("<f4")
            payload.tofile(asset_dir / f"{column}.day.bin")

    return assets, start, end


def run_qlib_alpha158_prepared(
    provider_dir: Path,
    instruments: list[str],
    start_time: str,
    end_time: str,
    workload: str,
    names: Iterable[str] | None = None,
    kernels: int | None = None,
):
    import logging

    import qlib
    from qlib.contrib.data.loader import Alpha158DL

    init_kwargs = {
        "provider_uri": str(provider_dir),
        "region": "cn",
        "logging_level": logging.ERROR,
        "clear_mem_cache": True,
    }
    if kernels is not None:
        init_kwargs["kernels"] = kernels
    qlib.init(**init_kwargs)
    fields, qlib_names = qlib_alpha158_config(workload, names)
    loader = Alpha158DL(config={"feature": (fields, qlib_names)}, swap_level=True)
    return loader.load(instruments=instruments, start_time=start_time, end_time=end_time)


def run_qweave(df: pl.DataFrame, workload: str, names: Iterable[str] | None = None) -> pl.DataFrame:
    qweave = importlib.import_module("qweave")
    if workload == "alpha158":
        selected = list(names) if names else None
        alphas = qweave.qlib_alpha158({}, alphas=selected)
    elif workload == "worldquant101":
        selected = worldquant_names(names)
        alphas = qweave.worldquant_alpha101({}, alphas=selected)
    else:
        raise ValueError(f"unsupported qweave workload: {workload}")
    return qweave.compute_alphas(df, symbol_col="asset", time_col="time", alphas=alphas)


def _kunquant_alpha_attr(name: str) -> str:
    number = int(name.removeprefix("alpha"))
    return f"alpha{number:03d}"


def ascii_temp_root() -> Path | None:
    if os.name != "nt":
        return None
    root = Path(os.environ.get("QWEAVE_BENCH_TMP", r"C:\qweave-bench-tmp"))
    root.mkdir(parents=True, exist_ok=True)
    return root


def run_kunquant_worldquant101(
    df: pl.DataFrame,
    names: Iterable[str] | None = None,
    threads: int | None = None,
) -> tuple[dict[str, np.ndarray], float]:
    """Run KunQuant's predefined Alpha101 path when the optional package exists."""
    selected = worldquant_names(names)
    try:
        from KunQuant.Driver import KunCompilerConfig
        from KunQuant.jit import cfake
        from KunQuant.Op import Builder, Input, Output
        from KunQuant.predefined import Alpha101
        from KunQuant.runner import KunRunner as kr
        from KunQuant.Stage import Function
    except ImportError as exc:  # pragma: no cover - depends on optional package
        raise RuntimeError(f"KunQuant is not installed: {exc}") from exc

    builder = Builder()
    with builder:
        low = Input("low")
        high = Input("high")
        close = Input("close")
        open_ = Input("open")
        amount = Input("amount")
        volume = Input("volume")
        all_data = Alpha101.AllData(
            low=low,
            high=high,
            close=close,
            open=open_,
            amount=amount,
            volume=volume,
        )
        for name in selected:
            fn = getattr(Alpha101, _kunquant_alpha_attr(name))
            Output(fn(all_data), name)

    compile_start = time.perf_counter()
    function = Function(builder.ops)
    with tempfile.TemporaryDirectory(
        prefix="qweave-kunquant-",
        dir=ascii_temp_root(),
        ignore_cleanup_errors=True,
    ) as tmp:
        out_name = str(Path(tmp) / "alpha101_lib")
        lib = cfake.compileit(
            [("alpha101", function, KunCompilerConfig(input_layout="TS", output_layout="TS"))],
            out_name,
            cfake.CppCompilerConfig(),
        )
        module = lib.getModule("alpha101")
        compile_seconds = time.perf_counter() - compile_start

        input_dict = panel_to_kunquant_inputs(df)
        executor = kr.createMultiThreadExecutor(threads or os.cpu_count() or 1)
        return kr.runGraph(executor, module, input_dict, 0, input_dict["close"].shape[0]), compile_seconds


def panel_to_kunquant_inputs(df: pl.DataFrame) -> dict[str, np.ndarray]:
    """Convert asset/time rows to KunQuant TS arrays shaped [time, stocks]."""
    sorted_df = df.sort(["asset", "time"])
    assets = sorted_df.get_column("asset").unique(maintain_order=True).to_list()
    times = sorted_df.get_column("time").unique(maintain_order=True).to_list()
    n_assets = len(assets)
    n_times = len(times)
    expected = n_assets * n_times
    if sorted_df.height != expected:
        raise ValueError("KunQuant benchmark requires a dense asset x time panel")
    out = {}
    for column in ["open", "high", "low", "close", "volume", "amount"]:
        values = sorted_df.get_column(column).to_numpy().reshape(n_assets, n_times)
        out[column] = np.ascontiguousarray(values.T.astype(np.float32))
    return out


def measure(call: Callable[[], object], repeats: int, warmups: int) -> tuple[list[float], object]:
    result = None
    for _ in range(warmups):
        result = call()
    times = []
    for _ in range(repeats):
        gc.collect()
        start = time.perf_counter()
        result = call()
        times.append(time.perf_counter() - start)
    return times, result


def summarize(
    engine: str,
    workload: str,
    rows: int,
    factors: int,
    times: list[float],
    output: object,
    compile_seconds: float | None = None,
    note: str = "",
) -> BenchResult:
    best = min(times)
    mean = statistics.fmean(times)
    stdev = statistics.stdev(times) if len(times) > 1 else 0.0
    out_rows, out_cols = output_shape(output)
    return BenchResult(
        engine=engine,
        workload=workload,
        status="ok",
        rows=rows,
        factors=factors,
        best_seconds=best,
        mean_seconds=mean,
        stdev_seconds=stdev,
        rows_per_second=rows / best if best > 0 else math.inf,
        cells_per_second=(rows * factors) / best if best > 0 else math.inf,
        compile_seconds=compile_seconds,
        output_rows=out_rows,
        output_columns=out_cols,
        note=note,
    )


def skipped(engine: str, workload: str, rows: int, factors: int, note: str) -> BenchResult:
    return BenchResult(
        engine=engine,
        workload=workload,
        status="skipped",
        rows=rows,
        factors=factors,
        note=note,
    )


def output_shape(output: object) -> tuple[int | None, int | None]:
    if isinstance(output, pl.DataFrame):
        return output.height, len(output.columns)
    if isinstance(output, dict):
        if not output:
            return 0, 0
        first = next(iter(output.values()))
        if hasattr(first, "shape"):
            return int(np.prod(first.shape)), len(output)
        return None, len(output)
    if hasattr(output, "shape") and len(output.shape) == 2:
        return int(output.shape[0]), int(output.shape[1])
    return None, None


def workload_names(workload: str, names: list[str] | None) -> list[str]:
    if workload == "alpha158":
        return names or []
    if workload == "worldquant101":
        return worldquant_names(names)
    raise ValueError(f"unknown workload: {workload}")


def default_engines(workload: str) -> list[str]:
    if workload == "alpha158":
        return ["qweave", "qlib"]
    if workload == "worldquant101":
        return ["qweave", "kunquant"]
    raise ValueError(f"unknown workload: {workload}")


def run_benchmarks(args: argparse.Namespace) -> list[BenchResult]:
    names = parse_csv(args.names)
    selected = workload_names(args.workload, names)
    factor_count = len(selected) if selected else 158
    engines = parse_csv(args.engines) or default_engines(args.workload)
    df = build_ohlcv_panel(args.symbols, args.days, seed=args.seed)
    results: list[BenchResult] = []

    for engine in engines:
        if engine == "qweave":
            call = lambda: run_qweave(df, args.workload, selected or None)
            try:
                times, output = measure(call, args.repeats, args.warmups)
                results.append(summarize(engine, args.workload, df.height, factor_count, times, output))
            except Exception as exc:  # pragma: no cover - environment dependent
                results.append(skipped(engine, args.workload, df.height, factor_count, str(exc)))
        elif engine == "kunquant":
            if args.workload != "worldquant101":
                results.append(
                    skipped(engine, args.workload, df.height, factor_count, "KunQuant runner supports worldquant101")
                )
                continue
            try:
                compile_seconds_holder: list[float] = []

                def call_kunquant():
                    output, compile_seconds = run_kunquant_worldquant101(
                        df,
                        selected,
                        threads=args.threads,
                    )
                    compile_seconds_holder.append(compile_seconds)
                    return output

                times, output = measure(call_kunquant, args.repeats, args.warmups)
                compile_seconds = min(compile_seconds_holder) if compile_seconds_holder else None
                results.append(
                    summarize(
                        engine,
                        args.workload,
                        df.height,
                        factor_count,
                        times,
                        output,
                        compile_seconds=compile_seconds,
                        note="elapsed includes compile+run; compile_seconds reports the best observed compile phase",
                    )
                )
            except Exception as exc:  # pragma: no cover - optional package/toolchain
                results.append(skipped(engine, args.workload, df.height, factor_count, str(exc)))
        elif engine == "qlib":
            if args.workload != "alpha158":
                results.append(
                    skipped(engine, args.workload, df.height, factor_count, "Qlib runner supports Alpha158 workloads")
                )
                continue
            try:
                with tempfile.TemporaryDirectory(prefix="qweave-qlib-") as tmp:
                    provider_dir = Path(tmp)
                    instruments, start_time, end_time = write_qlib_provider(df, provider_dir)
                    call = lambda: run_qlib_alpha158_prepared(
                        provider_dir,
                        instruments,
                        start_time,
                        end_time,
                        args.workload,
                        selected or None,
                        kernels=args.threads,
                    )
                    times, output = measure(call, args.repeats, args.warmups)
                    results.append(
                        summarize(
                            engine,
                            args.workload,
                            df.height,
                            factor_count,
                            times,
                            output,
                            note="uses Qlib Alpha158DL over a generated local Qlib binary provider",
                        )
                    )
            except Exception as exc:  # pragma: no cover - optional package/provider details
                results.append(skipped(engine, args.workload, df.height, factor_count, str(exc)))
        else:
            results.append(skipped(engine, args.workload, df.height, factor_count, f"unknown engine: {engine}"))
    return results


def format_seconds(value: float | None) -> str:
    return "-" if value is None else f"{value:.4f}"


def format_rate(value: float | None) -> str:
    if value is None:
        return "-"
    if math.isinf(value):
        return "inf"
    return f"{value:,.0f}"


def print_table(results: list[BenchResult]) -> None:
    headers = [
        "engine",
        "status",
        "workload",
        "rows",
        "factors",
        "best_s",
        "mean_s",
        "cells/s",
        "compile_s",
        "note",
    ]
    rows = [
        [
            r.engine,
            r.status,
            r.workload,
            f"{r.rows:,}",
            str(r.factors),
            format_seconds(r.best_seconds),
            format_seconds(r.mean_seconds),
            format_rate(r.cells_per_second),
            format_seconds(r.compile_seconds),
            r.note,
        ]
        for r in results
    ]
    widths = [
        max(len(str(row[idx])) for row in [headers, *rows])
        for idx in range(len(headers))
    ]
    print("  ".join(header.ljust(widths[idx]) for idx, header in enumerate(headers)))
    print("  ".join("-" * width for width in widths))
    for row in rows:
        print("  ".join(str(value).ljust(widths[idx]) for idx, value in enumerate(row)))


def write_json(path: str, results: list[BenchResult]) -> None:
    payload = [asdict(result) for result in results]
    Path(path).write_text(json.dumps(payload, indent=2), encoding="utf-8")


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--workload",
        choices=["alpha158", "worldquant101"],
        default="alpha158",
        help="factor set to benchmark",
    )
    parser.add_argument("--engines", help="comma-separated engines; defaults depend on workload")
    parser.add_argument("--names", help="comma-separated factor names to benchmark")
    parser.add_argument("--symbols", type=int, default=64)
    parser.add_argument("--days", type=int, default=260)
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--repeats", type=int, default=3)
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--threads", type=int, help="thread count for optional KunQuant executor")
    parser.add_argument("--json", help="write machine-readable results to this file")
    return parser


def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.repeats < 1:
        raise SystemExit("--repeats must be >= 1")
    if args.warmups < 0:
        raise SystemExit("--warmups must be >= 0")
    results = run_benchmarks(args)
    print_table(results)
    if args.json:
        write_json(args.json, results)
    return 0 if any(result.status == "ok" for result in results) else 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
