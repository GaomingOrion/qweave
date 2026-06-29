use std::collections::{BTreeSet, HashSet};
use std::env;

use polars::prelude::*;
use rayon::prelude::*;

use crate::alpha_dag::eval_alphas as eval_alphas_dag;
use crate::alpha_eval::{eval, to_cells};
use crate::alpha_registry::alpha_registry;
use crate::cellset::{CellSet, build_cellset};
use crate::compute_panel::{ComputePanelOptions, reject_nan_values};
use crate::compute_sink::{ComputeResult, ComputeSink};
use crate::error::{QFactorsError, Result};
use crate::expr::{Expr, collect_fields};
use crate::layout::Layout;

enum AlphaEngine {
    Tree,
    Dag,
}

struct ResolvedAlphaObservations {
    values: Column,
    time_blocks: Vec<Option<usize>>,
}

pub fn compute_alphas(
    df: DataFrame,
    options: ComputePanelOptions,
    alpha_names: Vec<String>,
    observation_times: Series,
    output_path: Option<&str>,
) -> Result<ComputeResult> {
    let resolved = resolve_alphas(&options, alpha_names)?;
    let mut fields = BTreeSet::new();
    for (_, expr) in &resolved {
        collect_fields(expr, &mut fields);
    }

    let cs = build_cellset(&df, &options, &fields)?;
    let results = match alpha_engine()? {
        AlphaEngine::Tree => eval_alphas_tree(resolved, &cs)?,
        AlphaEngine::Dag => eval_alphas_dag(&resolved, &cs)?,
    };
    let observations = resolve_alpha_observations(&df, &options.time_col, &cs, observation_times)?;

    let mut sink = ComputeSink::for_output(output_path);
    for (input_index, time_block) in observations.time_blocks.iter().enumerate() {
        let frame = build_observation_frame(
            &cs,
            &results,
            &observations.values,
            input_index,
            *time_block,
            &options,
        )?;
        sink.write_observation(frame)?;
    }

    sink.finish()
}

fn alpha_engine() -> Result<AlphaEngine> {
    match env::var("QF_ENGINE") {
        Ok(value) if value == "dag" => Ok(AlphaEngine::Dag),
        Ok(value) if value == "tree" => Ok(AlphaEngine::Tree),
        Ok(value) => Err(QFactorsError::InvalidAlphaEngine(value)),
        Err(env::VarError::NotPresent) => Ok(AlphaEngine::Tree),
        Err(env::VarError::NotUnicode(value)) => Err(QFactorsError::InvalidAlphaEngine(
            value.to_string_lossy().into_owned(),
        )),
    }
}

fn eval_alphas_tree(
    resolved: Vec<(String, Expr)>,
    cs: &CellSet,
) -> Result<Vec<(String, Vec<f64>)>> {
    resolved
        .into_par_iter()
        .map(|(name, expr)| Ok((name, to_cells(eval(&expr, cs)?, Layout::Tn, cs))))
        .collect()
}

fn resolve_alphas(
    options: &ComputePanelOptions,
    alpha_names: Vec<String>,
) -> Result<Vec<(String, Expr)>> {
    let registry = alpha_registry()?;
    let mut output_names = HashSet::new();
    let mut resolved = Vec::with_capacity(alpha_names.len());

    for name in alpha_names {
        ensure_output_name_available(options, &mut output_names, &name)?;
        let descriptor = registry
            .get(&name)
            .ok_or_else(|| QFactorsError::UnknownFactor(name.clone()))?;
        resolved.push((name, (descriptor.build)()));
    }

    Ok(resolved)
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

#[allow(clippy::mutable_key_type)]
fn resolve_alpha_observations(
    df: &DataFrame,
    time_col: &str,
    cs: &CellSet,
    observation_times: Series,
) -> Result<ResolvedAlphaObservations> {
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

    let mut seen = HashSet::with_capacity(values.len());
    let mut time_blocks = Vec::with_capacity(values.len());
    for row in 0..values.len() {
        let value = values.get(row)?.into_static();
        if !seen.insert(value.clone()) {
            return Err(QFactorsError::DuplicateObservationTime(format!(
                "{value:?}"
            )));
        }
        time_blocks.push(cs.time_block_by_value.get(&value).copied());
    }

    Ok(ResolvedAlphaObservations {
        values,
        time_blocks,
    })
}

fn build_observation_frame(
    cs: &CellSet,
    results: &[(String, Vec<f64>)],
    observation_values: &Column,
    input_index: usize,
    time_block: Option<usize>,
    options: &ComputePanelOptions,
) -> Result<DataFrame> {
    let range = time_block
        .map(|idx| cs.time_blocks[idx].clone())
        .unwrap_or(0..0);
    let n_rows = range.len();

    let mut time = observation_values.new_from_index(input_index, n_rows);
    time.rename(options.time_col.clone().into());

    let mut symbol = cs.symbols_tn.slice(range.start as i64, n_rows);
    symbol.rename(options.symbol_col.clone().into());

    let mut columns = vec![time, symbol];
    for (name, values) in results {
        let column = if range.is_empty() {
            Column::new_empty(name.clone().into(), &DataType::Float64)
        } else {
            Column::new(name.clone().into(), values[range.clone()].to_vec())
        };
        columns.push(column);
    }

    Ok(DataFrame::new_infer_height(columns)?)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use linkme::distributed_slice;

    use super::*;
    use crate::alpha_registry::{ALPHA_DESCRIPTORS, AlphaDescriptor};

    fn test_alpha_build() -> Expr {
        Expr::Field("close".to_string())
    }

    fn test_alpha_descriptor() -> AlphaDescriptor {
        AlphaDescriptor {
            name: "test_alpha",
            build: test_alpha_build,
        }
    }

    #[distributed_slice(ALPHA_DESCRIPTORS)]
    static TEST_ALPHA_DESCRIPTOR: fn() -> AlphaDescriptor = test_alpha_descriptor;

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
    fn compute_alphas_samples_only_present_symbols() -> Result<()> {
        let df = df!(
            "asset" => ["B", "A", "A"],
            "time" => [2i64, 1, 2],
            "close" => [20.0, 10.0, 11.0],
        )?;

        let out = memory_frame(compute_alphas(
            df,
            options(),
            vec!["test_alpha".to_string()],
            Series::new("time".into(), [1i64, 2]),
            None,
        )?)?;

        assert_eq!(out.height(), 3);
        assert_eq!(
            out.column("test_alpha")?
                .try_f64()
                .expect("test_alpha is f64")
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            [10.0, 11.0, 20.0]
        );
        Ok(())
    }

    #[test]
    fn missing_observation_time_preserves_schema() -> Result<()> {
        let df = df!(
            "asset" => ["A"],
            "time" => [1i64],
            "close" => [10.0],
        )?;

        let out = memory_frame(compute_alphas(
            df,
            options(),
            vec!["test_alpha".to_string()],
            Series::new("time".into(), [9i64]),
            None,
        )?)?;

        assert_eq!(out.height(), 0);
        assert_eq!(
            out.get_column_names()
                .iter()
                .map(|name| name.to_string())
                .collect::<Vec<_>>(),
            ["time", "asset", "test_alpha"]
        );
        Ok(())
    }
}
