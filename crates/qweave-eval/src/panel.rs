use std::ops::Range;

use polars::prelude::*;
use qweave_core::{PanelOptions, QWeaveError};

use crate::error::{EvalError, Result};

const ORIG_INDEX: &str = "__qweave_orig_index";

/// Lightweight TN-order panel index for `evaluate`.
///
/// Unlike `CellSet` this sorts the panel once (time-major) and skips the
/// symbol-major pass entirely — evaluation only ever slices by day. Structural
/// guarantees match `build_cellset`: no nulls/NaN in the key columns and no
/// duplicate (symbol, time) pairs.
#[derive(Debug)]
pub(crate) struct TimeIndex {
    pub blocks: Vec<Range<usize>>,
    pub orig_index_tn: Vec<usize>,
    pub times_tn: Column,
    /// Symbol identity per TN row, coded by lexicographic symbol order — so
    /// within a day block (symbols ascending) codes ascend too, and cross-day
    /// set operations are linear merges.
    pub symbol_code_tn: Vec<u32>,
    pub n_symbols: usize,
}

pub(crate) fn build_time_index(df: &DataFrame, panel: &PanelOptions) -> Result<TimeIndex> {
    let symbol = df
        .column(&panel.symbol_col)
        .map_err(|_| EvalError::Core(QWeaveError::MissingColumn(panel.symbol_col.clone())))?;
    let time = df
        .column(&panel.time_col)
        .map_err(|_| EvalError::Core(QWeaveError::MissingColumn(panel.time_col.clone())))?;
    validate_structural(symbol, true)?;
    validate_structural(time, false)?;

    let sorted = df
        .select([panel.time_col.as_str(), panel.symbol_col.as_str()])?
        .with_row_index(ORIG_INDEX.into(), None)?
        .sort(
            [&panel.time_col, &panel.symbol_col],
            SortMultipleOptions::default(),
        )?;
    let orig_index_tn = sorted
        .column(ORIG_INDEX)?
        .as_materialized_series()
        .idx()?
        .into_no_null_iter()
        .map(|idx| idx as usize)
        .collect::<Vec<_>>();
    let times_tn = sorted.column(&panel.time_col)?.clone();

    let time_series = times_tn.as_materialized_series();
    let time_changed = time_series.not_equal_missing(&time_series.shift(1))?;
    let symbol_series = sorted.column(&panel.symbol_col)?.as_materialized_series();
    let symbol_changed = symbol_series.not_equal_missing(&symbol_series.shift(1))?;
    // A duplicate (symbol, time) pair shows up as a consecutive row where
    // neither key changed (row 0 compares against null, hence "changed").
    if !(&time_changed | &symbol_changed).all() {
        return Err(EvalError::Core(QWeaveError::DuplicateSymbolTime {
            symbol_col: panel.symbol_col.clone(),
            time_col: panel.time_col.clone(),
        }));
    }

    let blocks = time_blocks(time_series, &time_changed)?;
    let (symbol_code_tn, n_symbols) = symbol_codes(symbol_series, &symbol_changed)?;

    Ok(TimeIndex {
        blocks,
        orig_index_tn,
        times_tn,
        symbol_code_tn,
        n_symbols,
    })
}

/// Code each TN row's symbol by the symbol's rank in lexicographic order.
///
/// Works off the (time, symbol)-sorted frame: `symbol_changed` marks every
/// first occurrence within a day run, so distinct AnyValues only need hashing
/// once per (day, symbol) run; the final code map is by sorted symbol order.
#[allow(clippy::mutable_key_type)]
fn symbol_codes(symbols: &Series, symbol_changed: &BooleanChunked) -> Result<(Vec<u32>, usize)> {
    use std::collections::HashMap;

    let n = symbols.len();
    // First pass: intern distinct symbols (arbitrary provisional codes).
    let mut interned: HashMap<AnyValue<'static>, u32> = HashMap::new();
    let mut provisional = vec![0u32; n];
    let mut current = 0u32;
    for (row, slot) in provisional.iter_mut().enumerate() {
        if row == 0 || symbol_changed.get(row).unwrap_or(true) {
            let value = symbols.get(row)?.into_static();
            let next = interned.len() as u32;
            current = *interned.entry(value).or_insert(next);
        }
        *slot = current;
    }
    // Second pass: remap provisional codes to lexicographic order.
    let mut pairs: Vec<(AnyValue<'static>, u32)> = interned.into_iter().collect();
    pairs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut remap = vec![0u32; pairs.len()];
    for (rank, (_, provisional_code)) in pairs.iter().enumerate() {
        remap[*provisional_code as usize] = rank as u32;
    }
    let n_symbols = pairs.len();
    for code in &mut provisional {
        *code = remap[*code as usize];
    }
    Ok((provisional, n_symbols))
}

/// Day-block boundaries on the sorted time column. The typed fast paths scan a
/// contiguous physical slice; other dtypes fall back to the change mask.
fn time_blocks(times: &Series, time_changed: &BooleanChunked) -> Result<Vec<Range<usize>>> {
    let physical = times.to_physical_repr();
    match physical.dtype() {
        DataType::Int64 => {
            let ca = physical.i64()?.rechunk();
            Ok(blocks_from_slice(ca.cont_slice().expect("no nulls")))
        }
        DataType::Int32 => {
            let ca = physical.i32()?.rechunk();
            Ok(blocks_from_slice(ca.cont_slice().expect("no nulls")))
        }
        _ => Ok(blocks_from_mask(time_changed)),
    }
}

fn blocks_from_slice<T: PartialEq>(values: &[T]) -> Vec<Range<usize>> {
    let mut blocks = Vec::new();
    let mut start = 0usize;
    for row in 1..values.len() {
        if values[row] != values[row - 1] {
            blocks.push(start..row);
            start = row;
        }
    }
    if !values.is_empty() {
        blocks.push(start..values.len());
    }
    blocks
}

fn blocks_from_mask(time_changed: &BooleanChunked) -> Vec<Range<usize>> {
    let n = time_changed.len();
    let mut blocks = Vec::new();
    let mut start = 0usize;
    for (row, changed) in time_changed
        .iter()
        .map(|changed| changed.unwrap_or(true))
        .enumerate()
        .skip(1)
    {
        if changed {
            blocks.push(start..row);
            start = row;
        }
    }
    if n > 0 {
        blocks.push(start..n);
    }
    blocks
}

fn validate_structural(column: &Column, is_symbol: bool) -> Result<()> {
    if column.null_count() > 0 {
        return Err(EvalError::Core(if is_symbol {
            QWeaveError::SymbolNull(column.name().to_string())
        } else {
            QWeaveError::TimeNull(column.name().to_string())
        }));
    }
    if matches!(column.dtype(), DataType::Float32 | DataType::Float64) {
        let has_nan = column.as_materialized_series().is_nan()?.any();
        if has_nan {
            return Err(EvalError::Core(QWeaveError::NaNNotAllowed {
                column: column.name().to_string(),
            }));
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

    #[test]
    fn builds_blocks_and_orig_index_matching_cellset() -> Result<()> {
        let df = df!(
            "asset" => ["B", "A", "B"],
            "time" => [1i64, 2, 2],
            "close" => [20.0, 10.0, 21.0],
        )?;

        let ti = build_time_index(&df, &panel())?;

        assert_eq!(ti.blocks, [0..1, 1..3]);
        // TN order: (1,B), (2,A), (2,B) -> original rows 0, 1, 2.
        assert_eq!(ti.orig_index_tn, [0, 1, 2]);
        // Codes by lexicographic symbol order: A=0, B=1.
        assert_eq!(ti.symbol_code_tn, [1, 0, 1]);
        assert_eq!(ti.n_symbols, 2);
        assert_eq!(
            ti.times_tn
                .try_i64()
                .expect("time is i64")
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            [1, 2, 2]
        );
        Ok(())
    }

    #[test]
    fn string_time_column_uses_mask_fallback() -> Result<()> {
        let df = df!(
            "asset" => ["B", "A", "B", "A"],
            "time" => ["d1", "d2", "d2", "d1"],
        )?;

        let ti = build_time_index(&df, &panel())?;

        assert_eq!(ti.blocks, [0..2, 2..4]);
        Ok(())
    }

    #[test]
    fn rejects_duplicates_and_structural_nulls() {
        let dup = df!(
            "asset" => ["A", "A"],
            "time" => [1i64, 1],
        )
        .unwrap();
        let err = build_time_index(&dup, &panel()).unwrap_err();
        assert!(matches!(
            err,
            EvalError::Core(QWeaveError::DuplicateSymbolTime { .. })
        ));

        let null_symbol = df!(
            "asset" => [Some("A"), None],
            "time" => [1i64, 2],
        )
        .unwrap();
        let err = build_time_index(&null_symbol, &panel()).unwrap_err();
        assert!(matches!(err, EvalError::Core(QWeaveError::SymbolNull(_))));

        let nan_time = df!(
            "asset" => ["A", "B"],
            "time" => [1.0f64, f64::NAN],
        )
        .unwrap();
        let err = build_time_index(&nan_time, &panel()).unwrap_err();
        assert!(matches!(
            err,
            EvalError::Core(QWeaveError::NaNNotAllowed { .. })
        ));
    }
}
