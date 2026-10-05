use std::collections::{BTreeMap, BTreeSet};

use pyo3::class::basic::CompareOp;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PySet;
use qweave_core::Expr;
use qweave_core::alpha;
use qweave_core::expr::{collect_fields, rename_fields};

#[pyclass(module = "qweave", skip_from_py_object)]
#[derive(Clone)]
pub(crate) struct PyExpr {
    inner: Expr,
    alias: Option<String>,
}

impl PyExpr {
    pub(crate) fn new(inner: Expr) -> Self {
        Self { inner, alias: None }
    }

    pub(crate) fn named(name: &str, expr: Expr) -> Self {
        Self {
            inner: expr,
            alias: Some(name.to_string()),
        }
    }

    pub(crate) fn output_name_ref(&self) -> Option<&str> {
        self.alias.as_deref()
    }

    pub(crate) fn expr(&self) -> Expr {
        self.inner.clone()
    }

    fn unary(&self, op: impl FnOnce(Expr) -> Expr) -> Self {
        Self::new(op(self.inner.clone()))
    }

    fn binary(&self, rhs: &PyExpr, op: impl FnOnce(Expr, Expr) -> Expr) -> Self {
        Self::new(op(self.inner.clone(), rhs.inner.clone()))
    }
}

#[pymethods]
impl PyExpr {
    fn alias(&self, name: &str) -> Self {
        Self {
            inner: self.inner.clone(),
            alias: Some(name.to_string()),
        }
    }

    fn collect_inputs<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PySet>> {
        let mut fields = BTreeSet::new();
        collect_fields(&self.inner, &mut fields);
        PySet::new(py, fields)
    }

    fn output_name(&self) -> Option<String> {
        self.alias.clone()
    }

    fn replace_inputs(&self, names: BTreeMap<String, String>) -> Self {
        Self {
            inner: rename_fields(&self.inner, &names),
            alias: self.alias.clone(),
        }
    }

    fn neg(&self) -> Self {
        self.unary(|x| -x)
    }

    fn abs(&self) -> Self {
        self.unary(alpha::abs)
    }

    fn log(&self) -> Self {
        self.unary(alpha::log)
    }

    fn sign(&self) -> Self {
        self.unary(alpha::sign)
    }

    fn rank(&self) -> Self {
        self.unary(alpha::rank)
    }

    #[pyo3(signature = (scale_to = 1.0))]
    fn scale(&self, scale_to: f64) -> Self {
        self.unary(|x| alpha::scale(x, scale_to))
    }

    fn delay(&self, days: usize) -> Self {
        self.unary(|x| alpha::delay(x, days))
    }

    fn delta(&self, days: usize) -> Self {
        self.unary(|x| alpha::delta(x, days))
    }

    fn ts_sum(&self, days: usize) -> Self {
        self.unary(|x| alpha::ts_sum(x, days))
    }

    fn ts_mean(&self, days: usize) -> Self {
        self.unary(|x| alpha::ts_mean(x, days))
    }

    fn product(&self, days: usize) -> Self {
        self.unary(|x| alpha::product(x, days))
    }

    fn ts_min(&self, days: usize) -> Self {
        self.unary(|x| alpha::ts_min(x, days))
    }

    fn ts_max(&self, days: usize) -> Self {
        self.unary(|x| alpha::ts_max(x, days))
    }

    fn ts_argmin(&self, days: usize) -> Self {
        self.unary(|x| alpha::ts_argmin(x, days))
    }

    fn ts_argmax(&self, days: usize) -> Self {
        self.unary(|x| alpha::ts_argmax(x, days))
    }

    fn ts_rank(&self, days: usize) -> Self {
        self.unary(|x| alpha::ts_rank(x, days))
    }

    fn ts_rank_raw(&self, days: usize) -> Self {
        self.unary(|x| alpha::ts_rank_raw(x, days))
    }

    fn ts_std(&self, days: usize) -> Self {
        self.unary(|x| alpha::ts_std(x, days))
    }

    /// Bias-corrected sample skewness over ``days`` bars.
    ///
    /// Parameters
    /// ----------
    /// days : int
    ///     Nonnegative period in bars. Requires days >= 3 and a full finite
    ///     window. Constant windows yield 0.
    ///
    /// Returns
    /// -------
    /// PyExpr
    ///     A new, unaliased expression; no data is evaluated at construction.
    ///     Zero or insufficient periods yield NaN when evaluated.
    ///
    /// Raises
    /// ------
    /// OverflowError
    ///     If days is negative or outside the platform's unsigned integer range.
    /// TypeError
    ///     If days is not an integer.
    fn ts_skew(&self, days: usize) -> Self {
        self.unary(|x| alpha::ts_skew(x, days))
    }

    /// Bias-corrected Fisher excess kurtosis over ``days`` bars.
    ///
    /// Parameters
    /// ----------
    /// days : int
    ///     Nonnegative period in bars. Requires days >= 4 and a full finite
    ///     window. Constant windows yield -3.
    ///
    /// Returns
    /// -------
    /// PyExpr
    ///     A new, unaliased expression; no data is evaluated at construction.
    ///     Zero or insufficient periods yield NaN when evaluated.
    ///
    /// Raises
    /// ------
    /// OverflowError
    ///     If days is negative or outside the platform's unsigned integer range.
    /// TypeError
    ///     If days is not an integer.
    fn ts_kurt(&self, days: usize) -> Self {
        self.unary(|x| alpha::ts_kurt(x, days))
    }

    /// Mean-seeded exponential moving average over each symbol.
    ///
    /// Parameters
    /// ----------
    /// days : int
    ///     Nonnegative period in bars. Uses alpha = 2/(days+1). The first days
    ///     consecutive finite samples seed the mean; non-finite samples reset
    ///     the state and restart warmup.
    ///
    /// Returns
    /// -------
    /// PyExpr
    ///     A new, unaliased expression; no data is evaluated at construction.
    ///     Zero or insufficient periods yield NaN when evaluated.
    ///
    /// Raises
    /// ------
    /// OverflowError
    ///     If days is negative or outside the platform's unsigned integer range.
    /// TypeError
    ///     If days is not an integer.
    fn ema(&self, days: usize) -> Self {
        self.unary(|x| alpha::ema(x, days))
    }

    /// Linearly weighted moving average over ``days`` bars.
    ///
    /// Parameters
    /// ----------
    /// days : int
    ///     Nonnegative period in bars. Weights are 1..days from oldest to newest.
    ///     Equivalent to decay_linear; requires a full NaN-free window.
    ///
    /// Returns
    /// -------
    /// PyExpr
    ///     A new, unaliased expression; no data is evaluated at construction.
    ///     Zero or insufficient periods yield NaN when evaluated.
    ///
    /// Raises
    /// ------
    /// OverflowError
    ///     If days is negative or outside the platform's unsigned integer range.
    /// TypeError
    ///     If days is not an integer.
    fn wma(&self, days: usize) -> Self {
        self.unary(|x| alpha::wma(x, days))
    }

    fn slope(&self, days: usize) -> Self {
        self.unary(|x| alpha::slope(x, days))
    }

    fn rsquare(&self, days: usize) -> Self {
        self.unary(|x| alpha::rsquare(x, days))
    }

    fn resi(&self, days: usize) -> Self {
        self.unary(|x| alpha::resi(x, days))
    }

    fn quantile(&self, days: usize, q: f64) -> Self {
        self.unary(|x| alpha::quantile(x, days, q))
    }

    fn decay_linear(&self, days: usize) -> Self {
        self.unary(|x| alpha::decay_linear(x, days))
    }

    fn __add__(&self, rhs: PyRef<'_, PyExpr>) -> Self {
        self.binary(&rhs, |lhs, rhs| lhs + rhs)
    }

    fn __sub__(&self, rhs: PyRef<'_, PyExpr>) -> Self {
        self.binary(&rhs, |lhs, rhs| lhs - rhs)
    }

    fn __mul__(&self, rhs: PyRef<'_, PyExpr>) -> Self {
        self.binary(&rhs, |lhs, rhs| lhs * rhs)
    }

    fn __truediv__(&self, rhs: PyRef<'_, PyExpr>) -> Self {
        self.binary(&rhs, |lhs, rhs| lhs / rhs)
    }

    fn __neg__(&self) -> Self {
        self.neg()
    }

    fn __richcmp__(&self, rhs: PyRef<'_, PyExpr>, op: CompareOp) -> PyResult<Self> {
        match op {
            CompareOp::Lt => Ok(self.binary(&rhs, alpha::lt)),
            CompareOp::Gt => Ok(self.binary(&rhs, alpha::gt)),
            CompareOp::Le => Ok(self.binary(&rhs, alpha::le)),
            CompareOp::Ge => Ok(self.binary(&rhs, alpha::ge)),
            CompareOp::Eq => Ok(self.binary(&rhs, alpha::eq)),
            CompareOp::Ne => Err(PyValueError::new_err("`!=` is not supported for PyExpr")),
        }
    }

    fn __repr__(&self) -> String {
        match &self.alias {
            Some(alias) => format!("PyExpr(alias={alias:?}, expr={})", self.inner),
            None => format!("PyExpr(expr={})", self.inner),
        }
    }
}

#[pyfunction]
pub(crate) fn col(name: &str) -> PyExpr {
    PyExpr::new(alpha::col(name))
}

#[pyfunction]
pub(crate) fn lit(value: f64) -> PyExpr {
    PyExpr::new(alpha::lit(value))
}

#[pyfunction]
pub(crate) fn min(lhs: PyRef<'_, PyExpr>, rhs: PyRef<'_, PyExpr>) -> PyExpr {
    lhs.binary(&rhs, alpha::min)
}

#[pyfunction]
pub(crate) fn max(lhs: PyRef<'_, PyExpr>, rhs: PyRef<'_, PyExpr>) -> PyExpr {
    lhs.binary(&rhs, alpha::max)
}

#[pyfunction]
pub(crate) fn power(lhs: PyRef<'_, PyExpr>, rhs: PyRef<'_, PyExpr>) -> PyExpr {
    lhs.binary(&rhs, alpha::power)
}

#[pyfunction]
pub(crate) fn signed_power(lhs: PyRef<'_, PyExpr>, rhs: PyRef<'_, PyExpr>) -> PyExpr {
    lhs.binary(&rhs, alpha::signed_power)
}

#[pyfunction]
pub(crate) fn correlation(lhs: PyRef<'_, PyExpr>, rhs: PyRef<'_, PyExpr>, days: usize) -> PyExpr {
    lhs.binary(&rhs, |lhs, rhs| alpha::correlation(lhs, rhs, days))
}

#[pyfunction]
pub(crate) fn covariance(lhs: PyRef<'_, PyExpr>, rhs: PyRef<'_, PyExpr>, days: usize) -> PyExpr {
    lhs.binary(&rhs, |lhs, rhs| alpha::covariance(lhs, rhs, days))
}

#[pyfunction]
pub(crate) fn group_rank(lhs: PyRef<'_, PyExpr>, rhs: PyRef<'_, PyExpr>) -> PyExpr {
    lhs.binary(&rhs, alpha::group_rank)
}

#[pyfunction]
pub(crate) fn group_neutralize(lhs: PyRef<'_, PyExpr>, rhs: PyRef<'_, PyExpr>) -> PyExpr {
    lhs.binary(&rhs, alpha::group_neutralize)
}

#[pyfunction(name = "where_")]
pub(crate) fn where_py(
    cond: PyRef<'_, PyExpr>,
    when_true: PyRef<'_, PyExpr>,
    when_false: PyRef<'_, PyExpr>,
) -> PyExpr {
    PyExpr::new(alpha::where_(
        cond.inner.clone(),
        when_true.inner.clone(),
        when_false.inner.clone(),
    ))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<PyExpr>()?;
    module.add_function(wrap_pyfunction!(col, module)?)?;
    module.add_function(wrap_pyfunction!(lit, module)?)?;
    module.add_function(wrap_pyfunction!(min, module)?)?;
    module.add_function(wrap_pyfunction!(max, module)?)?;
    module.add_function(wrap_pyfunction!(power, module)?)?;
    module.add_function(wrap_pyfunction!(signed_power, module)?)?;
    module.add_function(wrap_pyfunction!(correlation, module)?)?;
    module.add_function(wrap_pyfunction!(covariance, module)?)?;
    module.add_function(wrap_pyfunction!(group_rank, module)?)?;
    module.add_function(wrap_pyfunction!(group_neutralize, module)?)?;
    module.add_function(wrap_pyfunction!(where_py, module)?)?;
    Ok(())
}
