use std::collections::HashMap;

use polars::prelude::Series;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};
use pyo3_polars::{PyDataFrame, PySeries};
use qfactors_core::{
    ComputePanelOptions, ComputeResult, ComputeSummary, QFactorsError,
    compute_panel as compute_panel_core, factor_catalog,
};

#[pyfunction]
fn roundtrip(df: PyDataFrame) -> PyDataFrame {
    df
}

#[pyfunction(name = "compute_panel", signature = (
    df,
    symbol_col,
    time_col,
    factors,
    observation_times,
    column_aliases = None,
    output_path = None
))]
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

#[pyfunction(name = "factor_catalog")]
fn factor_catalog_py() -> PyResult<PyDataFrame> {
    qfactors_factors::ensure_linked();
    factor_catalog().map(PyDataFrame).map_err(to_py_err)
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

#[pymodule]
fn qfactors(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    qfactors_factors::ensure_linked();
    module.add_function(wrap_pyfunction!(roundtrip, module)?)?;
    module.add_function(wrap_pyfunction!(compute_panel_py, module)?)?;
    module.add_function(wrap_pyfunction!(factor_catalog_py, module)?)?;
    Ok(())
}

fn to_py_err(err: QFactorsError) -> PyErr {
    PyValueError::new_err(err.to_string())
}
