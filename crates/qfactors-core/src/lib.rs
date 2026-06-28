pub mod column_store;
pub mod compute_panel;
pub mod compute_sink;
pub mod error;
pub mod factor;
pub mod factor_catalog;
pub mod registry;

pub use column_store::ColumnStore;
pub use compute_panel::{ComputePanelOptions, compute_panel};
pub use compute_sink::{ComputeResult, ComputeSummary};
pub use error::{QFactorsError, Result};
pub use factor::{
    ColumnSpec, DType, FactorComputeFn, FactorDescriptor, FactorResult, ParamSpec, ParamValue,
    ResolvedFactor, default_output_columns,
};
pub use factor_catalog::factor_catalog;
pub use registry::{FactorRegistry, factor_registry};
