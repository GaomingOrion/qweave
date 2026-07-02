use std::collections::{BTreeMap, HashMap};

use polars::prelude::Series;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};
use pyo3_polars::{PyDataFrame, PySeries};
use qfactors_core::alpha_registry::{AlphaRegistry, alpha_registry};
use qfactors_core::{
    ComputePanelOptions, ComputeResult, ComputeSummary, Expr, QFactorsError,
    compute_alphas as compute_alphas_core, compute_panel as compute_panel_core, factor_catalog,
    with_alphas as with_alphas_core,
};

mod expr;
use expr::PyExpr;

// Rust-side allocations (the large per-node `Vec<f64>` buffers in the alpha engine) go
// through jemalloc on unix and mimalloc (v3, the crate default) on Windows. This only
// affects allocations made inside the extension module, not Python's own allocator.
#[cfg(not(target_os = "windows"))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[cfg(target_os = "windows")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Return the input Polars DataFrame unchanged.
#[pyfunction]
fn roundtrip(df: PyDataFrame) -> PyDataFrame {
    df
}

/// Compute registered factor kernels on a Polars panel.
///
/// The input DataFrame must contain the symbol and time columns plus every field
/// required by the requested factors. Results are sampled at `observation_times`.
/// Float input nulls become NaN; structural columns and observation times must
/// not contain nulls. If `output_path` is set, the result is written as Parquet
/// and a summary dict is returned. Otherwise a Polars DataFrame is returned.
#[pyfunction(name = "compute_panel", signature = (
    df,
    symbol_col,
    time_col,
    factors,
    observation_times,
    column_aliases = None,
    output_path = None
))]
#[allow(clippy::too_many_arguments)]
fn compute_panel_py(
    py: Python<'_>,
    df: PyDataFrame,
    symbol_col: &str,
    time_col: &str,
    factors: Vec<String>,
    observation_times: &Bound<'_, PyAny>,
    column_aliases: Option<HashMap<String, String>>,
    output_path: Option<&str>,
) -> PyResult<Py<PyAny>> {
    let observation_times = observation_series_from_py(py, observation_times)?;
    let options = ComputePanelOptions {
        symbol_col: symbol_col.to_string(),
        time_col: time_col.to_string(),
        column_aliases: column_aliases.unwrap_or_default(),
    };

    let result = compute_panel_core(df.into(), options, factors, observation_times, output_path)
        .map_err(to_py_err)?;
    match result {
        ComputeResult::Memory(df) => Ok(PyDataFrame(df).into_pyobject(py)?.unbind()),
        ComputeResult::File(summary) => summary_to_py(py, summary),
    }
}

/// Return metadata for all registered factor kernels.
#[pyfunction(name = "factor_catalog")]
fn factor_catalog_py() -> PyResult<PyDataFrame> {
    qfactors_factors::ensure_linked();
    factor_catalog().map(PyDataFrame).map_err(to_py_err)
}

/// Compute registered alpha expressions on a Polars panel.
///
/// The input DataFrame must contain the symbol and time columns plus every field
/// required by the requested alphas. The result always contains the full
/// (time, symbol) panel. Float input nulls become NaN; structural columns must
/// not contain nulls. If `output_path` is set, the result is written as Parquet
/// and a summary dict is returned. Otherwise a Polars DataFrame is returned.
#[pyfunction(name = "compute_alphas", signature = (
    df,
    symbol_col,
    time_col,
    alphas,
    output_path = None
))]
fn compute_alphas_py(
    py: Python<'_>,
    df: PyDataFrame,
    symbol_col: &str,
    time_col: &str,
    alphas: Vec<Py<PyExpr>>,
    output_path: Option<&str>,
) -> PyResult<Py<PyAny>> {
    let options = ComputePanelOptions {
        symbol_col: symbol_col.to_string(),
        time_col: time_col.to_string(),
        column_aliases: HashMap::new(),
    };
    let alphas = alpha_specs_from_py(py, alphas).map_err(to_py_err)?;

    let result = compute_alphas_core(df.into(), options, alphas, output_path).map_err(to_py_err)?;
    match result {
        ComputeResult::Memory(df) => Ok(PyDataFrame(df).into_pyobject(py)?.unbind()),
        ComputeResult::File(summary) => summary_to_py(py, summary),
    }
}

/// Append alpha expression outputs to the input DataFrame in original row order.
#[pyfunction(name = "with_alphas", signature = (
    df,
    symbol_col,
    time_col,
    alphas
))]
fn with_alphas_py(
    py: Python<'_>,
    df: PyDataFrame,
    symbol_col: &str,
    time_col: &str,
    alphas: Vec<Py<PyExpr>>,
) -> PyResult<PyDataFrame> {
    let options = ComputePanelOptions {
        symbol_col: symbol_col.to_string(),
        time_col: time_col.to_string(),
        column_aliases: HashMap::new(),
    };
    let alphas = alpha_specs_from_py(py, alphas).map_err(to_py_err)?;

    with_alphas_core(df.into(), options, alphas)
        .map(PyDataFrame)
        .map_err(to_py_err)
}

#[pyfunction(name = "_worldquant101_alphas")]
fn worldquant101_alphas_dict_py(py: Python<'_>) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    for (name, expr) in worldquant101_exprs().map_err(to_py_err)? {
        dict.set_item(&name, PyExpr::named(&name, expr))?;
    }
    Ok(dict.into_any().unbind())
}

#[pyfunction(name = "worldquant101_alphas", signature = (input_alias, alphas = None))]
fn worldquant101_alphas_py(
    input_alias: HashMap<String, String>,
    alphas: Option<Vec<String>>,
) -> PyResult<Vec<PyExpr>> {
    let input_alias: BTreeMap<String, String> = input_alias.into_iter().collect();
    let selected = match alphas {
        Some(names) => {
            let registry = worldquant101_registry().map_err(to_py_err)?;
            names
                .into_iter()
                .map(|name| worldquant101_expr(registry, &name))
                .collect::<qfactors_core::Result<Vec<_>>>()
                .map_err(to_py_err)?
        }
        None => worldquant101_exprs().map_err(to_py_err)?,
    };

    Ok(selected
        .into_iter()
        .map(|(name, expr)| {
            PyExpr::named(
                &name,
                qfactors_core::expr::rename_fields(&expr, &input_alias),
            )
        })
        .collect())
}

fn observation_series_from_py(
    py: Python<'_>,
    observation_times: &Bound<'_, PyAny>,
) -> PyResult<Series> {
    if let Ok(series) = observation_times.extract::<PySeries>() {
        return Ok(series.0);
    }

    let polars = PyModule::import(py, "polars")?;
    let series = polars.getattr("Series")?.call1((observation_times,))?;
    Ok(series.extract::<PySeries>()?.0)
}

fn summary_to_py(py: Python<'_>, summary: ComputeSummary) -> PyResult<Py<PyAny>> {
    let dict = PyDict::new(py);
    dict.set_item("output_path", summary.output_path)?;
    dict.set_item("n_observations", summary.n_observations)?;
    dict.set_item("n_rows", summary.n_rows)?;
    Ok(dict.into_any().unbind())
}

fn alpha_specs_from_py(
    py: Python<'_>,
    alphas: Vec<Py<PyExpr>>,
) -> qfactors_core::Result<Vec<(String, Expr)>> {
    alphas
        .into_iter()
        .map(|alpha| {
            let alpha = alpha.borrow(py);
            let name = alpha
                .output_name()
                .ok_or(QFactorsError::AlphaAliasRequired)?
                .to_string();
            Ok((name, alpha.expr()))
        })
        .collect()
}

fn worldquant101_registry() -> qfactors_core::Result<&'static AlphaRegistry> {
    qfactors_factors::ensure_linked();
    alpha_registry()
}

fn worldquant101_exprs() -> qfactors_core::Result<Vec<(String, Expr)>> {
    let registry = worldquant101_registry()?;
    (1..=101)
        .map(|idx| worldquant101_expr(registry, &format!("alpha{idx}")))
        .collect()
}

fn worldquant101_expr(
    registry: &AlphaRegistry,
    name: &str,
) -> qfactors_core::Result<(String, Expr)> {
    if !is_worldquant101_name(name) {
        return Err(QFactorsError::UnknownFactor(name.to_string()));
    }
    let descriptor = registry
        .get(name)
        .ok_or_else(|| QFactorsError::UnknownFactor(name.to_string()))?;
    Ok((name.to_string(), (descriptor.build)()))
}

fn is_worldquant101_name(name: &str) -> bool {
    alpha_number(name).is_some_and(|number| (1..=101).contains(&number))
}

fn alpha_number(name: &str) -> Option<usize> {
    name.strip_prefix("alpha")?.parse().ok()
}

#[pymodule]
fn qfactors(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    qfactors_factors::ensure_linked();
    expr::register(module)?;
    module.add_function(wrap_pyfunction!(roundtrip, module)?)?;
    module.add_function(wrap_pyfunction!(compute_panel_py, module)?)?;
    module.add_function(wrap_pyfunction!(factor_catalog_py, module)?)?;
    module.add_function(wrap_pyfunction!(compute_alphas_py, module)?)?;
    module.add_function(wrap_pyfunction!(with_alphas_py, module)?)?;
    module.add_function(wrap_pyfunction!(worldquant101_alphas_dict_py, module)?)?;
    module.add_function(wrap_pyfunction!(worldquant101_alphas_py, module)?)?;
    Ok(())
}

fn to_py_err(err: QFactorsError) -> PyErr {
    PyValueError::new_err(err.to_string())
}
