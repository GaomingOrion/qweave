use std::collections::{BTreeSet, HashMap};
use std::ops::Range;

use polars::prelude::*;

use crate::column_store::ensure_dtype;
use crate::compute_panel::{ComputePanelOptions, sort_panel, validate_structural_column};
use crate::error::{QFactorsError, Result};
use crate::factor::DType;

const NT_INDEX: &str = "__qfactors_nt_index";

#[derive(Debug, Clone)]
pub struct CellSet {
    pub n_cells: usize,
    pub sym_blocks: Vec<Range<usize>>,
    pub time_blocks: Vec<Range<usize>>,
    pub tn_order: Vec<usize>,
    pub fields: HashMap<String, Vec<f64>>,
    pub symbols_tn: Column,
    pub times_tn: Column,
    pub time_block_by_value: HashMap<AnyValue<'static>, usize>,
}

pub fn build_cellset(
    df: &DataFrame,
    options: &ComputePanelOptions,
    fields: &BTreeSet<String>,
) -> Result<CellSet> {
    let symbol_col = df
        .column(&options.symbol_col)
        .map_err(|_| QFactorsError::MissingColumn(options.symbol_col.clone()))?;
    let time_col = df
        .column(&options.time_col)
        .map_err(|_| QFactorsError::MissingColumn(options.time_col.clone()))?;

    validate_structural_column(symbol_col, true)?;
    validate_structural_column(time_col, false)?;
    validate_fields(df, options, fields)?;

    let sorted = sort_panel(df, options)?;
    let sym_blocks = sym_blocks(&sorted, options)?;

    // TN (time, symbol) ordering via polars' typed, multi-threaded sort instead of a
    // single-threaded `AnyValue` comparison sort. The row-index column, read back in the
    // sorted order, is exactly the NT->TN permutation.
    let tn_sorted = sorted.with_row_index(NT_INDEX.into(), None)?.sort(
        [&options.time_col, &options.symbol_col],
        SortMultipleOptions::default(),
    )?;
    let tn_order = tn_sorted
        .column(NT_INDEX)?
        .as_materialized_series()
        .idx()?
        .into_no_null_iter()
        .map(|idx| idx as usize)
        .collect::<Vec<_>>();
    let symbols_tn = tn_sorted.column(&options.symbol_col)?.clone();
    let times_tn = tn_sorted.column(&options.time_col)?.clone();
    let (time_blocks, time_block_by_value) = time_blocks(&times_tn)?;

    let fields = build_fields(&sorted, options, fields)?;

    Ok(CellSet {
        n_cells: sorted.height(),
        sym_blocks,
        time_blocks,
        tn_order,
        fields,
        symbols_tn,
        times_tn,
        time_block_by_value,
    })
}

fn validate_fields(
    df: &DataFrame,
    options: &ComputePanelOptions,
    fields: &BTreeSet<String>,
) -> Result<()> {
    for logical_name in fields {
        let column_name = options
            .column_aliases
            .get(logical_name)
            .cloned()
            .unwrap_or_else(|| logical_name.clone());
        let column = df
            .column(&column_name)
            .map_err(|_| QFactorsError::MissingColumn(column_name.clone()))?;
        ensure_dtype(column, DType::F64)?;
    }
    Ok(())
}

fn build_fields(
    df: &DataFrame,
    options: &ComputePanelOptions,
    fields: &BTreeSet<String>,
) -> Result<HashMap<String, Vec<f64>>> {
    let mut out = HashMap::with_capacity(fields.len());
    for logical_name in fields {
        let column_name = options
            .column_aliases
            .get(logical_name)
            .cloned()
            .unwrap_or_else(|| logical_name.clone());
        let column = df
            .column(&column_name)
            .map_err(|_| QFactorsError::MissingColumn(column_name.clone()))?;
        let values = column
            .try_f64()
            .expect("dtype checked before sorting")
            .iter()
            .map(|value| value.unwrap_or(f64::NAN))
            .collect::<Vec<_>>();
        out.insert(logical_name.clone(), values);
    }
    Ok(out)
}

fn sym_blocks(sorted: &DataFrame, options: &ComputePanelOptions) -> Result<Vec<Range<usize>>> {
    let n_cells = sorted.height();
    let symbol = sorted.column(&options.symbol_col)?.as_materialized_series();
    let time = sorted.column(&options.time_col)?.as_materialized_series();
    let symbol_changed = symbol.not_equal_missing(&symbol.shift(1))?;
    let time_changed = time.not_equal_missing(&time.shift(1))?;

    let mut blocks = Vec::new();
    let mut start = 0usize;
    for row in 1..n_cells {
        if symbol_changed.get(row).unwrap_or(true) {
            blocks.push(start..row);
            start = row;
        } else if !time_changed.get(row).unwrap_or(true) {
            return Err(QFactorsError::DuplicateSymbolTime {
                symbol_col: options.symbol_col.clone(),
                time_col: options.time_col.clone(),
            });
        }
    }
    if n_cells > 0 {
        blocks.push(start..n_cells);
    }
    Ok(blocks)
}

type TimeBlocks = (Vec<Range<usize>>, HashMap<AnyValue<'static>, usize>);

#[allow(clippy::mutable_key_type)]
fn time_blocks(times_tn: &Column) -> Result<TimeBlocks> {
    let n_cells = times_tn.len();
    let series = times_tn.as_materialized_series();
    let changed = series.not_equal_missing(&series.shift(1))?;

    let mut blocks = Vec::new();
    let mut by_value = HashMap::new();
    let mut start = 0usize;
    for row in 1..n_cells {
        if changed.get(row).unwrap_or(true) {
            by_value.insert(times_tn.get(start)?.into_static(), blocks.len());
            blocks.push(start..row);
            start = row;
        }
    }
    if n_cells > 0 {
        by_value.insert(times_tn.get(start)?.into_static(), blocks.len());
        blocks.push(start..n_cells);
    }
    Ok((blocks, by_value))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> ComputePanelOptions {
        ComputePanelOptions {
            symbol_col: "asset".to_string(),
            time_col: "time".to_string(),
            column_aliases: HashMap::new(),
        }
    }

    #[test]
    fn build_cellset_sorts_nt_and_builds_tn_blocks() -> Result<()> {
        let df = df!(
            "asset" => ["B", "A", "B"],
            "time" => [1i64, 2, 2],
            "open" => [20.0, 10.0, 21.0],
            "close" => [21.0, 10.5, 22.0],
        )?;
        let fields = BTreeSet::from(["open".to_string(), "close".to_string()]);

        let cs = build_cellset(&df, &options(), &fields)?;

        assert_eq!(cs.n_cells, 3);
        assert_eq!(cs.sym_blocks, [0..1, 1..3]);
        assert_eq!(cs.tn_order, [1, 0, 2]);
        assert_eq!(cs.time_blocks, [0..1, 1..3]);
        assert_eq!(cs.fields["open"], [10.0, 20.0, 21.0]);
        assert_eq!(
            cs.symbols_tn
                .try_str()
                .expect("asset is string")
                .iter()
                .collect::<Vec<_>>(),
            [Some("B"), Some("A"), Some("B")]
        );
        assert_eq!(
            cs.times_tn
                .try_i64()
                .expect("time is i64")
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            [1, 2, 2]
        );
        assert_eq!(
            cs.time_block_by_value.get(&AnyValue::Int64(1)).copied(),
            Some(0)
        );
        assert_eq!(
            cs.time_block_by_value.get(&AnyValue::Int64(2)).copied(),
            Some(1)
        );
        Ok(())
    }

    #[test]
    fn build_cellset_uses_aliases_and_float_nulls_become_nan() -> Result<()> {
        let mut options = options();
        options
            .column_aliases
            .insert("open".to_string(), "raw_open".to_string());
        let df = df!(
            "asset" => ["A", "A"],
            "time" => [1i64, 2],
            "raw_open" => [Some(10.0), None],
        )?;
        let fields = BTreeSet::from(["open".to_string()]);

        let cs = build_cellset(&df, &options, &fields)?;

        assert_eq!(cs.fields["open"][0], 10.0);
        assert!(cs.fields["open"][1].is_nan());
        Ok(())
    }

    #[test]
    fn build_cellset_rejects_duplicate_symbol_time() {
        let df = df!(
            "asset" => ["A", "A"],
            "time" => [1i64, 1],
            "open" => [10.0, 11.0],
        )
        .unwrap();
        let fields = BTreeSet::from(["open".to_string()]);

        let err = build_cellset(&df, &options(), &fields).unwrap_err();

        assert!(matches!(err, QFactorsError::DuplicateSymbolTime { .. }));
    }

    #[test]
    fn build_cellset_rejects_missing_wrong_dtype_and_structural_null() {
        let missing = df!(
            "asset" => ["A"],
            "time" => [1i64],
        )
        .unwrap();
        let fields = BTreeSet::from(["open".to_string()]);
        let err = build_cellset(&missing, &options(), &fields).unwrap_err();
        assert!(matches!(err, QFactorsError::MissingColumn(_)));

        let wrong_dtype = df!(
            "asset" => ["A"],
            "time" => [1i64],
            "open" => [1i64],
        )
        .unwrap();
        let err = build_cellset(&wrong_dtype, &options(), &fields).unwrap_err();
        assert!(matches!(err, QFactorsError::DTypeMismatch { .. }));

        let structural_null = df!(
            "asset" => [Some("A"), None],
            "time" => [1i64, 2],
            "open" => [10.0, 11.0],
        )
        .unwrap();
        let err = build_cellset(&structural_null, &options(), &fields).unwrap_err();
        assert!(matches!(err, QFactorsError::SymbolNull(_)));
    }
}
