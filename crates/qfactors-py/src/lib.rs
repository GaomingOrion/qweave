use std::collections::HashMap;

use polars::prelude::Series;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict};
use pyo3_polars::{PyDataFrame, PySeries};
use qfactors_core::{
    ComputeResult, ComputeSummary, NullPolicy, PreparePanelOptions, PreparedPanel, QFactorsError,
    compute_panel, factor_catalog,
};

#[pyclass(name = "PreparedPanel", unsendable)]
struct PyPreparedPanel {
    inner: PreparedPanel,
}

#[pymethods]
impl PyPreparedPanel {
    #[getter]
    fn height(&self) -> usize {
        self.inner.dataframe().height()
    }

    #[getter]
    fn group_count(&self) -> usize {
        self.inner.groups().len()
    }

    #[getter]
    fn group_col(&self) -> String {
        self.inner.group_col().to_string()
    }

    #[getter]
    fn time_col(&self) -> String {
        self.inner.time_col().to_string()
    }

    fn to_frame(&self) -> PyDataFrame {
        PyDataFrame(self.inner.dataframe().clone())
    }

    #[pyo3(signature = (observation_times, factors, output_path = None))]
    fn compute_panel(
        &self,
        py: Python<'_>,
        observation_times: &Bound<'_, PyAny>,
        factors: Vec<String>,
        output_path: Option<&str>,
    ) -> PyResult<Py<PyAny>> {
        let observation_times = observation_series_from_py(py, observation_times)?;
        let result = compute_panel(&self.inner, observation_times, factors, output_path)
            .map_err(to_py_err)?;
        match result {
            ComputeResult::Memory(df) => Ok(PyDataFrame(df).into_pyobject(py)?.unbind()),
            ComputeResult::File(summary) => summary_to_py(py, summary),
        }
    }
}

#[pyfunction]
fn roundtrip(df: PyDataFrame) -> PyDataFrame {
    df
}

#[pyfunction(signature = (
    df,
    group_col,
    time_col,
    column_aliases = None,
    sort = true,
    rechunk = true,
    null_policy = "error",
    output_group_id = false
))]
fn prepare_panel(
    df: PyDataFrame,
    group_col: &str,
    time_col: &str,
    column_aliases: Option<HashMap<String, String>>,
    sort: bool,
    rechunk: bool,
    null_policy: &str,
    output_group_id: bool,
) -> PyResult<PyPreparedPanel> {
    let options = PreparePanelOptions {
        group_col: group_col.to_string(),
        time_col: time_col.to_string(),
        column_aliases: column_aliases.unwrap_or_default(),
        sort,
        rechunk,
        null_policy: NullPolicy::parse(null_policy).map_err(to_py_err)?,
        output_group_id,
    };

    let panel = PreparedPanel::new(df.into(), options).map_err(to_py_err)?;
    Ok(PyPreparedPanel { inner: panel })
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
    module.add_class::<PyPreparedPanel>()?;
    module.add_function(wrap_pyfunction!(roundtrip, module)?)?;
    module.add_function(wrap_pyfunction!(prepare_panel, module)?)?;
    module.add_function(wrap_pyfunction!(factor_catalog_py, module)?)?;
    Ok(())
}

fn to_py_err(err: QFactorsError) -> PyErr {
    PyValueError::new_err(err.to_string())
}
