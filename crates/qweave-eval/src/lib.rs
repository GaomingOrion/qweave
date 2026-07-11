pub mod context;
pub mod correlation;
pub mod error;
pub mod evaluate;
pub(crate) mod factor_source;
pub mod flows;
pub mod labels;
pub mod metrics;
pub(crate) mod panel;
pub mod stats;

pub use context::{Binning, Demean, EvalContext, EvalSpec, Weighting};
pub use correlation::factor_correlation;
pub use error::{EvalError, Result};
pub use evaluate::{EvalOutput, EvaluateOptions, TableData, evaluate, save_output};
pub use labels::{LabelOptions, LabelsOutput, with_labels};
