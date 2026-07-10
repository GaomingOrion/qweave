use polars::prelude::PolarsError;
use qweave_core::QWeaveError;
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

    #[error("demean=\"group\" requires group_col")]
    GroupColumnRequired,

    #[error("null values are not allowed in group column `{0}`")]
    GroupNull(String),

    #[error("no label columns found: pass label_cols or add `ret_{{h}}` columns (see with_labels)")]
    NoLabelColumns,

    #[error("label column `{0}` must be named `ret_{{h}}` with an integer horizon")]
    BadLabelColumn(String),

    #[error("factor_cols must be non-empty and unique; `{0}` is invalid")]
    BadFactorColumns(String),

    #[error("quantiles must be at least 2; got {0}")]
    InvalidQuantiles(usize),

    #[error("result tables were streamed to `{0}`; save() is only available in memory mode")]
    AlreadySaved(String),

    #[error("factor_source panel does not match the DataFrame's (symbol, time) panel")]
    FactorSourcePanelMismatch,

    #[error(transparent)]
    Core(#[from] QWeaveError),

    #[error("Polars error: {0}")]
    Polars(#[from] PolarsError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}
