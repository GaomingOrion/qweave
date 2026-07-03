use polars::prelude::PolarsError;
use qfactors_core::QFactorsError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, EvalError>;

#[derive(Debug, Error)]
pub enum EvalError {
    #[error("horizons must be non-empty, positive, and unique; got {0}")]
    InvalidHorizons(String),

    #[error("calendar dtype {calendar} does not match time column dtype {time}")]
    CalendarDTypeMismatch { calendar: String, time: String },

    #[error("calendar must be strictly increasing without duplicates")]
    CalendarNotSorted,

    #[error("panel time `{0}` is not in the provided calendar")]
    TimeNotInCalendar(String),

    #[error("column `{column}` has dtype {actual}; expected {expected}")]
    DTypeMismatch {
        column: String,
        expected: &'static str,
        actual: String,
    },

    #[error("output column `{0}` already exists in the input DataFrame")]
    OutputColumnConflict(String),

    #[error(transparent)]
    Core(#[from] QFactorsError),

    #[error("Polars error: {0}")]
    Polars(#[from] PolarsError),
}
