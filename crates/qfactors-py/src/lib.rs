use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3_polars::PyDataFrame;
use qfactors_core::{NullPolicy, PreparePanelOptions, PreparedPanel, QFactorsError};

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

#[pymodule]
fn qfactors(_py: Python<'_>, module: &Bound<'_, PyModule>) -> PyResult<()> {
    qfactors_factors::ensure_linked();
    module.add_class::<PyPreparedPanel>()?;
    module.add_function(wrap_pyfunction!(roundtrip, module)?)?;
    module.add_function(wrap_pyfunction!(prepare_panel, module)?)?;
    Ok(())
}

fn to_py_err(err: QFactorsError) -> PyErr {
    PyValueError::new_err(err.to_string())
}
