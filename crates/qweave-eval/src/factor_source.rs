use std::collections::BTreeSet;
use std::fs::File;

use polars::prelude::*;
use qweave_core::{PanelOptions, QWeaveError};

use crate::error::{EvalError, Result};

/// A parquet panel (e.g. `compute_alphas(output_path=...)` output) supplying
/// factor columns lazily, so a thousand-factor evaluation never materializes
/// the full wide frame in memory.
///
/// The parquet must cover exactly the same `(symbol, time)` panel as the
/// DataFrame passed to `evaluate` (verified once up front); factor columns are
/// then read one batch at a time — via parquet column projection, so only the
/// batch's columns are touched — and sorted into TN order to match the grid.
#[derive(Debug)]
pub(crate) struct FactorSource {
    path: String,
    panel: PanelOptions,
}

impl FactorSource {
    /// Open `path` and verify it covers the same panel as `df`.
    pub(crate) fn open(
        path: &str,
        panel: &PanelOptions,
        df: &DataFrame,
        factor_cols: &[String],
    ) -> Result<Self> {
        // Presence check up front; f64 dtype is enforced when the batch is read.
        let schema = ParquetReader::new(File::open(path)?).schema()?;
        for name in [&panel.symbol_col, &panel.time_col]
            .into_iter()
            .chain(factor_cols)
        {
            if schema.index_of(name).is_none() {
                return Err(EvalError::Core(QWeaveError::MissingColumn(name.clone())));
            }
        }

        let source = Self {
            path: path.to_string(),
            panel: panel.clone(),
        };
        source.verify_panel(df)?;
        Ok(source)
    }

    /// The parquet's `(time, symbol)` keys, sorted TN, must equal the frame's.
    fn verify_panel(&self, df: &DataFrame) -> Result<()> {
        let keys = [self.panel.time_col.as_str(), self.panel.symbol_col.as_str()];
        let parquet_keys = self
            .read_projection(&[])?
            .sort(keys, SortMultipleOptions::default())?;
        let frame_keys = df
            .select(keys)?
            .sort(keys, SortMultipleOptions::default())?;
        if !parquet_keys.equals(&frame_keys) {
            return Err(EvalError::FactorSourcePanelMismatch);
        }
        Ok(())
    }

    /// Read `batch` factor columns as TN-ordered (time-major, symbol-ascending)
    /// dense vectors.
    pub(crate) fn read_batch(&self, batch: &[String]) -> Result<Vec<Vec<f64>>> {
        let keys = [self.panel.time_col.as_str(), self.panel.symbol_col.as_str()];
        let sorted = self
            .read_projection(batch)?
            .sort(keys, SortMultipleOptions::default())?;
        batch
            .iter()
            .map(|name| {
                let column = sorted.column(name)?;
                let values = column.try_f64().ok_or_else(|| EvalError::DTypeMismatch {
                    column: name.clone(),
                    expected: "f64",
                    actual: column.dtype().to_string(),
                })?;
                Ok(values.iter().map(|v| v.unwrap_or(f64::NAN)).collect())
            })
            .collect()
    }

    /// Read the key columns plus `extra`, projecting so unlisted (factor)
    /// columns are never decoded.
    fn read_projection(&self, extra: &[String]) -> Result<DataFrame> {
        let mut columns = vec![self.panel.time_col.clone(), self.panel.symbol_col.clone()];
        columns.extend(extra.iter().cloned());
        Ok(ParquetReader::new(File::open(&self.path)?)
            .with_columns(Some(columns))
            .finish()?)
    }
}

/// Deduplicate + validate factor names when they come from a parquet source
/// (they need not exist in `df`, so the in-frame dtype check does not apply).
pub(crate) fn validate_source_factor_cols(
    factor_cols: &[String],
    label_pairs: &[(String, usize)],
) -> Result<()> {
    if factor_cols.is_empty() {
        return Err(EvalError::BadFactorColumns("<empty>".to_string()));
    }
    let mut seen = BTreeSet::new();
    for name in factor_cols {
        if !seen.insert(name.as_str()) || label_pairs.iter().any(|(label, _)| label == name) {
            return Err(EvalError::BadFactorColumns(name.clone()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn panel() -> PanelOptions {
        PanelOptions {
            symbol_col: "asset".to_string(),
            time_col: "time".to_string(),
        }
    }

    fn write_parquet(df: &mut DataFrame, tag: &str) -> String {
        let path = std::env::temp_dir().join(format!(
            "qweave-factor-source-{}-{tag}.parquet",
            std::process::id(),
        ));
        let file = std::fs::File::create(&path).unwrap();
        ParquetWriter::new(file).finish(df).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn reads_batch_in_tn_order() -> Result<()> {
        // Parquet deliberately not in TN order.
        let mut parquet = df!(
            "time" => [2i64, 1, 2, 1],
            "asset" => ["B", "A", "A", "B"],
            "f1" => [22.0, 10.0, 12.0, 20.0],
        )?;
        let path = write_parquet(&mut parquet, "tn");
        let df = df!(
            "asset" => ["A", "B", "A", "B"],
            "time" => [1i64, 1, 2, 2],
            "ret_1" => [0.1, 0.2, 0.3, 0.4],
        )?;

        let source = FactorSource::open(&path, &panel(), &df, &["f1".to_string()])?;
        let batch = source.read_batch(&["f1".to_string()])?;

        // TN order: (1,A), (1,B), (2,A), (2,B).
        assert_eq!(batch[0], [10.0, 20.0, 12.0, 22.0]);
        std::fs::remove_file(&path).ok();
        Ok(())
    }

    #[test]
    fn rejects_panel_mismatch_and_missing_columns() -> Result<()> {
        let mut parquet = df!(
            "time" => [1i64, 2],
            "asset" => ["A", "A"],
            "f1" => [1.0, 2.0],
        )?;
        let path = write_parquet(&mut parquet, "mismatch");

        // Different panel (asset B instead of a second A row).
        let df = df!(
            "asset" => ["A", "B"],
            "time" => [1i64, 1],
        )?;
        let err = FactorSource::open(&path, &panel(), &df, &["f1".to_string()]).unwrap_err();
        assert!(matches!(err, EvalError::FactorSourcePanelMismatch));

        let df_ok = df!(
            "asset" => ["A", "A"],
            "time" => [1i64, 2],
        )?;
        let err =
            FactorSource::open(&path, &panel(), &df_ok, &["missing".to_string()]).unwrap_err();
        assert!(matches!(
            err,
            EvalError::Core(QWeaveError::MissingColumn(_))
        ));

        std::fs::remove_file(&path).ok();
        Ok(())
    }
}
