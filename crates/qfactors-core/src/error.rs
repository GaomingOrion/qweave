use polars::prelude::PolarsError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, QFactorsError>;

#[derive(Debug, Error)]
pub enum QFactorsError {
    #[error("missing column `{0}`")]
    MissingColumn(String),

    #[error("internal column `{0}` conflicts with input DataFrame")]
    InternalColumnConflict(&'static str),

    #[error("null values are not allowed in column `{column}`")]
    NullNotAllowed { column: String },

    #[error("null values are not allowed in time column `{0}`")]
    TimeNull(String),

    #[error("null values are not allowed in group column `{0}`")]
    GroupNull(String),

    #[error("null_policy `{0}` is not supported")]
    UnsupportedNullPolicy(String),

    #[error("column `{column}` has dtype {dtype}; expected Float64 for float_null_to_nan")]
    FloatNullToNanTypeMismatch { column: String, dtype: String },

    #[error("input must be sorted by [`{group_col}`, `{time_col}`] when sort=false")]
    SortOrder { group_col: String, time_col: String },

    #[error("duplicate (`{group_col}`, `{time_col}`) value found")]
    DuplicateGroupTime { group_col: String, time_col: String },

    #[error("Polars error: {0}")]
    Polars(#[from] PolarsError),
}
