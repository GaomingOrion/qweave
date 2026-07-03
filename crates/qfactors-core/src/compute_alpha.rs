use std::collections::{BTreeSet, HashSet};
use std::env;

use polars::prelude::*;
use rayon::prelude::*;

use crate::alpha_dag::eval_exprs as eval_exprs_dag;
use crate::alpha_eval::{eval, to_cells};
use crate::cellset::{CellSet, PanelOptions, build_cellset};
use crate::compute_sink::{ComputeResult, ComputeSink};
use crate::error::{QFactorsError, Result};
use crate::expr::{Expr, collect_fields};
use crate::layout::Layout;

enum AlphaEngine {
    Tree,
    Dag,
}

pub fn compute_alphas(
    df: DataFrame,
    options: PanelOptions,
    alphas: Vec<(String, Expr)>,
    output_path: Option<&str>,
) -> Result<ComputeResult> {
    let (names, exprs) = prepare_alphas(&options, alphas, &HashSet::new())?;
    let cs = build_cellset(&df, &options, &fields_for(&exprs))?;
    let values = eval_exprs(&cs, &exprs)?;
    let frame = build_full_frame(&cs, names.into_iter().zip(values).collect(), &options)?;

    match output_path {
        None => Ok(ComputeResult::Memory(frame)),
        Some(_) => {
            let mut sink = ComputeSink::for_output(output_path);
            sink.write_observation(frame)?;
            sink.finish()
        }
    }
}

pub fn with_alphas(
    df: DataFrame,
    options: PanelOptions,
    alphas: Vec<(String, Expr)>,
) -> Result<DataFrame> {
    let input_names = df
        .get_column_names()
        .iter()
        .map(|name| name.to_string())
        .collect::<HashSet<_>>();
    let (names, exprs) = prepare_alphas(&options, alphas, &input_names)?;
    let cs = build_cellset(&df, &options, &fields_for(&exprs))?;
    let values = eval_exprs(&cs, &exprs)?;

    let mut columns = Vec::with_capacity(names.len());
    for (name, values_tn) in names.into_iter().zip(values) {
        let mut values_orig = vec![f64::NAN; df.height()];
        for (tn_index, value) in values_tn.into_iter().enumerate() {
            values_orig[cs.orig_index_tn[tn_index]] = value;
        }
        columns.push(Column::new(name.into(), values_orig));
    }

    let mut out = df;
    out.hstack_mut(&columns)?;
    Ok(out)
}

pub fn eval_exprs(cs: &CellSet, exprs: &[Expr]) -> Result<Vec<Vec<f64>>> {
    match alpha_engine()? {
        AlphaEngine::Tree => eval_exprs_tree(exprs, cs),
        AlphaEngine::Dag => eval_exprs_dag(exprs, cs),
    }
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

fn eval_exprs_tree(exprs: &[Expr], cs: &CellSet) -> Result<Vec<Vec<f64>>> {
    exprs
        .par_iter()
        .map(|expr| Ok(to_cells(eval(expr, cs)?, Layout::Tn, cs).into_owned()))
        .collect()
}

fn prepare_alphas(
    options: &PanelOptions,
    alphas: Vec<(String, Expr)>,
    input_names: &HashSet<String>,
) -> Result<(Vec<String>, Vec<Expr>)> {
    let mut output_names = HashSet::new();
    let mut names = Vec::with_capacity(alphas.len());
    let mut exprs = Vec::with_capacity(alphas.len());

    for (name, expr) in alphas {
        ensure_output_name_available(options, &mut output_names, input_names, &name)?;
        names.push(name);
        exprs.push(expr);
    }

    Ok((names, exprs))
}

fn ensure_output_name_available(
    options: &PanelOptions,
    seen: &mut HashSet<String>,
    input_names: &HashSet<String>,
    name: &str,
) -> Result<()> {
    if name == options.time_col
        || name == options.symbol_col
        || input_names.contains(name)
        || !seen.insert(name.to_string())
    {
        return Err(QFactorsError::OutputColumnConflict(name.to_string()));
    }
    Ok(())
}

fn fields_for(exprs: &[Expr]) -> BTreeSet<String> {
    let mut fields = BTreeSet::new();
    for expr in exprs {
        collect_fields(expr, &mut fields);
    }
    fields
}

/// Assemble the full-panel output by moving each alpha's Tn vector into a column
/// (no copy) and cloning the shared, cheap (Arc-backed) index columns.
fn build_full_frame(
    cs: &CellSet,
    results: Vec<(String, Vec<f64>)>,
    options: &PanelOptions,
) -> Result<DataFrame> {
    let mut columns = Vec::with_capacity(results.len() + 2);

    let mut time = cs.times_tn.clone();
    time.rename(options.time_col.clone().into());
    let mut symbol = cs.symbols_tn.clone();
    symbol.rename(options.symbol_col.clone().into());
    columns.push(time);
    columns.push(symbol);

    for (name, values) in results {
        columns.push(
            Float64Chunked::from_vec(name.into(), values)
                .into_series()
                .into_column(),
        );
    }

    Ok(DataFrame::new_infer_height(columns)?)
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

    fn memory_frame(result: ComputeResult) -> Result<DataFrame> {
        match result {
            ComputeResult::Memory(df) => Ok(df),
            ComputeResult::File(_) => panic!("expected memory result"),
        }
    }

    #[test]
    fn compute_alphas_outputs_full_tn_panel() -> Result<()> {
        let df = df!(
            "asset" => ["B", "A", "A"],
            "time" => [2i64, 1, 2],
            "close" => [20.0, 10.0, 11.0],
        )?;

        let out = memory_frame(compute_alphas(
            df,
            options(),
            vec![("test_alpha".to_string(), Expr::Field("close".to_string()))],
            None,
        )?)?;

        assert_eq!(out.height(), 3);
        assert_eq!(
            out.column("time")?
                .try_i64()
                .expect("time is i64")
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            [1, 2, 2]
        );
        assert_eq!(
            out.column("asset")?
                .try_str()
                .expect("asset is string")
                .iter()
                .collect::<Vec<_>>(),
            [Some("A"), Some("A"), Some("B")]
        );
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
    fn with_alphas_preserves_original_row_order() -> Result<()> {
        let df = df!(
            "asset" => ["B", "A", "A"],
            "time" => [2i64, 1, 2],
            "close" => [20.0, 10.0, 11.0],
        )?;

        let out = with_alphas(
            df,
            options(),
            vec![("close_copy".to_string(), Expr::Field("close".to_string()))],
        )?;

        assert_eq!(
            out.column("time")?
                .try_i64()
                .expect("time is i64")
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            [2, 1, 2]
        );
        assert_eq!(
            out.column("asset")?
                .try_str()
                .expect("asset is string")
                .iter()
                .collect::<Vec<_>>(),
            [Some("B"), Some("A"), Some("A")]
        );
        assert_eq!(
            out.column("close_copy")?
                .try_f64()
                .expect("close_copy is f64")
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            [20.0, 10.0, 11.0]
        );
        Ok(())
    }
}
