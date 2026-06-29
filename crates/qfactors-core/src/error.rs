use polars::prelude::PolarsError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, QFactorsError>;

#[derive(Debug, Error)]
pub enum QFactorsError {
    #[error("missing column `{0}`")]
    MissingColumn(String),

    #[error("null values are not allowed in column `{column}`")]
    NullNotAllowed { column: String },

    #[error("null values are not allowed in time column `{0}`")]
    TimeNull(String),

    #[error("null values are not allowed in symbol column `{0}`")]
    SymbolNull(String),

    #[error("NaN values are not allowed in structural column `{column}`")]
    NaNNotAllowed { column: String },

    #[error("column `{column}` cannot be ordered")]
    NonComparableColumn { column: String },

    #[error("duplicate (`{symbol_col}`, `{time_col}`) value found")]
    DuplicateSymbolTime {
        symbol_col: String,
        time_col: String,
    },

    #[error("column `{column}` has dtype {actual}; expected {expected}")]
    DTypeMismatch {
        column: String,
        expected: &'static str,
        actual: String,
    },

    #[error("column `{0}` is not contiguous; prepare with rechunk=True")]
    NonContiguousColumn(String),

    #[error("factor `{0}` is not known")]
    UnknownFactor(String),

    #[error("factor `{0}` is registered more than once")]
    DuplicateFactorName(String),

    #[error("factor `{factor_name}` has invalid window {window}")]
    InvalidWindow {
        factor_name: &'static str,
        window: usize,
    },

    #[error("duplicate observation time `{0}`")]
    DuplicateObservationTime(String),

    #[error("observation_times cannot be empty")]
    ObservationTimesEmpty,

    #[error("null values are not allowed in observation_times")]
    ObservationTimeNull,

    #[error("output column `{0}` conflicts with another output column")]
    OutputColumnConflict(String),

    #[error("invalid QF_ENGINE `{0}`; expected `tree` or `dag`")]
    InvalidAlphaEngine(String),

    #[error("factor `{factor_name}` returned {actual} columns; expected {expected}")]
    FactorOutputCount {
        factor_name: &'static str,
        expected: usize,
        actual: usize,
    },

    #[error(
        "factor `{factor_name}` output column `{column}` has length {actual}; expected {expected}"
    )]
    FactorOutputLength {
        factor_name: &'static str,
        column: String,
        expected: usize,
        actual: usize,
    },

    #[error("factor `{factor_name}` output column `{actual}` should be `{expected}`")]
    FactorOutputName {
        factor_name: &'static str,
        expected: String,
        actual: String,
    },

    #[error("Polars error: {0}")]
    Polars(#[from] PolarsError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
