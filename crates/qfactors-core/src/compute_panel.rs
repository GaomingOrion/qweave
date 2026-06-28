use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::ops::Range;

use polars::prelude::*;
use rayon::prelude::*;

use crate::column_store::{ColumnStore, ensure_dtype};
use crate::compute_sink::{ComputeResult, ComputeSink};
use crate::error::{QFactorsError, Result};
use crate::factor::{DType, FactorResult, ResolvedFactor, default_output_columns};
use crate::registry::{FactorRegistry, factor_registry};

#[derive(Debug, Clone)]
pub struct ComputePanelOptions {
    pub symbol_col: String,
    pub time_col: String,
    pub column_aliases: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct RequiredInput {
    name: String,
    dtype: DType,
}

#[derive(Debug)]
struct ResolvedPanel<'a> {
    factors: Vec<ResolvedFactor<'a>>,
    required_inputs: Vec<RequiredInput>,
}

#[derive(Debug)]
struct ResolvedObservations {
    values: Column,
    index_by_value: HashMap<AnyValue<'static>, usize>,
}

#[derive(Debug)]
struct ObservationPartition {
    input_index: usize,
    row_indices: Vec<usize>,
    avail_lens: Vec<usize>,
}

pub fn compute_panel(
    df: DataFrame,
    options: ComputePanelOptions,
    factor_names: Vec<String>,
    observation_times: Series,
    output_path: Option<&str>,
) -> Result<ComputeResult> {
    let resolved = resolve_factors(&df, &options, factor_registry()?, &factor_names)?;
    let mut df = project_panel(&df, &options, &resolved.required_inputs)?;

    validate_structural_column(df.column(&options.symbol_col)?, true)?;
    validate_structural_column(df.column(&options.time_col)?, false)?;

    if !is_sorted_by_symbol_time(&df, &options)? {
        df = sort_panel(&df, &options)?;
    }

    let observations = resolve_observation_times(&df, &options.time_col, observation_times)?;
    let partitions = build_observation_partitions(&df, &options, &observations.index_by_value)?;

    apply_input_null_rules(&mut df, &resolved.required_inputs)?;
    if df.max_n_chunks() > 1 {
        df.rechunk_mut();
    }

    let columns = ColumnStore::new(&df);
    let mut sink = ComputeSink::for_output(output_path);

    for partition in &partitions {
        let factor_columns = compute_factors_for_partition(&columns, partition, &resolved.factors)?;
        let frame = build_observation_frame(
            &df,
            &options,
            &observations.values,
            partition,
            factor_columns,
        )?;
        sink.write_observation(frame)?;
    }

    sink.finish()
}

fn resolve_factors<'a>(
    df: &DataFrame,
    options: &ComputePanelOptions,
    registry: &'a FactorRegistry,
    factor_names: &[String],
) -> Result<ResolvedPanel<'a>> {
    ensure_column_exists(df, &options.symbol_col)?;
    ensure_column_exists(df, &options.time_col)?;

    let mut output_names = HashSet::new();
    let mut required_names = HashSet::new();
    let mut required_inputs = Vec::new();
    let mut factors = Vec::with_capacity(factor_names.len());

    for factor_name in factor_names {
        let desc = registry
            .get(factor_name)
            .ok_or_else(|| QFactorsError::UnknownFactor(factor_name.clone()))?;

        if desc.window == 0 {
            return Err(QFactorsError::InvalidWindow {
                factor_name: desc.factor_name,
                window: desc.window,
            });
        }

        let mut input_columns = Vec::with_capacity(desc.inputs.len());
        for input in desc.inputs {
            let column_name = options
                .column_aliases
                .get(input.name)
                .cloned()
                .unwrap_or_else(|| input.name.to_string());
            let column = df
                .column(&column_name)
                .map_err(|_| QFactorsError::MissingColumn(column_name.clone()))?;
            ensure_dtype(column, input.dtype)?;

            if required_names.insert(column_name.clone()) {
                required_inputs.push(RequiredInput {
                    name: column_name.clone(),
                    dtype: input.dtype,
                });
            }
            input_columns.push(column_name);
        }

        let output_columns = default_output_columns(desc);
        for output_column in &output_columns {
            ensure_output_name_available(options, &mut output_names, output_column)?;
        }

        factors.push(ResolvedFactor {
            desc,
            input_columns,
            output_columns,
        });
    }

    Ok(ResolvedPanel {
        factors,
        required_inputs,
    })
}

fn ensure_column_exists(df: &DataFrame, name: &str) -> Result<()> {
    df.column(name)
        .map(|_| ())
        .map_err(|_| QFactorsError::MissingColumn(name.to_string()))
}

fn ensure_output_name_available(
    options: &ComputePanelOptions,
    seen: &mut HashSet<String>,
    name: &str,
) -> Result<()> {
    if name == options.time_col || name == options.symbol_col || !seen.insert(name.to_string()) {
        return Err(QFactorsError::OutputColumnConflict(name.to_string()));
    }
    Ok(())
}

fn project_panel(
    df: &DataFrame,
    options: &ComputePanelOptions,
    required_inputs: &[RequiredInput],
) -> Result<DataFrame> {
    let mut projection = Vec::with_capacity(2 + required_inputs.len());
    push_unique(&mut projection, &options.symbol_col);
    push_unique(&mut projection, &options.time_col);
    for input in required_inputs {
        push_unique(&mut projection, &input.name);
    }

    Ok(df.select(projection.iter().map(String::as_str))?)
}

fn push_unique(values: &mut Vec<String>, value: &str) {
    if !values.iter().any(|existing| existing == value) {
        values.push(value.to_string());
    }
}

fn validate_structural_column(column: &Column, is_symbol: bool) -> Result<()> {
    if column.null_count() > 0 {
        if is_symbol {
            return Err(QFactorsError::SymbolNull(column.name().to_string()));
        }
        return Err(QFactorsError::TimeNull(column.name().to_string()));
    }

    reject_nan_values(column)
}

fn reject_nan_values(column: &Column) -> Result<()> {
    if !matches!(column.dtype(), DataType::Float32 | DataType::Float64) {
        return Ok(());
    }

    for row in 0..column.len() {
        if is_nan_value(&column.get(row)?) {
            return Err(QFactorsError::NaNNotAllowed {
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

fn is_sorted_by_symbol_time(df: &DataFrame, options: &ComputePanelOptions) -> Result<bool> {
    let symbol = df.column(&options.symbol_col)?;
    let time = df.column(&options.time_col)?;

    for row in 1..df.height() {
        let ordering = compare_symbol_time(
            symbol.get(row - 1)?,
            time.get(row - 1)?,
            symbol.get(row)?,
            time.get(row)?,
            options,
        )?;
        if ordering == Ordering::Greater {
            return Ok(false);
        }
    }

    Ok(true)
}

fn compare_symbol_time(
    left_symbol: AnyValue<'_>,
    left_time: AnyValue<'_>,
    right_symbol: AnyValue<'_>,
    right_time: AnyValue<'_>,
    options: &ComputePanelOptions,
) -> Result<Ordering> {
    match left_symbol.partial_cmp(&right_symbol) {
        Some(Ordering::Equal) => {}
        Some(ordering) => return Ok(ordering),
        None => {
            return Err(QFactorsError::NonComparableColumn {
                column: options.symbol_col.clone(),
            });
        }
    }

    left_time
        .partial_cmp(&right_time)
        .ok_or_else(|| QFactorsError::NonComparableColumn {
            column: options.time_col.clone(),
        })
}

fn sort_panel(df: &DataFrame, options: &ComputePanelOptions) -> Result<DataFrame> {
    Ok(df.sort(
        [&options.symbol_col, &options.time_col],
        SortMultipleOptions::default(),
    )?)
}

fn resolve_observation_times(
    df: &DataFrame,
    time_col: &str,
    observation_times: Series,
) -> Result<ResolvedObservations> {
    let time_dtype = df.column(time_col)?.dtype().clone();
    let mut values = observation_times.cast(&time_dtype)?.into_column();
    values.rename(time_col.into());

    if values.is_empty() {
        return Err(QFactorsError::ObservationTimesEmpty);
    }
    if values.null_count() > 0 {
        return Err(QFactorsError::ObservationTimeNull);
    }
    reject_nan_values(&values)?;

    let mut index_by_value = HashMap::with_capacity(values.len());
    for row in 0..values.len() {
        let value = values.get(row)?.into_static();
        if index_by_value.insert(value.clone(), row).is_some() {
            return Err(QFactorsError::DuplicateObservationTime(format!(
                "{value:?}"
            )));
        }
    }

    Ok(ResolvedObservations {
        values,
        index_by_value,
    })
}

fn build_observation_partitions(
    df: &DataFrame,
    options: &ComputePanelOptions,
    observation_index: &HashMap<AnyValue<'static>, usize>,
) -> Result<Vec<ObservationPartition>> {
    let symbol = df.column(&options.symbol_col)?;
    let time = df.column(&options.time_col)?;
    let mut partitions = (0..observation_index.len())
        .map(|input_index| ObservationPartition {
            input_index,
            row_indices: Vec::new(),
            avail_lens: Vec::new(),
        })
        .collect::<Vec<_>>();

    let mut previous_symbol: Option<AnyValue<'static>> = None;
    let mut previous_time: Option<AnyValue<'static>> = None;
    let mut avail_len = 0usize;

    for row in 0..df.height() {
        let symbol_value = symbol.get(row)?.into_static();
        let time_value = time.get(row)?.into_static();

        if previous_symbol.as_ref() == Some(&symbol_value) {
            if previous_time.as_ref() == Some(&time_value) {
                return Err(QFactorsError::DuplicateSymbolTime {
                    symbol_col: options.symbol_col.clone(),
                    time_col: options.time_col.clone(),
                });
            }
            avail_len += 1;
        } else {
            avail_len = 1;
        }

        if let Some(&input_index) = observation_index.get(&time_value) {
            partitions[input_index].row_indices.push(row);
            partitions[input_index].avail_lens.push(avail_len);
        }

        previous_symbol = Some(symbol_value);
        previous_time = Some(time_value);
    }

    Ok(partitions)
}

fn apply_input_null_rules(df: &mut DataFrame, required_inputs: &[RequiredInput]) -> Result<()> {
    for input in required_inputs {
        let column = df.column(&input.name)?;
        if column.null_count() == 0 {
            continue;
        }

        match input.dtype {
            DType::F64 => fill_float_nulls_with_nan(df, &input.name)?,
            DType::U32 | DType::I64 => {
                return Err(QFactorsError::NullNotAllowed {
                    column: input.name.clone(),
                });
            }
        }
    }

    Ok(())
}

fn fill_float_nulls_with_nan(df: &mut DataFrame, column_name: &str) -> Result<()> {
    let index = df
        .get_column_index(column_name)
        .expect("required input column came from this DataFrame");
    let column = df.column(column_name)?;
    let values = column
        .try_f64()
        .expect("required input dtype was checked")
        .iter()
        .map(|value| value.unwrap_or(f64::NAN))
        .collect::<Vec<_>>();

    df.replace_column(index, Column::new(column_name.into(), values))?;
    Ok(())
}

fn compute_factors_for_partition(
    columns: &ColumnStore<'_>,
    partition: &ObservationPartition,
    factors: &[ResolvedFactor<'_>],
) -> Result<FactorResult> {
    let results = factors
        .par_iter()
        .map(|factor| {
            let ranges = ranges_for_partition(partition, factor.desc.window);
            let result = (factor.desc.compute)(columns, &ranges, factor)?;
            validate_factor_result(partition, factor, &result)?;
            Ok(result)
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(results.into_iter().flatten().collect())
}

fn ranges_for_partition(
    partition: &ObservationPartition,
    window: usize,
) -> Vec<Option<Range<usize>>> {
    partition
        .row_indices
        .iter()
        .zip(&partition.avail_lens)
        .map(|(&row_idx, &avail_len)| {
            if avail_len < window {
                None
            } else {
                Some(row_idx + 1 - window..row_idx + 1)
            }
        })
        .collect()
}

fn validate_factor_result(
    partition: &ObservationPartition,
    factor: &ResolvedFactor<'_>,
    result: &FactorResult,
) -> Result<()> {
    if result.len() != factor.output_columns.len() {
        return Err(QFactorsError::FactorOutputCount {
            factor_name: factor.desc.factor_name,
            expected: factor.output_columns.len(),
            actual: result.len(),
        });
    }

    for (column, expected_name) in result.iter().zip(&factor.output_columns) {
        if column.len() != partition.row_indices.len() {
            return Err(QFactorsError::FactorOutputLength {
                factor_name: factor.desc.factor_name,
                column: column.name().to_string(),
                expected: partition.row_indices.len(),
                actual: column.len(),
            });
        }

        if column.name().as_str() != expected_name {
            return Err(QFactorsError::FactorOutputName {
                factor_name: factor.desc.factor_name,
                expected: expected_name.clone(),
                actual: column.name().to_string(),
            });
        }
    }

    Ok(())
}

fn build_observation_frame(
    df: &DataFrame,
    options: &ComputePanelOptions,
    observation_values: &Column,
    partition: &ObservationPartition,
    factor_columns: FactorResult,
) -> Result<DataFrame> {
    let n_rows = partition.row_indices.len();
    let mut time = observation_values.new_from_index(partition.input_index, n_rows);
    time.rename(options.time_col.clone().into());

    let row_indices = partition
        .row_indices
        .iter()
        .map(|&row_idx| row_idx as IdxSize)
        .collect::<Vec<_>>();
    let mut symbol = df.column(&options.symbol_col)?.take_slice(&row_indices)?;
    symbol.rename(options.symbol_col.clone().into());

    let mut columns = vec![time, symbol];
    columns.extend(factor_columns);
    Ok(DataFrame::new_infer_height(columns)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::factor::{ColumnSpec, FactorDescriptor};

    static INPUTS: [ColumnSpec; 1] = [ColumnSpec {
        name: "close",
        dtype: DType::F64,
    }];
    static INT_INPUTS: [ColumnSpec; 1] = [ColumnSpec {
        name: "count",
        dtype: DType::I64,
    }];
    static OUTPUTS: [ColumnSpec; 1] = [ColumnSpec {
        name: "dummy",
        dtype: DType::F64,
    }];

    fn dummy_descriptor() -> FactorDescriptor {
        FactorDescriptor {
            factor_name: "dummy",
            kernel_name: "dummy",
            window: 2,
            inputs: &INPUTS,
            outputs: &OUTPUTS,
            param_set: None,
            params: &[],
            compute: dummy_compute,
        }
    }

    fn int_dummy_descriptor() -> FactorDescriptor {
        FactorDescriptor {
            factor_name: "int_dummy",
            kernel_name: "int_dummy",
            window: 2,
            inputs: &INT_INPUTS,
            outputs: &OUTPUTS,
            param_set: None,
            params: &[],
            compute: int_dummy_compute,
        }
    }

    #[linkme::distributed_slice(crate::registry::FACTOR_DESCRIPTORS)]
    static TEST_DUMMY_DESCRIPTOR: fn() -> FactorDescriptor = dummy_descriptor;

    #[linkme::distributed_slice(crate::registry::FACTOR_DESCRIPTORS)]
    static TEST_INT_DUMMY_DESCRIPTOR: fn() -> FactorDescriptor = int_dummy_descriptor;

    fn dummy_compute(
        columns: &ColumnStore<'_>,
        ranges: &[Option<Range<usize>>],
        factor: &ResolvedFactor<'_>,
    ) -> Result<FactorResult> {
        let close = columns.f64(&factor.input_columns[0])?;
        let mut values = vec![f64::NAN; ranges.len()];
        for (idx, range) in ranges.iter().enumerate() {
            if let Some(range) = range {
                values[idx] = close[range.end - 1] - close[range.start];
            }
        }
        Ok(vec![Column::new(
            factor.output_columns[0].clone().into(),
            values,
        )])
    }

    fn int_dummy_compute(
        columns: &ColumnStore<'_>,
        ranges: &[Option<Range<usize>>],
        factor: &ResolvedFactor<'_>,
    ) -> Result<FactorResult> {
        let count = columns.i64(&factor.input_columns[0])?;
        let mut values = vec![f64::NAN; ranges.len()];
        for (idx, range) in ranges.iter().enumerate() {
            if let Some(range) = range {
                values[idx] = (count[range.end - 1] - count[range.start]) as f64;
            }
        }
        Ok(vec![Column::new(
            factor.output_columns[0].clone().into(),
            values,
        )])
    }

    fn options() -> ComputePanelOptions {
        ComputePanelOptions {
            symbol_col: "asset".to_string(),
            time_col: "time".to_string(),
            column_aliases: HashMap::new(),
        }
    }

    fn memory_frame(result: ComputeResult) -> Result<DataFrame> {
        match result {
            ComputeResult::Memory(df) => Ok(df),
            ComputeResult::File(_) => panic!("expected memory result"),
        }
    }

    #[test]
    fn computes_only_symbols_present_on_observation_time() -> Result<()> {
        let df = df!(
            "asset" => ["B", "A", "A", "B", "A"],
            "time" => [1i64, 3, 1, 3, 2],
            "close" => [20.0, 12.0, 10.0, 22.0, 11.0],
        )?;

        let out = memory_frame(compute_panel(
            df,
            options(),
            vec!["dummy".to_string()],
            Series::new("time".into(), [2i64, 3]),
            None,
        )?)?;

        assert_eq!(out.height(), 3);
        assert_eq!(
            time_asset_rows(&out)?,
            [
                (2, "A".to_string()),
                (3, "A".to_string()),
                (3, "B".to_string())
            ]
        );

        let values = out
            .column("dummy")?
            .try_f64()
            .expect("dummy is f64")
            .into_no_null_iter()
            .collect::<Vec<_>>();
        assert_eq!(values, [1.0, 1.0, 2.0]);
        Ok(())
    }

    #[test]
    fn present_symbol_with_insufficient_window_outputs_nan() -> Result<()> {
        let df = df!(
            "asset" => ["A", "A", "B", "B", "C"],
            "time" => [1i64, 2, 1, 3, 3],
            "close" => [10.0, 11.0, 20.0, 22.0, 30.0],
        )?;

        let out = memory_frame(compute_panel(
            df,
            options(),
            vec!["dummy".to_string()],
            Series::new("time".into(), [3i64]),
            None,
        )?)?;

        assert_eq!(
            time_asset_rows(&out)?,
            [(3, "B".to_string()), (3, "C".to_string())]
        );
        let values = out
            .column("dummy")?
            .try_f64()
            .expect("dummy is f64")
            .into_no_null_iter()
            .collect::<Vec<_>>();
        assert_eq!(values[0], 2.0);
        assert!(values[1].is_nan());
        Ok(())
    }

    #[test]
    fn missing_observation_time_keeps_schema_and_outputs_no_rows() -> Result<()> {
        let df = df!(
            "asset" => ["A", "A"],
            "time" => [1i64, 2],
            "close" => [10.0, 11.0],
        )?;

        let out = memory_frame(compute_panel(
            df,
            options(),
            vec!["dummy".to_string()],
            Series::new("time".into(), [9i64]),
            None,
        )?)?;

        assert_eq!(out.height(), 0);
        assert_eq!(column_names(&out), ["time", "asset", "dummy"]);
        Ok(())
    }

    #[test]
    fn unused_column_nulls_are_ignored() -> Result<()> {
        let df = df!(
            "asset" => ["A", "A"],
            "time" => [1i64, 2],
            "close" => [10.0, 11.0],
            "unused" => [Some(1i64), None],
        )?;

        let out = memory_frame(compute_panel(
            df,
            options(),
            vec!["dummy".to_string()],
            Series::new("time".into(), [2i64]),
            None,
        )?)?;

        assert_eq!(out.height(), 1);
        Ok(())
    }

    #[test]
    fn float_input_nulls_become_nan() -> Result<()> {
        let df = df!(
            "asset" => ["A", "A"],
            "time" => [1i64, 2],
            "close" => [Some(10.0), None],
        )?;

        let out = memory_frame(compute_panel(
            df,
            options(),
            vec!["dummy".to_string()],
            Series::new("time".into(), [2i64]),
            None,
        )?)?;
        let value = out
            .column("dummy")?
            .try_f64()
            .expect("dummy is f64")
            .get(0)
            .expect("result row exists");
        assert!(value.is_nan());
        Ok(())
    }

    #[test]
    fn integer_input_nulls_are_rejected() {
        let df = df!(
            "asset" => ["A", "A"],
            "time" => [1i64, 2],
            "count" => [Some(1i64), None],
        )
        .unwrap();

        let err = compute_panel(
            df,
            options(),
            vec!["int_dummy".to_string()],
            Series::new("time".into(), [2i64]),
            None,
        )
        .unwrap_err();
        assert!(matches!(err, QFactorsError::NullNotAllowed { .. }));
    }

    #[test]
    fn structural_nulls_are_rejected() {
        let df = df!(
            "asset" => ["A", "A"],
            "time" => [Some(1i64), None],
            "close" => [10.0, 11.0],
        )
        .unwrap();

        let err = compute_panel(
            df,
            options(),
            vec!["dummy".to_string()],
            Series::new("time".into(), [2i64]),
            None,
        )
        .unwrap_err();
        assert!(matches!(err, QFactorsError::TimeNull(_)));
    }

    #[test]
    fn duplicate_symbol_time_is_rejected() {
        let df = df!(
            "asset" => ["A", "A"],
            "time" => [1i64, 1],
            "close" => [10.0, 11.0],
        )
        .unwrap();

        let err = compute_panel(
            df,
            options(),
            vec!["dummy".to_string()],
            Series::new("time".into(), [1i64]),
            None,
        )
        .unwrap_err();
        assert!(matches!(err, QFactorsError::DuplicateSymbolTime { .. }));
    }

    #[test]
    fn observation_times_reject_duplicates() {
        let df = df!(
            "asset" => ["A", "A"],
            "time" => [1i64, 2],
            "close" => [10.0, 11.0],
        )
        .unwrap();

        let err = compute_panel(
            df,
            options(),
            vec!["dummy".to_string()],
            Series::new("time".into(), [2i64, 2]),
            None,
        )
        .unwrap_err();
        assert!(matches!(err, QFactorsError::DuplicateObservationTime(_)));
    }

    #[test]
    fn resolve_rejects_unknown_factor() {
        let df = df!(
            "asset" => ["A", "A"],
            "time" => [1i64, 2],
            "close" => [10.0, 11.0],
        )
        .unwrap();

        let err = compute_panel(
            df,
            options(),
            vec!["missing".to_string()],
            Series::new("time".into(), [2i64]),
            None,
        )
        .unwrap_err();
        assert!(matches!(err, QFactorsError::UnknownFactor(_)));
    }

    fn time_asset_rows(df: &DataFrame) -> Result<Vec<(i64, String)>> {
        let times = df.column("time")?.try_i64().expect("time is i64");
        let assets = df.column("asset")?.try_str().expect("asset is string");
        Ok(times
            .into_no_null_iter()
            .zip(assets.iter())
            .map(|(time, asset)| (time, asset.expect("asset has no nulls").to_string()))
            .collect())
    }

    fn column_names(df: &DataFrame) -> Vec<String> {
        df.get_column_names()
            .iter()
            .map(|name| name.to_string())
            .collect()
    }
}
