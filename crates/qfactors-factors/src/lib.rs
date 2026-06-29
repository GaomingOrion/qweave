use qfactors_macros::factor;

pub mod alphas;
pub mod worldquant101;

pub fn ensure_linked() {}

#[factor(window = 60)]
pub fn ret(open: &[f64], close: &[f64]) -> f64 {
    close[close.len() - 1] / open[0] - 1.0
}

#[factor(
    windows = [20, 60],
    params = [
        { name = "k15", k = 1.5 },
        { name = "k20", k = 2.0 },
    ]
)]
pub fn volume_breakout(volume: &[f64], k: f64) -> f64 {
    let last = volume[volume.len() - 1];
    let mean = volume.iter().sum::<f64>() / volume.len() as f64;
    if last > k * mean { 1.0 } else { 0.0 }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;
    use std::collections::HashMap;
    use std::env;
    use std::time::Instant;

    use polars::prelude::*;
    use qfactors_core::{
        ComputePanelOptions, ComputeResult, Result, alpha_registry, compute_alphas, compute_panel,
        factor_catalog,
    };

    use super::*;

    #[factor(windows = [2, 3])]
    fn delta(close: &[f64]) -> f64 {
        close[close.len() - 1] - close[0]
    }

    #[factor(window = 2, outputs = ["first", "last"])]
    fn bounds(close: &[f64]) -> (f64, f64) {
        (close[0], close[close.len() - 1])
    }

    #[factor(window = 2)]
    fn checked_delta(close: &[f64]) -> Result<f64> {
        Ok(close[close.len() - 1] - close[0])
    }

    #[test]
    fn ret_descriptor_computes_valid_and_insufficient_windows() -> qfactors_core::Result<()> {
        let asset = (0..61).map(|_| "A").chain(["B"]).collect::<Vec<_>>();
        let time = (1i64..=61).chain([61]).collect::<Vec<_>>();
        let open = (1..=61)
            .map(|value| value as f64)
            .chain([100.0])
            .collect::<Vec<_>>();
        let close = (2..=62)
            .map(|value| value as f64)
            .chain([110.0])
            .collect::<Vec<_>>();
        let df = df!(
            "asset" => asset,
            "time" => time,
            "open" => open,
            "close" => close,
        )?;
        let out = memory_frame(compute_panel(
            df,
            options(),
            vec!["ret".to_string()],
            Series::new("time".into(), [61i64]),
            None,
        )?)?;
        let values = out
            .column("ret")?
            .try_f64()
            .expect("ret is f64")
            .into_no_null_iter()
            .collect::<Vec<_>>();

        assert_eq!(values[0], 62.0 / 2.0 - 1.0);
        assert!(values[1].is_nan());

        Ok(())
    }

    #[test]
    fn macro_generated_factors_support_windows_outputs_and_result() -> qfactors_core::Result<()> {
        let df = df!(
            "asset" => ["A", "A", "A"],
            "time" => [1i64, 2, 3],
            "open" => [10.0, 11.0, 12.0],
            "close" => [20.0, 23.0, 27.0],
        )?;
        let out = memory_frame(compute_panel(
            df,
            options(),
            vec![
                "delta_2".to_string(),
                "delta_3".to_string(),
                "bounds".to_string(),
                "checked_delta".to_string(),
            ],
            Series::new("time".into(), [3i64]),
            None,
        )?)?;

        assert_eq!(
            out.column("delta_2")?
                .try_f64()
                .expect("delta_2 is f64")
                .get(0),
            Some(4.0)
        );
        assert_eq!(
            out.column("delta_3")?
                .try_f64()
                .expect("delta_3 is f64")
                .get(0),
            Some(7.0)
        );
        assert_eq!(
            out.column("bounds.first")?
                .try_f64()
                .expect("bounds.first is f64")
                .get(0),
            Some(23.0)
        );
        assert_eq!(
            out.column("bounds.last")?
                .try_f64()
                .expect("bounds.last is f64")
                .get(0),
            Some(27.0)
        );
        assert_eq!(
            out.column("checked_delta")?
                .try_f64()
                .expect("checked_delta is f64")
                .get(0),
            Some(4.0)
        );

        Ok(())
    }

    #[test]
    fn macro_generated_param_factors_are_cataloged_and_computed() -> qfactors_core::Result<()> {
        let volume = (1..=60)
            .map(|idx| if idx == 60 { 100.0 } else { 10.0 })
            .collect::<Vec<_>>();
        let df = df!(
            "asset" => ["A"; 60],
            "time" => (1i64..=60).collect::<Vec<_>>(),
            "open" => vec![1.0; 60],
            "close" => vec![2.0; 60],
            "volume" => volume,
        )?;
        let catalog = factor_catalog()?;
        let row_idx = catalog
            .column("factor_name")?
            .try_str()
            .expect("factor_name is string")
            .iter()
            .position(|value| value == Some("volume_breakout_20_k15"))
            .expect("volume_breakout_20_k15 is registered");
        assert_eq!(
            catalog
                .column("param_set")?
                .try_str()
                .expect("param_set is string")
                .get(row_idx),
            Some("k15")
        );
        assert_eq!(
            catalog
                .column("param_k")?
                .try_f64()
                .expect("param_k is f64")
                .get(row_idx),
            Some(1.5)
        );

        let out = memory_frame(compute_panel(
            df,
            options(),
            vec![
                "volume_breakout_20_k15".to_string(),
                "volume_breakout_20_k20".to_string(),
                "volume_breakout_60_k15".to_string(),
            ],
            Series::new("time".into(), [60i64]),
            None,
        )?)?;

        assert_eq!(
            out.column("volume_breakout_20_k15")?
                .try_f64()
                .expect("volume_breakout_20_k15 is f64")
                .get(0),
            Some(1.0)
        );
        assert_eq!(
            out.column("volume_breakout_20_k20")?
                .try_f64()
                .expect("volume_breakout_20_k20 is f64")
                .get(0),
            Some(1.0)
        );
        assert_eq!(
            out.column("volume_breakout_60_k15")?
                .try_f64()
                .expect("volume_breakout_60_k15 is f64")
                .get(0),
            Some(1.0)
        );

        Ok(())
    }

    #[test]
    fn alpha8_is_registered() -> qfactors_core::Result<()> {
        assert!(alpha_registry()?.get("alpha8").is_some());
        Ok(())
    }

    #[test]
    fn phase_b_alphas_are_registered() -> qfactors_core::Result<()> {
        let registry = alpha_registry()?;
        for name in [
            "alpha6",
            "alpha12",
            "alpha13",
            "alpha101",
            "group_returns_rank",
            "industry_neutral_close",
        ] {
            assert!(registry.get(name).is_some(), "{name} is registered");
        }
        Ok(())
    }

    #[test]
    fn worldquant101_alphas_are_registered_and_run_on_complete_synthetic_panel()
    -> qfactors_core::Result<()> {
        let registry = alpha_registry()?;
        let alpha_names = (1..=101)
            .map(|idx| format!("alpha{idx}"))
            .collect::<Vec<_>>();
        for name in &alpha_names {
            assert!(registry.get(name).is_some(), "{name} is registered");
        }

        let n_symbols = 6;
        let n_times = 260;
        let out = memory_frame(compute_alphas(
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
    fn alpha8_end_to_end_matches_reference_and_compact_edges() -> qfactors_core::Result<()> {
        let fixture = alpha8_fixture()?;
        let expected = reference_alpha8(&fixture);
        let out = memory_frame(compute_alphas(
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
    fn phase_b_wq_alphas_match_independent_reference() -> qfactors_core::Result<()> {
        let fixture = phase_b_fixture()?;
        let time = fixture.n_times - 1;
        let expected_alpha6 = reference_alpha6(&fixture, time);
        let expected_alpha12 = reference_alpha12(&fixture, time);
        let expected_alpha13 = reference_alpha13(&fixture, time);
        let expected_alpha101 = reference_alpha101(&fixture, time);

        let out = memory_frame(compute_alphas(
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
    fn phase_b_group_alphas_match_independent_reference() -> qfactors_core::Result<()> {
        let out = memory_frame(compute_alphas(
            group_fixture()?,
            options(),
            vec![
                "group_returns_rank".to_string(),
                "industry_neutral_close".to_string(),
            ],
            Series::new("time".into(), [2i64]),
            None,
        )?)?;

        assert_eq!(
            time_asset_rows(&out)?,
            [
                (2, "A".to_string()),
                (2, "B".to_string()),
                (2, "C".to_string()),
                (2, "D".to_string())
            ]
        );
        let group_rank = column_values(&out, "group_returns_rank")?;
        assert_f64_eq(group_rank[0], 0.75);
        assert_f64_eq(group_rank[1], 0.75);
        assert_f64_eq(group_rank[2], 1.0);
        assert!(group_rank[3].is_nan());

        let neutralized = column_values(&out, "industry_neutral_close")?;
        assert_f64_eq(neutralized[0], -5.5);
        assert_f64_eq(neutralized[1], 5.5);
        assert_f64_eq(neutralized[2], 0.0);
        assert!(neutralized[3].is_nan());
        Ok(())
    }

    #[test]
    fn alpha_missing_observation_time_keeps_schema() -> qfactors_core::Result<()> {
        let fixture = alpha8_fixture()?;
        let out = memory_frame(compute_alphas(
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
    fn synthetic_alpha_benchmark() -> qfactors_core::Result<()> {
        let n_symbols = bench_env_usize("QFACTORS_BENCH_SYMBOLS", 200);
        let n_times = bench_env_usize("QFACTORS_BENCH_TIMES", 260);
        let repeats = bench_env_usize("QFACTORS_BENCH_REPEATS", 3);
        let df = synthetic_alpha_bench_frame(n_symbols, n_times)?;
        let mut alpha_names = alpha_registry()?
            .descriptors()
            .map(|descriptor| descriptor.name.to_string())
            .collect::<Vec<_>>();
        alpha_names.sort();

        println!(
            "manual run: QFACTORS_BENCH_SYMBOLS={n_symbols} QFACTORS_BENCH_TIMES={n_times} \
             QFACTORS_BENCH_REPEATS={repeats} cargo test -p qfactors-factors \
             synthetic_alpha_benchmark -- --ignored --nocapture"
        );

        let started = Instant::now();
        let mut total_rows = 0usize;
        for _ in 0..repeats {
            let out = memory_frame(compute_alphas(
                df.clone(),
                options(),
                alpha_names.clone(),
                Series::new("time".into(), [n_times as i64]),
                None,
            )?)?;
            total_rows += out.height();
        }
        let elapsed = started.elapsed();
        println!(
            "compute_alphas synthetic: symbols={n_symbols} times={n_times} \
             alphas={} repeats={repeats} rows={total_rows} elapsed={elapsed:?} \
             per_run={:?}",
            alpha_names.len(),
            elapsed / repeats as u32
        );

        Ok(())
    }

    fn memory_frame(result: ComputeResult) -> qfactors_core::Result<DataFrame> {
        match result {
            ComputeResult::Memory(df) => Ok(df),
            ComputeResult::File(_) => panic!("expected memory result"),
        }
    }

    fn options() -> ComputePanelOptions {
        ComputePanelOptions {
            symbol_col: "asset".to_string(),
            time_col: "time".to_string(),
            column_aliases: HashMap::new(),
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
    ) -> qfactors_core::Result<DataFrame> {
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
                let base = symbol * 10.0 + time * 0.2;
                let close_value = base * (1.0 + ((time_idx % 11) as f64 - 5.0) * 0.001);
                let high_value = base.max(close_value) + 1.0 + symbol_idx as f64 * 0.001;
                let low_value = base.min(close_value) - 1.0;
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
                sector.push((symbol_idx % 2) as f64);
                industry.push((symbol_idx % 3) as f64);
                subindustry.push((symbol_idx % 3) as f64);
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

    fn alpha8_fixture() -> qfactors_core::Result<Alpha8Fixture> {
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

    fn phase_b_fixture() -> qfactors_core::Result<PhaseBFixture> {
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

    fn group_fixture() -> qfactors_core::Result<DataFrame> {
        Ok(df!(
            "asset" => ["A", "A", "B", "B", "C", "C", "D", "D"],
            "time" => [1i64, 2, 1, 2, 1, 2, 1, 2],
            "close" => [10.0, 11.0, 20.0, 22.0, 30.0, 33.0, 40.0, f64::NAN],
            "industry" => [1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0],
        )?)
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

    fn time_asset_rows(df: &DataFrame) -> qfactors_core::Result<Vec<(i64, String)>> {
        let times = df.column("time")?.try_i64().expect("time is i64");
        let assets = df.column("asset")?.try_str().expect("asset is string");
        Ok(times
            .into_no_null_iter()
            .zip(assets.iter())
            .map(|(time, asset)| (time, asset.expect("asset has no nulls").to_string()))
            .collect())
    }

    fn column_values(df: &DataFrame, name: &str) -> qfactors_core::Result<Vec<f64>> {
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
