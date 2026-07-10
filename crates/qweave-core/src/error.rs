use polars::prelude::PolarsError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, QWeaveError>;

#[derive(Debug, Error)]
pub enum QWeaveError {
    #[error("missing column `{0}`")]
    MissingColumn(String),

    #[error("null values are not allowed in time column `{0}`")]
    TimeNull(String),

    #[error("null values are not allowed in symbol column `{0}`")]
    SymbolNull(String),

    #[error("NaN values are not allowed in structural column `{column}`")]
    NaNNotAllowed { column: String },

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

    #[error("factor `{0}` is not known")]
    UnknownFactor(String),

    #[error("output column `{0}` conflicts with another output column")]
    OutputColumnConflict(String),

    #[error("alpha expression is missing an alias")]
    AlphaAliasRequired,

    #[error("invalid QWEAVE_ENGINE `{0}`; expected `tree` or `dag`")]
    InvalidAlphaEngine(String),

    #[error("Polars error: {0}")]
    Polars(#[from] PolarsError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
