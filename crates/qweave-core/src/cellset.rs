use std::collections::{BTreeSet, HashMap};
use std::ops::Range;
use std::sync::Arc;

use polars::prelude::*;

use crate::error::{QWeaveError, Result};

const NT_INDEX: &str = "__qweave_nt_index";
const ORIG_INDEX: &str = "__qweave_orig_index";

#[derive(Debug, Clone)]
pub struct PanelOptions {
    pub symbol_col: String,
    pub time_col: String,
}

#[derive(Debug, Clone)]
pub struct CellSet {
    pub n_cells: usize,
    pub sym_blocks: Vec<Range<usize>>,
    pub time_blocks: Vec<Range<usize>>,
    pub tn_order: Vec<usize>,
    pub orig_index_tn: Vec<usize>,
    pub fields: HashMap<String, Arc<Vec<f64>>>,
    pub groups: HashMap<String, Arc<Vec<i32>>>,
    pub symbols_tn: Column,
    pub times_tn: Column,
    pub time_block_by_value: HashMap<AnyValue<'static>, usize>,
}

pub fn build_cellset(
    df: &DataFrame,
    options: &PanelOptions,
    fields: &BTreeSet<String>,
) -> Result<CellSet> {
    build_cellset_with_groups(df, options, fields, &BTreeSet::new())
}

pub fn build_cellset_with_groups(
    df: &DataFrame,
    options: &PanelOptions,
    fields: &BTreeSet<String>,
    groups: &BTreeSet<String>,
) -> Result<CellSet> {
    let symbol_col = df
        .column(&options.symbol_col)
        .map_err(|_| QWeaveError::MissingColumn(options.symbol_col.clone()))?;
    let time_col = df
        .column(&options.time_col)
        .map_err(|_| QWeaveError::MissingColumn(options.time_col.clone()))?;

    validate_structural_column(symbol_col, true)?;
    validate_structural_column(time_col, false)?;
    validate_fields(df, fields)?;
    validate_groups(df, groups)?;

    let indexed = df.with_row_index(ORIG_INDEX.into(), None)?;
    // Only the structural columns, the row index, and the requested field columns
    // are ever read back from `sorted`. Projecting before the sort keeps it from
    // gathering a full-width copy of every (possibly hundreds of) unrelated column.
    let mut projection: Vec<&str> = Vec::with_capacity(fields.len() + groups.len() + 3);
    projection.push(&options.symbol_col);
    projection.push(&options.time_col);
    projection.push(ORIG_INDEX);
    for field in fields {
        if field != &options.symbol_col && field != &options.time_col {
            projection.push(field);
        }
    }
    for group in groups {
        if group != &options.symbol_col && group != &options.time_col && !fields.contains(group) {
            projection.push(group);
        }
    }
    let narrow = indexed.select(projection)?;
    let sorted = sort_panel(&narrow, options)?;
    let sym_blocks = sym_blocks(&sorted, options)?;

    // TN (time, symbol) ordering via polars' typed, multi-threaded sort instead of a
    // single-threaded `AnyValue` comparison sort. The row-index column, read back in the
    // sorted order, is exactly the NT->TN permutation.
    let tn_sorted = sorted
        .with_row_index(NT_INDEX.into(), None)?
        .select([
            options.time_col.as_str(),
            options.symbol_col.as_str(),
            NT_INDEX,
            ORIG_INDEX,
        ])?
        .sort(
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
    let orig_index_tn = tn_sorted
        .column(ORIG_INDEX)?
        .as_materialized_series()
        .idx()?
        .into_no_null_iter()
        .map(|idx| idx as usize)
        .collect::<Vec<_>>();
    let symbols_tn = tn_sorted.column(&options.symbol_col)?.clone();
    let times_tn = tn_sorted.column(&options.time_col)?.clone();
    let (time_blocks, time_block_by_value) = time_blocks(&times_tn)?;

    let fields = build_fields(&sorted, options, fields)?;
    let groups = build_groups(&sorted, groups)?;

    Ok(CellSet {
        n_cells: sorted.height(),
        sym_blocks,
        time_blocks,
        tn_order,
        orig_index_tn,
        fields,
        groups,
        symbols_tn,
        times_tn,
        time_block_by_value,
    })
}

fn validate_fields(df: &DataFrame, fields: &BTreeSet<String>) -> Result<()> {
    for column_name in fields {
        let column = df
            .column(column_name)
            .map_err(|_| QWeaveError::MissingColumn(column_name.clone()))?;
        ensure_f64(column)?;
    }
    Ok(())
}

fn build_fields(
    df: &DataFrame,
    _options: &PanelOptions,
    fields: &BTreeSet<String>,
) -> Result<HashMap<String, Arc<Vec<f64>>>> {
    let mut out = HashMap::with_capacity(fields.len());
    for column_name in fields {
        let column = df
            .column(column_name)
            .map_err(|_| QWeaveError::MissingColumn(column_name.clone()))?;
        let values = column
            .try_f64()
            .expect("dtype checked before sorting")
            .iter()
            .map(|value| value.unwrap_or(f64::NAN))
            .collect::<Vec<_>>();
        out.insert(column_name.clone(), Arc::new(values));
    }
    Ok(out)
}

fn ensure_f64(column: &Column) -> Result<()> {
    if column.dtype() == &DataType::Float64 {
        Ok(())
    } else {
        Err(QWeaveError::DTypeMismatch {
            column: column.name().to_string(),
            expected: "f64",
            actual: column.dtype().to_string(),
        })
    }
}

fn validate_groups(df: &DataFrame, groups: &BTreeSet<String>) -> Result<()> {
    for name in groups {
        let column = df
            .column(name)
            .map_err(|_| QWeaveError::MissingColumn(name.clone()))?;
        if column.null_count() > 0 {
            return Err(QWeaveError::GroupNull(name.clone()));
        }
        if !matches!(
            column.dtype(),
            DataType::String
                | DataType::Int8
                | DataType::Int16
                | DataType::Int32
                | DataType::Int64
                | DataType::UInt8
                | DataType::UInt16
                | DataType::UInt32
                | DataType::UInt64
        ) {
            return Err(QWeaveError::DTypeMismatch {
                column: name.clone(),
                expected: "string or integer group column",
                actual: column.dtype().to_string(),
            });
        }
    }
    Ok(())
}

fn build_groups(
    df: &DataFrame,
    groups: &BTreeSet<String>,
) -> Result<HashMap<String, Arc<Vec<i32>>>> {
    let mut out = HashMap::with_capacity(groups.len());
    for name in groups {
        let column = df.column(name)?;
        let values = if column.dtype() == &DataType::String {
            let mut codes = HashMap::<&str, i32>::new();
            column
                .try_str()
                .expect("dtype checked")
                .iter()
                .map(|value| {
                    let value = value.expect("nulls rejected before sorting");
                    let next = i32::try_from(codes.len()).map_err(|_| {
                        QWeaveError::GroupValueOutOfRange {
                            column: name.clone(),
                            value: "more than i32::MAX distinct groups".to_string(),
                        }
                    })?;
                    Ok(*codes.entry(value).or_insert(next))
                })
                .collect::<Result<Vec<_>>>()?
        } else {
            column
                .as_materialized_series()
                .iter()
                .map(|value| integer_to_i32(name, value))
                .collect::<Result<Vec<_>>>()?
        };
        out.insert(name.clone(), Arc::new(values));
    }
    Ok(out)
}

fn integer_to_i32(column: &str, value: AnyValue<'_>) -> Result<i32> {
    let value = match value {
        AnyValue::Int8(v) => v as i128,
        AnyValue::Int16(v) => v as i128,
        AnyValue::Int32(v) => v as i128,
        AnyValue::Int64(v) => v as i128,
        AnyValue::UInt8(v) => v as i128,
        AnyValue::UInt16(v) => v as i128,
        AnyValue::UInt32(v) => v as i128,
        AnyValue::UInt64(v) => v as i128,
        _ => unreachable!("group dtype checked before sorting"),
    };
    i32::try_from(value).map_err(|_| QWeaveError::GroupValueOutOfRange {
        column: column.to_string(),
        value: value.to_string(),
    })
}

pub(crate) fn validate_structural_column(column: &Column, is_symbol: bool) -> Result<()> {
    if column.null_count() > 0 {
        if is_symbol {
            return Err(QWeaveError::SymbolNull(column.name().to_string()));
        }
        return Err(QWeaveError::TimeNull(column.name().to_string()));
    }

    reject_nan_values(column)
}

fn reject_nan_values(column: &Column) -> Result<()> {
    if !matches!(column.dtype(), DataType::Float32 | DataType::Float64) {
        return Ok(());
    }

    for row in 0..column.len() {
        if is_nan_value(&column.get(row)?) {
            return Err(QWeaveError::NaNNotAllowed {
                column: column.name().to_string(),
            });
        }
    }

    Ok(())
}

fn is_nan_value(value: &AnyValue<'_>) -> bool {
    match value {
        AnyValue::Float32(value) => value.is_nan(),
        AnyValue::Float64(value) => value.is_nan(),
        _ => false,
    }
}

pub(crate) fn sort_panel(df: &DataFrame, options: &PanelOptions) -> Result<DataFrame> {
    Ok(df.sort(
        [&options.symbol_col, &options.time_col],
        SortMultipleOptions::default(),
    )?)
}

fn sym_blocks(sorted: &DataFrame, options: &PanelOptions) -> Result<Vec<Range<usize>>> {
    let n_cells = sorted.height();
    let symbol = sorted.column(&options.symbol_col)?.as_materialized_series();
    let time = sorted.column(&options.time_col)?.as_materialized_series();
    let symbol_changed = symbol.not_equal_missing(&symbol.shift(1))?;
    let time_changed = time.not_equal_missing(&time.shift(1))?;

    let mut blocks = Vec::new();
    let mut start = 0usize;
    for (row, (symbol_changed, time_changed)) in symbol_changed
        .iter()
        .map(|changed| changed.unwrap_or(true))
        .zip(time_changed.iter().map(|changed| changed.unwrap_or(true)))
        .enumerate()
        .skip(1)
    {
        if symbol_changed {
            blocks.push(start..row);
            start = row;
        } else if !time_changed {
            return Err(QWeaveError::DuplicateSymbolTime {
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
    for (row, changed) in changed
        .iter()
        .map(|changed| changed.unwrap_or(true))
        .enumerate()
        .skip(1)
    {
        if changed {
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

    fn options() -> PanelOptions {
        PanelOptions {
            symbol_col: "asset".to_string(),
            time_col: "time".to_string(),
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
        assert_eq!(cs.orig_index_tn, [0, 1, 2]);
        assert_eq!(cs.time_blocks, [0..1, 1..3]);
        assert_eq!(cs.fields["open"].as_slice(), [10.0, 20.0, 21.0]);
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
    fn build_cellset_float_nulls_become_nan() -> Result<()> {
        let df = df!(
            "asset" => ["A", "A"],
            "time" => [1i64, 2],
            "open" => [Some(10.0), None],
        )?;
        let fields = BTreeSet::from(["open".to_string()]);

        let cs = build_cellset(&df, &options(), &fields)?;

        assert_eq!(cs.fields["open"][0], 10.0);
        assert!(cs.fields["open"][1].is_nan());
        Ok(())
    }

    #[test]
    fn build_cellset_encodes_string_and_integer_groups_as_i32() -> Result<()> {
        let df = df!(
            "asset" => ["A", "A", "A"],
            "time" => [1i64, 2, 3],
            "industry" => ["tech", "finance", "tech"],
            "sector" => [20i32, 10, 20],
        )?;
        let groups = BTreeSet::from(["industry".to_string(), "sector".to_string()]);

        let cs = build_cellset_with_groups(&df, &options(), &BTreeSet::new(), &groups)?;

        assert_eq!(cs.groups["industry"].as_slice(), [0, 1, 0]);
        assert_eq!(cs.groups["sector"].as_slice(), [20, 10, 20]);
        Ok(())
    }

    #[test]
    fn build_cellset_rejects_float_null_and_out_of_range_groups() {
        let groups = BTreeSet::from(["group".to_string()]);
        let float_group = df!(
            "asset" => ["A"], "time" => [1i64], "group" => [1.0f64],
        )
        .unwrap();
        assert!(matches!(
            build_cellset_with_groups(&float_group, &options(), &BTreeSet::new(), &groups)
                .unwrap_err(),
            QWeaveError::DTypeMismatch { .. }
        ));

        let null_group = df!(
            "asset" => ["A"], "time" => [1i64], "group" => [None::<&str>],
        )
        .unwrap();
        assert!(matches!(
            build_cellset_with_groups(&null_group, &options(), &BTreeSet::new(), &groups)
                .unwrap_err(),
            QWeaveError::GroupNull(_)
        ));

        assert!(matches!(
            integer_to_i32("group", AnyValue::Int64(i64::from(i32::MAX) + 1)).unwrap_err(),
            QWeaveError::GroupValueOutOfRange { .. }
        ));
        assert!(matches!(
            integer_to_i32("group", AnyValue::UInt64(u64::MAX)).unwrap_err(),
            QWeaveError::GroupValueOutOfRange { .. }
        ));
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

        assert!(matches!(err, QWeaveError::DuplicateSymbolTime { .. }));
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
        assert!(matches!(err, QWeaveError::MissingColumn(_)));

        let wrong_dtype = df!(
            "asset" => ["A"],
            "time" => [1i64],
            "open" => [true],
        )
        .unwrap();
        let err = build_cellset(&wrong_dtype, &options(), &fields).unwrap_err();
        assert!(matches!(err, QWeaveError::DTypeMismatch { .. }));

        let structural_null = df!(
            "asset" => [Some("A"), None],
            "time" => [1i64, 2],
            "open" => [10.0, 11.0],
        )
        .unwrap();
        let err = build_cellset(&structural_null, &options(), &fields).unwrap_err();
        assert!(matches!(err, QWeaveError::SymbolNull(_)));
    }
}
