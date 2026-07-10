use polars::prelude::DataFrame;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3_polars::PyDataFrame;
use qweave_core::PanelOptions;
use qweave_eval::{
    Binning, Demean, EvalOutput, EvaluateOptions, TableData, Weighting, evaluate as evaluate_core,
    factor_correlation as factor_correlation_core, save_output, to_html as to_html_core,
};
use qweave_server::{ReportData, run_server};

/// Result object for `evaluate`: Polars tables plus the parameter snapshot.
///
/// In memory mode every table is a `polars.DataFrame`; with `output_dir` set,
/// the large tables (`ic`, `quantile_returns`, `coverage`) are returned as
/// `polars.LazyFrame` scans over the streamed parquet files.
#[pyclass(name = "EvalResult", frozen)]
pub struct PyEvalResult {
    output: EvalOutput,
}

#[pymethods]
impl PyEvalResult {
    /// One row per (factor, horizon): IC/RankIC statistics, top-bottom spread,
    /// monotonicity, and coverage.
    #[getter]
    fn summary(&self) -> PyDataFrame {
        PyDataFrame(self.output.summary.clone())
    }

    /// Daily IC and RankIC: date, factor, horizon, ic, rank_ic.
    #[getter]
    fn ic(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        table_to_py(py, &self.output.ic)
    }

    /// Daily per-bucket rows: date, factor, bin, bin_lo, bin_hi, count,
    /// mean_ret_{h}...
    #[getter]
    fn quantile_returns(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        table_to_py(py, &self.output.quantile_returns)
    }

    /// Daily sample accounting per factor: date, factor, n_valid, n_masked.
    #[getter]
    fn coverage(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        table_to_py(py, &self.output.coverage)
    }

    /// Daily top/bottom quantile turnover: date, factor, horizon,
    /// top_turnover, bottom_turnover.
    #[getter]
    fn turnover(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        table_to_py(py, &self.output.turnover)
    }

    /// Staggered long-short portfolio: date, factor, horizon, gross, net,
    /// turnover (needs a ret_1 label; NaN otherwise).
    #[getter]
    fn portfolio(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        table_to_py(py, &self.output.portfolio)
    }

    /// Time-mean factor rank autocorrelation: factor, lag, rank_autocorr.
    #[getter]
    fn rank_autocorr(&self) -> PyDataFrame {
        PyDataFrame(self.output.rank_autocorr.clone())
    }

    /// Monthly IC means (only when the time column is Date/Datetime).
    #[getter]
    fn ic_monthly(&self) -> Option<PyDataFrame> {
        self.output.ic_monthly.clone().map(PyDataFrame)
    }

    /// Snapshot of every evaluation parameter, as a dict.
    #[getter]
    fn meta(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let json = py.import("json")?;
        Ok(json
            .call_method1("loads", (self.output.meta_json.as_str(),))?
            .unbind())
    }

    /// Write all tables plus meta.json to `dir` (memory mode only; streamed
    /// results already live in their output_dir).
    fn save(&self, py: Python<'_>, dir: &str) -> PyResult<()> {
        py.detach(|| save_output(&self.output, dir))
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    /// Write a self-contained HTML report (sortable summary table + per-factor
    /// quantile-return and monthly-IC charts) to `path`. Memory mode only;
    /// `max_detail_factors` caps the drill-down bundle to bound file size.
    #[pyo3(signature = (path, max_detail_factors = 200))]
    fn to_html(&self, py: Python<'_>, path: &str, max_detail_factors: usize) -> PyResult<()> {
        py.detach(|| to_html_core(&self.output, path, max_detail_factors))
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    /// Open an interactive report (summary table + per-factor Returns/IC
    /// tearsheets) in the default browser and block until the server is stopped
    /// (Ctrl-C). The server and UI are embedded — no external files needed.
    /// Memory mode only (call `save()` then `qweave-server --dir <dir>` for a
    /// streamed result).
    fn view(&self, py: Python<'_>) -> PyResult<()> {
        let data = report_data(&self.output)?;
        py.detach(|| run_server(data, 0, true, None))
            .map_err(|err| PyValueError::new_err(err.to_string()))
    }

    fn __repr__(&self) -> String {
        format!(
            "EvalResult(summary_rows={}, mode={})",
            self.output.summary.height(),
            match self.output.ic {
                TableData::Memory(_) => "memory",
                TableData::File(_) => "streamed",
            },
        )
    }
}

/// Assemble the in-memory report tables for the server. Memory mode only; a
/// streamed result already lives on disk and should be served with the CLI.
fn report_data(output: &EvalOutput) -> PyResult<ReportData> {
    let df = |table: &TableData| -> PyResult<DataFrame> {
        match table {
            TableData::Memory(df) => Ok(df.clone()),
            TableData::File(_) => Err(PyValueError::new_err(
                "view() requires an in-memory result; call save(dir) then \
                 run `qweave-server --dir <dir>` for a streamed result",
            )),
        }
    };
    ReportData::new(
        output.summary.clone(),
        df(&output.ic)?,
        df(&output.quantile_returns)?,
        df(&output.portfolio)?,
        output.ic_monthly.clone(),
        output.meta_json.clone(),
    )
    .map_err(|err| PyValueError::new_err(err.to_string()))
}

fn table_to_py(py: Python<'_>, table: &TableData) -> PyResult<Py<PyAny>> {
    match table {
        TableData::Memory(df) => Ok(PyDataFrame(df.clone()).into_pyobject(py)?.unbind()),
        TableData::File(path) => {
            let polars = py.import("polars")?;
            Ok(polars
                .call_method1("scan_parquet", (path.as_str(),))?
                .unbind())
        }
    }
}

/// Evaluate factor columns against `ret_{h}` label columns on a single panel
/// DataFrame (see `with_alphas` / `with_labels` for producing the inputs).
#[pyfunction(name = "evaluate", signature = (
    df,
    symbol_col,
    time_col,
    factor_cols,
    label_cols = None,
    quantiles = 10,
    binning = "daily",
    group_col = None,
    tradable_col = None,
    demean = "none",
    min_cs_count = 30,
    cost_bps = 0.0,
    weighting = "quantile",
    factor_source = None,
    output_dir = None
))]
#[allow(clippy::too_many_arguments)]
pub fn evaluate_py(
    py: Python<'_>,
    df: PyDataFrame,
    symbol_col: &str,
    time_col: &str,
    factor_cols: Vec<String>,
    label_cols: Option<Vec<String>>,
    quantiles: usize,
    binning: &str,
    group_col: Option<String>,
    tradable_col: Option<String>,
    demean: &str,
    min_cs_count: usize,
    cost_bps: f64,
    weighting: &str,
    factor_source: Option<String>,
    output_dir: Option<String>,
) -> PyResult<PyEvalResult> {
    let panel = PanelOptions {
        symbol_col: symbol_col.to_string(),
        time_col: time_col.to_string(),
    };
    let binning = match binning {
        "daily" => Binning::Daily,
        "global" => Binning::Global,
        other => {
            return Err(PyValueError::new_err(format!(
                "binning must be \"daily\" or \"global\"; got {other:?}"
            )));
        }
    };
    let demean = match demean {
        "none" => Demean::None,
        "universe" => Demean::Universe,
        "group" => Demean::Group,
        other => {
            return Err(PyValueError::new_err(format!(
                "demean must be \"none\", \"universe\", or \"group\"; got {other:?}"
            )));
        }
    };
    let weighting = match weighting {
        "factor" => Weighting::Factor,
        "quantile" => Weighting::Quantile,
        other => {
            return Err(PyValueError::new_err(format!(
                "weighting must be \"factor\" or \"quantile\"; got {other:?}"
            )));
        }
    };
    let options = EvaluateOptions {
        factor_cols,
        label_cols,
        quantiles,
        binning,
        demean,
        min_cs_count,
        group_col,
        tradable_col,
        cost_bps,
        weighting,
        factor_source,
        output_dir,
    };

    let output = py
        .detach(move || evaluate_core(&df.into(), &panel, &options))
        .map_err(|err| PyValueError::new_err(err.to_string()))?;
    Ok(PyEvalResult { output })
}

/// Time-averaged daily cross-sectional rank correlation between factors
/// (pairwise complete observations). Intended for the filtered shortlist
/// after `evaluate`: every factor column is held densely in memory.
#[pyfunction(name = "factor_correlation", signature = (
    df,
    symbol_col,
    time_col,
    factor_cols,
    tradable_col = None,
    min_cs_count = 30
))]
pub fn factor_correlation_py(
    py: Python<'_>,
    df: PyDataFrame,
    symbol_col: &str,
    time_col: &str,
    factor_cols: Vec<String>,
    tradable_col: Option<String>,
    min_cs_count: usize,
) -> PyResult<PyDataFrame> {
    let panel = PanelOptions {
        symbol_col: symbol_col.to_string(),
        time_col: time_col.to_string(),
    };
    py.detach(move || {
        factor_correlation_core(
            &df.into(),
            &panel,
            &factor_cols,
            tradable_col.as_deref(),
            min_cs_count,
        )
    })
    .map(PyDataFrame)
    .map_err(|err| PyValueError::new_err(err.to_string()))
}
