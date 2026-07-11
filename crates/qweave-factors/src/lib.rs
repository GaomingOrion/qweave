// Bench/test builds use the same global allocator as the production cdylib so
// `synthetic_alpha_benchmark` numbers reflect jemalloc/mimalloc. `cfg(test)` keeps it out
// of the library object linked into qweave-py (which sets its own), avoiding a duplicate
// `#[global_allocator]`.
#[cfg(all(test, not(target_os = "windows")))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[cfg(all(test, target_os = "windows"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

pub mod qlib_alpha158;
pub mod worldquant_alpha101;

pub use qlib_alpha158::qlib_alpha158;
pub use worldquant_alpha101::worldquant_alpha101;

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;
    use std::collections::HashMap;
    use std::env;
    use std::time::Instant;

    use polars::prelude::*;
    use qweave_core::{ComputeResult, Expr, PanelOptions, QWeaveError, compute_alphas};

    use super::*;

    fn worldquant_alpha_map() -> HashMap<String, Expr> {
        worldquant_alpha101().into_iter().collect()
    }

    fn worldquant_alpha_names_sorted() -> Vec<String> {
        let mut names = worldquant_alpha101()
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>();
        names.sort();
        names
    }

    #[test]
    fn worldquant_alpha101_builder_returns_exact_name_set() {
        let mut names = worldquant_alpha101()
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>();
        names.sort();

        let mut expected = (1..=101)
            .map(|idx| format!("alpha{idx}"))
            .collect::<Vec<_>>();
        expected.sort();

        assert_eq!(names, expected);
    }

    #[test]
    fn worldquant_alpha101_alphas_run_on_complete_synthetic_panel() -> qweave_core::Result<()> {
        let alpha_names = (1..=101)
            .map(|idx| format!("alpha{idx}"))
            .collect::<Vec<_>>();

        let n_symbols = 6;
        let n_times = 260;
        let out = memory_frame(compute_alpha_names(
            synthetic_alpha_bench_frame(n_symbols, n_times)?,
            options(),
            alpha_names.clone(),
            Series::new("time".into(), [n_times as i64]),
            None,
        )?)?;

        let expected_columns = ["time".to_string(), "asset".to_string()]
            .into_iter()
            .chain(alpha_names)
            .collect::<Vec<_>>();
        assert_eq!(column_names(&out), expected_columns);
        assert_eq!(
            time_asset_rows(&out)?,
            (0..n_symbols)
                .map(|symbol_idx| (n_times as i64, format!("S{symbol_idx:04}")))
                .collect::<Vec<_>>()
        );
        Ok(())
    }

    #[test]
    fn qlib_alpha158_alphas_run_on_complete_synthetic_panel() -> qweave_core::Result<()> {
        let alphas = qlib_alpha158();
        assert_eq!(alphas.len(), 158);
        let alpha_names = alphas
            .iter()
            .map(|(name, _)| name.clone())
            .collect::<Vec<_>>();

        let n_symbols = 6;
        let n_times = 260;
        let full = memory_frame(compute_alphas(
            synthetic_alpha_bench_frame(n_symbols, n_times)?,
            options(),
            alphas,
            None,
        )?)?;

        let expected_columns = ["time".to_string(), "asset".to_string()]
            .into_iter()
            .chain(alpha_names)
            .collect::<Vec<_>>();
        assert_eq!(column_names(&full), expected_columns);
        assert_eq!(full.height(), n_symbols * n_times);

        // End-to-end sanity on the longest window, exercising an existing kernel
        // plus the new `slope`/`quantile` kernels: all finite once warmup passes.
        let last =
            sample_observation_times(full, "time", Series::new("time".into(), [n_times as i64]))?;
        for factor in ["MA60", "BETA60", "QTLU60"] {
            let column = last.column(factor)?.try_f64().expect("factor is f64");
            assert!(
                column.into_no_null_iter().all(f64::is_finite),
                "{factor} is finite after warmup"
            );
        }
        Ok(())
    }

    #[test]
    fn alpha8_end_to_end_matches_reference_and_compact_edges() -> qweave_core::Result<()> {
        let fixture = alpha8_fixture()?;
        let expected = reference_alpha8(&fixture);
        let out = memory_frame(compute_alpha_names(
            fixture.df,
            options(),
            vec!["alpha8".to_string()],
            Series::new("time".into(), [18i64]),
            None,
        )?)?;

        assert_eq!(out.height(), 4);
        assert_eq!(
            time_asset_rows(&out)?,
            [
                (18, "A".to_string()),
                (18, "B".to_string()),
                (18, "C".to_string()),
                (18, "D".to_string())
            ]
        );

        let values = out
            .column("alpha8")?
            .try_f64()
            .expect("alpha8 is f64")
            .into_no_null_iter()
            .collect::<Vec<_>>();
        assert_f64_eq(values[0], expected[0]);
        assert_f64_eq(values[1], expected[1]);
        assert!(values[2].is_nan());
        assert!(values[3].is_nan());
        assert!(expected[2].is_nan());
        assert!(expected[3].is_nan());
        assert!(expected[4].is_nan());

        Ok(())
    }

    #[test]
    fn phase_b_wq_alphas_match_independent_reference() -> qweave_core::Result<()> {
        let fixture = phase_b_fixture()?;
        let time = fixture.n_times - 1;
        let expected_alpha6 = reference_alpha6(&fixture, time);
        let expected_alpha12 = reference_alpha12(&fixture, time);
        let expected_alpha13 = reference_alpha13(&fixture, time);
        let expected_alpha101 = reference_alpha101(&fixture, time);

        let out = memory_frame(compute_alpha_names(
            fixture.df,
            options(),
            vec![
                "alpha6".to_string(),
                "alpha12".to_string(),
                "alpha13".to_string(),
                "alpha101".to_string(),
            ],
            Series::new("time".into(), [fixture.n_times as i64]),
            None,
        )?)?;

        assert_eq!(
            time_asset_rows(&out)?,
            ["A", "B", "C", "D", "E"]
                .into_iter()
                .map(|asset| (fixture.n_times as i64, asset.to_string()))
                .collect::<Vec<_>>()
        );
        for (actual, expected) in column_values(&out, "alpha6")?.iter().zip(expected_alpha6) {
            assert_f64_eq(*actual, expected);
        }
        for (actual, expected) in column_values(&out, "alpha12")?.iter().zip(expected_alpha12) {
            assert_f64_eq(*actual, expected);
        }
        for (actual, expected) in column_values(&out, "alpha13")?.iter().zip(expected_alpha13) {
            assert_f64_eq(*actual, expected);
        }
        for (actual, expected) in column_values(&out, "alpha101")?
            .iter()
            .zip(expected_alpha101)
        {
            assert_f64_eq(*actual, expected);
        }
        Ok(())
    }

    #[test]
    fn alpha_missing_observation_time_keeps_schema() -> qweave_core::Result<()> {
        let fixture = alpha8_fixture()?;
        let out = memory_frame(compute_alpha_names(
            fixture.df,
            options(),
            vec!["alpha8".to_string()],
            Series::new("time".into(), [99i64]),
            None,
        )?)?;

        assert_eq!(out.height(), 0);
        assert_eq!(column_names(&out), ["time", "asset", "alpha8"]);
        Ok(())
    }

    #[test]
    #[ignore]
    fn synthetic_alpha_benchmark() -> qweave_core::Result<()> {
        let n_symbols = bench_env_usize("QWEAVE_BENCH_SYMBOLS", 200);
        let n_times = bench_env_usize("QWEAVE_BENCH_TIMES", 260);
        let repeats = bench_env_usize("QWEAVE_BENCH_REPEATS", 3);
        let df = synthetic_alpha_bench_frame(n_symbols, n_times)?;
        let mut alphas = worldquant_alpha101();
        alphas.sort_by(|(lhs, _), (rhs, _)| lhs.cmp(rhs));

        println!(
            "manual run: QWEAVE_BENCH_SYMBOLS={n_symbols} QWEAVE_BENCH_TIMES={n_times} \
             QWEAVE_BENCH_REPEATS={repeats} cargo test -p qweave-factors \
             synthetic_alpha_benchmark -- --ignored --nocapture"
        );

        let started = Instant::now();
        let mut total_rows = 0usize;
        for _ in 0..repeats {
            let out = memory_frame(compute_alphas(df.clone(), options(), alphas.clone(), None)?)?;
            total_rows += out.height();
        }
        let elapsed = started.elapsed();
        println!(
            "compute_alphas synthetic: symbols={n_symbols} times={n_times} \
             alphas={} repeats={repeats} rows={total_rows} elapsed={elapsed:?} \
             per_run={:?}",
            alphas.len(),
            elapsed / repeats as u32
        );

        Ok(())
    }

    #[test]
    #[ignore]
    fn synthetic_alpha158_benchmark() -> qweave_core::Result<()> {
        // Alpha158 counterpart to `synthetic_alpha_benchmark`: this set leans on
        // rolling quantiles (QTLU/QTLD) and ts_argmin/argmax (IMAX/IMIN) over long
        // windows, so it is the load that exercises those kernels — the WQ101 set
        // does not. Same env knobs and A/B protocol.
        let n_symbols = bench_env_usize("QWEAVE_BENCH_SYMBOLS", 200);
        let n_times = bench_env_usize("QWEAVE_BENCH_TIMES", 260);
        let repeats = bench_env_usize("QWEAVE_BENCH_REPEATS", 3);
        let df = synthetic_alpha_bench_frame(n_symbols, n_times)?;
        let mut alphas = qlib_alpha158();
        alphas.sort_by(|(lhs, _), (rhs, _)| lhs.cmp(rhs));

        println!(
            "manual run: QWEAVE_BENCH_SYMBOLS={n_symbols} QWEAVE_BENCH_TIMES={n_times} \
             QWEAVE_BENCH_REPEATS={repeats} cargo test -p qweave-factors \
             synthetic_alpha158_benchmark -- --ignored --nocapture"
        );

        let started = Instant::now();
        let mut total_rows = 0usize;
        for _ in 0..repeats {
            let out = memory_frame(compute_alphas(df.clone(), options(), alphas.clone(), None)?)?;
            total_rows += out.height();
        }
        let elapsed = started.elapsed();
        println!(
            "compute_alphas alpha158: symbols={n_symbols} times={n_times} \
             alphas={} repeats={repeats} rows={total_rows} elapsed={elapsed:?} \
             per_run={:?}",
            alphas.len(),
            elapsed / repeats as u32
        );

        Ok(())
    }

    #[test]
    fn all_alphas_golden_matches_frozen_baseline() -> qweave_core::Result<()> {
        // Phase 0 safety net: every registered alpha computed on a fixed deterministic
        // panel must stay numerically stable across the 0.2.x optimization phases.
        // Baseline frozen at v0.1.0. Re-bless only for an intentional output change:
        //   GOLDEN_BLESS=1 cargo test -p qweave-factors all_alphas_golden -- --nocapture
        let n_symbols = 40;
        let n_times = 700;
        let df = synthetic_alpha_bench_frame(n_symbols, n_times)?;
        let alpha_names = worldquant_alpha_names_sorted();

        let observation_times = Series::new(
            "time".into(),
            [(n_times - 2) as i64, (n_times - 1) as i64, n_times as i64],
        );
        let mut out = memory_frame(compute_alpha_names(
            df,
            options(),
            alpha_names,
            observation_times,
            None,
        )?)?;

        let fixture = format!(
            "{}/tests/fixtures/golden_alphas.parquet",
            env!("CARGO_MANIFEST_DIR")
        );

        if env::var("GOLDEN_BLESS").is_ok() {
            std::fs::create_dir_all(
                std::path::Path::new(&fixture)
                    .parent()
                    .expect("fixture path has a parent"),
            )
            .expect("create fixtures dir");
            let file = std::fs::File::create(&fixture).expect("create golden fixture");
            ParquetWriter::new(file)
                .finish(&mut out)
                .expect("write golden fixture");
            println!("blessed golden fixture: {fixture} ({} rows)", out.height());
            return Ok(());
        }

        let baseline_all = ParquetReader::new(
            std::fs::File::open(&fixture)
                .expect("golden fixture exists (run once with GOLDEN_BLESS=1)"),
        )
        .finish()
        .expect("read golden fixture");
        let baseline_columns = column_names(&out);
        let baseline = baseline_all.select(baseline_columns.iter().map(String::as_str))?;

        assert_golden_within_tol(&out, &baseline, 1e-8, 1e-8);
        Ok(())
    }

    #[test]
    fn qlib_alpha158_golden_matches_frozen_baseline() -> qweave_core::Result<()> {
        // Separate Alpha158 fixture: re-bless only for intentional Alpha158 output changes:
        //   QLIB_GOLDEN_BLESS=1 cargo test -p qweave-factors qlib_alpha158_golden -- --nocapture
        let n_symbols = 12;
        let n_times = 180;
        let df = synthetic_alpha_bench_frame(n_symbols, n_times)?;
        let alpha_names = qlib_alpha158()
            .into_iter()
            .map(|(name, _)| name)
            .collect::<Vec<_>>();

        let observation_times = Series::new(
            "time".into(),
            [(n_times - 2) as i64, (n_times - 1) as i64, n_times as i64],
        );
        let mut out = memory_frame(compute_alphas(df, options(), qlib_alpha158(), None)?)?;
        out = sample_observation_times(out, "time", observation_times)?;

        let expected_columns = ["time".to_string(), "asset".to_string()]
            .into_iter()
            .chain(alpha_names)
            .collect::<Vec<_>>();
        assert_eq!(column_names(&out), expected_columns);

        let fixture = format!(
            "{}/tests/fixtures/golden_qlib_alpha158.parquet",
            env!("CARGO_MANIFEST_DIR")
        );

        if env::var("QLIB_GOLDEN_BLESS").is_ok() {
            std::fs::create_dir_all(
                std::path::Path::new(&fixture)
                    .parent()
                    .expect("fixture path has a parent"),
            )
            .expect("create fixtures dir");
            let file = std::fs::File::create(&fixture).expect("create golden fixture");
            ParquetWriter::new(file)
                .finish(&mut out)
                .expect("write golden fixture");
            println!(
                "blessed qlib Alpha158 golden fixture: {fixture} ({} rows)",
                out.height()
            );
            return Ok(());
        }

        let baseline = ParquetReader::new(
            std::fs::File::open(&fixture)
                .expect("qlib Alpha158 golden fixture exists (run once with QLIB_GOLDEN_BLESS=1)"),
        )
        .finish()
        .expect("read qlib Alpha158 golden fixture");

        assert_golden_within_tol(&out, &baseline, 1e-8, 1e-8);
        Ok(())
    }

    fn assert_golden_within_tol(actual: &DataFrame, baseline: &DataFrame, atol: f64, rtol: f64) {
        assert_eq!(
            column_names(actual),
            column_names(baseline),
            "golden columns drifted; re-bless with GOLDEN_BLESS=1 if intentional"
        );
        assert_eq!(
            actual.height(),
            baseline.height(),
            "golden row count drifted"
        );

        for name in actual.get_column_names() {
            let a = actual.column(name).expect("actual column");
            let b = baseline.column(name).expect("baseline column");

            if name.as_str() == "time" || name.as_str() == "asset" {
                assert!(
                    a.as_materialized_series()
                        .equals(b.as_materialized_series()),
                    "golden structural column {name} drifted"
                );
                continue;
            }

            let a = a.try_f64().expect("alpha column is f64");
            let b = b.try_f64().expect("baseline alpha column is f64");
            for idx in 0..a.len() {
                let av = a.get(idx).unwrap_or(f64::NAN);
                let bv = b.get(idx).unwrap_or(f64::NAN);
                match (av.is_nan(), bv.is_nan()) {
                    (true, true) => {}
                    (false, false) => {
                        let diff = (av - bv).abs();
                        assert!(
                            diff <= atol + rtol * bv.abs(),
                            "golden drift in {name}[{idx}]: actual={av} baseline={bv} diff={diff}"
                        );
                    }
                    _ => panic!(
                        "golden NaN-position drift in {name}[{idx}]: actual={av} baseline={bv}"
                    ),
                }
            }
        }
    }

    fn memory_frame(result: ComputeResult) -> qweave_core::Result<DataFrame> {
        match result {
            ComputeResult::Memory(df) => Ok(df),
            ComputeResult::File(_) => panic!("expected memory result"),
        }
    }

    fn compute_alpha_names(
        df: DataFrame,
        options: PanelOptions,
        alpha_names: Vec<String>,
        observation_times: Series,
        output_path: Option<&str>,
    ) -> qweave_core::Result<ComputeResult> {
        assert!(
            output_path.is_none(),
            "test helper only supports memory mode"
        );
        let mut by_name = worldquant_alpha_map();
        let alphas = alpha_names
            .into_iter()
            .map(|name| {
                let expr = by_name
                    .remove(&name)
                    .ok_or_else(|| QWeaveError::UnknownFactor(name.clone()))?;
                Ok((name, expr))
            })
            .collect::<qweave_core::Result<Vec<_>>>()?;
        let full = memory_frame(compute_alphas(df, options.clone(), alphas, None)?)?;
        Ok(ComputeResult::Memory(sample_observation_times(
            full,
            &options.time_col,
            observation_times,
        )?))
    }

    #[allow(clippy::mutable_key_type)]
    fn sample_observation_times(
        df: DataFrame,
        time_col: &str,
        observation_times: Series,
    ) -> qweave_core::Result<DataFrame> {
        let time = df.column(time_col)?;
        let observations = observation_times.cast(time.dtype())?;
        let mut indices = Vec::new();
        for obs_index in 0..observations.len() {
            let observation = observations.get(obs_index)?.into_static();
            for row in 0..df.height() {
                if time.get(row)?.into_static() == observation {
                    indices.push(row as IdxSize);
                }
            }
        }
        Ok(df.take(&IdxCa::from_vec("idx".into(), indices))?)
    }

    fn options() -> PanelOptions {
        PanelOptions {
            symbol_col: "asset".to_string(),
            time_col: "time".to_string(),
        }
    }

    fn bench_env_usize(name: &str, default: usize) -> usize {
        env::var(name)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    }

    fn synthetic_alpha_bench_frame(
        n_symbols: usize,
        n_times: usize,
    ) -> qweave_core::Result<DataFrame> {
        let n_rows = n_symbols * n_times;
        let mut assets = Vec::with_capacity(n_rows);
        let mut times = Vec::with_capacity(n_rows);
        let mut open = Vec::with_capacity(n_rows);
        let mut close = Vec::with_capacity(n_rows);
        let mut high = Vec::with_capacity(n_rows);
        let mut low = Vec::with_capacity(n_rows);
        let mut volume = Vec::with_capacity(n_rows);
        let mut vwap = Vec::with_capacity(n_rows);
        let mut cap = Vec::with_capacity(n_rows);
        let mut sector = Vec::with_capacity(n_rows);
        let mut industry = Vec::with_capacity(n_rows);
        let mut subindustry = Vec::with_capacity(n_rows);

        for symbol_idx in 0..n_symbols {
            for time_idx in 1..=n_times {
                let symbol = symbol_idx as f64 + 1.0;
                let time = time_idx as f64;
                let low_variance = symbol_idx < n_symbols.min(8);
                let base = if low_variance {
                    1_000.0 + symbol * 0.1
                } else {
                    symbol * 10.0 + time * 0.2
                };
                let close_value = if low_variance {
                    base * (1.0 + ((time_idx % 11) as f64 - 5.0) * 0.00001)
                } else {
                    base * (1.0 + ((time_idx % 11) as f64 - 5.0) * 0.001)
                };
                let high_value = if low_variance {
                    close_value + base * 0.00003
                } else {
                    base.max(close_value) + 1.0 + symbol_idx as f64 * 0.001
                };
                let low_value = if low_variance {
                    close_value - base * 0.00003
                } else {
                    base.min(close_value) - 1.0
                };
                let volume_value = 1_000.0 + symbol * 3.0 + time * 5.0;

                assets.push(format!("S{symbol_idx:04}"));
                times.push(time_idx as i64);
                open.push(base);
                close.push(close_value);
                high.push(high_value);
                low.push(low_value);
                volume.push(volume_value);
                vwap.push((high_value + low_value + close_value) / 3.0);
                cap.push(close_value * (1_000_000.0 + symbol * 10_000.0));
                sector.push((symbol_idx % 2) as i32);
                industry.push((symbol_idx % 3) as i32);
                subindustry.push((symbol_idx % 3) as i32);
            }
        }

        Ok(df!(
            "asset" => assets,
            "time" => times,
            "open" => open,
            "close" => close,
            "high" => high,
            "low" => low,
            "volume" => volume,
            "vwap" => vwap,
            "cap" => cap,
            "sector" => sector,
            "industry" => industry,
            "subindustry" => subindustry,
        )?)
    }

    struct Alpha8Fixture {
        df: DataFrame,
        n_symbols: usize,
        n_times: usize,
        open: Vec<f64>,
        close: Vec<f64>,
    }

    fn alpha8_fixture() -> qweave_core::Result<Alpha8Fixture> {
        let symbols = ["A", "B", "C", "D", "E"];
        let n_symbols = symbols.len();
        let n_times = 18;
        let mut assets = Vec::new();
        let mut times = Vec::new();
        let mut open_values = Vec::new();
        let mut close_values = Vec::new();
        let mut open = vec![f64::NAN; n_symbols * n_times];
        let mut close = vec![f64::NAN; n_symbols * n_times];

        for (symbol_idx, symbol) in symbols.iter().enumerate() {
            for day in 1..=18 {
                if *symbol == "C" && day < 9 {
                    continue;
                }
                if *symbol == "E" && day == 18 {
                    continue;
                }

                let (raw_open, raw_close) = if *symbol == "D" && day == 8 {
                    (f64::NAN, f64::NAN)
                } else {
                    let raw_open = open_value(symbol_idx, day);
                    (raw_open, raw_open * (1.0 + return_rate(symbol_idx, day)))
                };
                let idx = symbol_idx * n_times + (day as usize - 1);
                open[idx] = raw_open;
                close[idx] = raw_close;
                assets.push((*symbol).to_string());
                times.push(day as i64);
                open_values.push(raw_open);
                close_values.push(raw_close);
            }
        }

        Ok(Alpha8Fixture {
            df: df!(
                "asset" => assets,
                "time" => times,
                "open" => open_values,
                "close" => close_values,
            )?,
            n_symbols,
            n_times,
            open,
            close,
        })
    }

    struct PhaseBFixture {
        df: DataFrame,
        n_symbols: usize,
        n_times: usize,
        open: Vec<f64>,
        high: Vec<f64>,
        low: Vec<f64>,
        close: Vec<f64>,
        volume: Vec<f64>,
    }

    fn phase_b_fixture() -> qweave_core::Result<PhaseBFixture> {
        let symbols = ["A", "B", "C", "D", "E"];
        let n_symbols = symbols.len();
        let n_times = 12;
        let mut assets = Vec::new();
        let mut times = Vec::new();
        let mut open_values = Vec::new();
        let mut high_values = Vec::new();
        let mut low_values = Vec::new();
        let mut close_values = Vec::new();
        let mut volume_values = Vec::new();
        let mut open = vec![f64::NAN; n_symbols * n_times];
        let mut high = vec![f64::NAN; n_symbols * n_times];
        let mut low = vec![f64::NAN; n_symbols * n_times];
        let mut close = vec![f64::NAN; n_symbols * n_times];
        let mut volume = vec![f64::NAN; n_symbols * n_times];

        for (symbol_idx, symbol) in symbols.iter().enumerate() {
            for day in 1..=n_times {
                let idx = symbol_idx * n_times + (day - 1);
                let raw_close = 50.0
                    + ((day * (symbol_idx + 2)) % 7) as f64
                    + symbol_idx as f64 * 0.3
                    + day as f64 * 0.2;
                let raw_open = raw_close - 0.5 + ((day + symbol_idx) % 3) as f64 * 0.2;
                let raw_volume = 1000.0
                    + ((day * (symbol_idx + 3) + symbol_idx) % 11) as f64 * 10.0
                    + symbol_idx as f64 * 7.0
                    + day as f64 * 3.0;
                let raw_high = raw_open.max(raw_close) + 1.0 + symbol_idx as f64 * 0.1;
                let raw_low = raw_open.min(raw_close) - 1.0 - day as f64 * 0.01;

                open[idx] = raw_open;
                high[idx] = raw_high;
                low[idx] = raw_low;
                close[idx] = raw_close;
                volume[idx] = raw_volume;
                assets.push((*symbol).to_string());
                times.push(day as i64);
                open_values.push(raw_open);
                high_values.push(raw_high);
                low_values.push(raw_low);
                close_values.push(raw_close);
                volume_values.push(raw_volume);
            }
        }

        Ok(PhaseBFixture {
            df: df!(
                "asset" => assets,
                "time" => times,
                "open" => open_values,
                "high" => high_values,
                "low" => low_values,
                "close" => close_values,
                "volume" => volume_values,
            )?,
            n_symbols,
            n_times,
            open,
            high,
            low,
            close,
            volume,
        })
    }

    fn open_value(symbol_idx: usize, day: i32) -> f64 {
        (symbol_idx as f64 + 1.0) * 20.0 + day as f64 * (symbol_idx as f64 + 1.5)
    }

    fn return_rate(symbol_idx: usize, day: i32) -> f64 {
        0.002 * (symbol_idx as f64 + 1.0) + 0.0005 * day as f64
    }

    fn reference_alpha8(fixture: &Alpha8Fixture) -> Vec<f64> {
        let returns = reference_returns(&fixture.close, fixture.n_symbols, fixture.n_times);
        let sum_open = reference_ts_sum(&fixture.open, fixture.n_symbols, fixture.n_times, 5);
        let sum_returns = reference_ts_sum(&returns, fixture.n_symbols, fixture.n_times, 5);
        let inner = sum_open
            .into_iter()
            .zip(sum_returns)
            .map(|(open, returns)| open * returns)
            .collect::<Vec<_>>();
        let delayed_inner = reference_delay(&inner, fixture.n_symbols, fixture.n_times, 10);
        let term = inner
            .into_iter()
            .zip(delayed_inner)
            .map(|(inner, delayed)| inner - delayed)
            .collect::<Vec<_>>();

        reference_rank_time(&term, fixture.n_symbols, fixture.n_times, 17)
            .into_iter()
            .map(|value| -value)
            .collect()
    }

    fn reference_alpha6(fixture: &PhaseBFixture, time: usize) -> Vec<f64> {
        (0..fixture.n_symbols)
            .map(|symbol| {
                -reference_correlation_at(
                    &fixture.open,
                    &fixture.volume,
                    fixture.n_times,
                    symbol,
                    time,
                    10,
                )
            })
            .collect()
    }

    fn reference_alpha12(fixture: &PhaseBFixture, time: usize) -> Vec<f64> {
        (0..fixture.n_symbols)
            .map(|symbol| {
                let idx = symbol * fixture.n_times + time;
                let prev = idx - 1;
                let volume_delta = fixture.volume[idx] - fixture.volume[prev];
                let close_delta = fixture.close[idx] - fixture.close[prev];
                volume_delta.signum() * -close_delta
            })
            .collect()
    }

    fn reference_alpha13(fixture: &PhaseBFixture, time: usize) -> Vec<f64> {
        let close_rank =
            reference_rank_all_times(&fixture.close, fixture.n_symbols, fixture.n_times);
        let volume_rank =
            reference_rank_all_times(&fixture.volume, fixture.n_symbols, fixture.n_times);
        let covariances = (0..fixture.n_symbols)
            .map(|symbol| {
                reference_covariance_at(&close_rank, &volume_rank, fixture.n_times, symbol, time, 5)
            })
            .collect::<Vec<_>>();
        rank_values(&covariances)
            .into_iter()
            .map(|value| -value)
            .collect()
    }

    fn reference_alpha101(fixture: &PhaseBFixture, time: usize) -> Vec<f64> {
        (0..fixture.n_symbols)
            .map(|symbol| {
                let idx = symbol * fixture.n_times + time;
                (fixture.close[idx] - fixture.open[idx])
                    / (fixture.high[idx] - fixture.low[idx] + 0.001)
            })
            .collect()
    }

    fn reference_returns(close: &[f64], n_symbols: usize, n_times: usize) -> Vec<f64> {
        let mut out = vec![f64::NAN; close.len()];
        for symbol in 0..n_symbols {
            let offset = symbol * n_times;
            for time in 1..n_times {
                out[offset + time] = close[offset + time] / close[offset + time - 1] - 1.0;
            }
        }
        out
    }

    fn reference_delay(values: &[f64], n_symbols: usize, n_times: usize, days: usize) -> Vec<f64> {
        let mut out = vec![f64::NAN; values.len()];
        for symbol in 0..n_symbols {
            let offset = symbol * n_times;
            for time in days..n_times {
                out[offset + time] = values[offset + time - days];
            }
        }
        out
    }

    fn reference_ts_sum(values: &[f64], n_symbols: usize, n_times: usize, days: usize) -> Vec<f64> {
        let mut out = vec![f64::NAN; values.len()];
        for symbol in 0..n_symbols {
            let offset = symbol * n_times;
            for time in days - 1..n_times {
                let window = &values[offset + time + 1 - days..=offset + time];
                if window.iter().all(|value| !value.is_nan()) {
                    out[offset + time] = window.iter().sum();
                }
            }
        }
        out
    }

    fn reference_rank_all_times(values: &[f64], n_symbols: usize, n_times: usize) -> Vec<f64> {
        let mut out = vec![f64::NAN; values.len()];
        for time in 0..n_times {
            let ranks = reference_rank_time(values, n_symbols, n_times, time);
            for (symbol, rank) in ranks.into_iter().enumerate() {
                out[symbol * n_times + time] = rank;
            }
        }
        out
    }

    fn reference_correlation_at(
        lhs: &[f64],
        rhs: &[f64],
        n_times: usize,
        symbol: usize,
        time: usize,
        days: usize,
    ) -> f64 {
        if time + 1 < days {
            return f64::NAN;
        }
        let start = symbol * n_times + time + 1 - days;
        let end = symbol * n_times + time + 1;
        let lhs = &lhs[start..end];
        let rhs = &rhs[start..end];
        if lhs.iter().any(|value| value.is_nan()) || rhs.iter().any(|value| value.is_nan()) {
            return f64::NAN;
        }
        let lhs_mean = lhs.iter().sum::<f64>() / lhs.len() as f64;
        let rhs_mean = rhs.iter().sum::<f64>() / rhs.len() as f64;
        let mut covariance = 0.0;
        let mut lhs_variance = 0.0;
        let mut rhs_variance = 0.0;
        for (lhs, rhs) in lhs.iter().zip(rhs) {
            let lhs_centered = lhs - lhs_mean;
            let rhs_centered = rhs - rhs_mean;
            covariance += lhs_centered * rhs_centered;
            lhs_variance += lhs_centered * lhs_centered;
            rhs_variance += rhs_centered * rhs_centered;
        }
        if lhs_variance == 0.0 || rhs_variance == 0.0 {
            f64::NAN
        } else {
            covariance / (lhs_variance.sqrt() * rhs_variance.sqrt())
        }
    }

    fn reference_covariance_at(
        lhs: &[f64],
        rhs: &[f64],
        n_times: usize,
        symbol: usize,
        time: usize,
        days: usize,
    ) -> f64 {
        if time + 1 < days || days < 2 {
            return f64::NAN;
        }
        let start = symbol * n_times + time + 1 - days;
        let end = symbol * n_times + time + 1;
        let lhs = &lhs[start..end];
        let rhs = &rhs[start..end];
        if lhs.iter().any(|value| value.is_nan()) || rhs.iter().any(|value| value.is_nan()) {
            return f64::NAN;
        }
        let lhs_mean = lhs.iter().sum::<f64>() / lhs.len() as f64;
        let rhs_mean = rhs.iter().sum::<f64>() / rhs.len() as f64;
        lhs.iter()
            .zip(rhs)
            .map(|(lhs, rhs)| (lhs - lhs_mean) * (rhs - rhs_mean))
            .sum::<f64>()
            / (lhs.len() as f64 - 1.0)
    }

    fn rank_values(values: &[f64]) -> Vec<f64> {
        let mut out = vec![f64::NAN; values.len()];
        let mut present = values
            .iter()
            .enumerate()
            .filter_map(|(idx, value)| (!value.is_nan()).then_some((idx, *value)))
            .collect::<Vec<_>>();
        present.sort_by(|(_, lhs), (_, rhs)| lhs.partial_cmp(rhs).unwrap_or(Ordering::Equal));

        let count = present.len() as f64;
        let mut start = 0usize;
        while start < present.len() {
            let mut end = start + 1;
            while end < present.len() && present[end].1 == present[start].1 {
                end += 1;
            }
            let pct = (start + 1 + end) as f64 / 2.0 / count;
            for (idx, _) in &present[start..end] {
                out[*idx] = pct;
            }
            start = end;
        }
        out
    }

    fn reference_rank_time(
        values: &[f64],
        n_symbols: usize,
        n_times: usize,
        time: usize,
    ) -> Vec<f64> {
        let mut out = vec![f64::NAN; n_symbols];
        let mut present = (0..n_symbols)
            .filter_map(|symbol| {
                let value = values[symbol * n_times + time];
                (!value.is_nan()).then_some((symbol, value))
            })
            .collect::<Vec<_>>();
        present.sort_by(|(_, lhs), (_, rhs)| lhs.partial_cmp(rhs).unwrap_or(Ordering::Equal));

        let count = present.len() as f64;
        let mut start = 0usize;
        while start < present.len() {
            let mut end = start + 1;
            while end < present.len() && present[end].1 == present[start].1 {
                end += 1;
            }
            let pct = (start + 1 + end) as f64 / 2.0 / count;
            for (symbol, _) in &present[start..end] {
                out[*symbol] = pct;
            }
            start = end;
        }
        out
    }

    fn time_asset_rows(df: &DataFrame) -> qweave_core::Result<Vec<(i64, String)>> {
        let times = df.column("time")?.try_i64().expect("time is i64");
        let assets = df.column("asset")?.try_str().expect("asset is string");
        Ok(times
            .into_no_null_iter()
            .zip(assets.iter())
            .map(|(time, asset)| (time, asset.expect("asset has no nulls").to_string()))
            .collect())
    }

    fn column_values(df: &DataFrame, name: &str) -> qweave_core::Result<Vec<f64>> {
        Ok(df
            .column(name)?
            .try_f64()
            .expect("alpha column is f64")
            .into_no_null_iter()
            .collect())
    }

    fn column_names(df: &DataFrame) -> Vec<String> {
        df.get_column_names()
            .iter()
            .map(|name| name.to_string())
            .collect()
    }

    fn assert_f64_eq(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() < 1e-12,
            "actual {actual}, expected {expected}"
        );
    }
}
