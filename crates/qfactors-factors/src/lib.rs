use qfactors_macros::factor;

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
    use std::collections::HashMap;

    use polars::prelude::*;
    use qfactors_core::{
        ComputePanelOptions, ComputeResult, Result, compute_panel, factor_catalog,
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
}
