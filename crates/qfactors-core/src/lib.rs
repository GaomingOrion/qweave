pub mod alpha;
pub mod alpha_catalog;
pub(crate) mod alpha_dag;
pub mod alpha_eval;
pub mod alpha_registry;
pub mod cellset;
pub mod column_store;
pub mod compute_alpha;
pub mod compute_panel;
pub mod compute_sink;
pub mod error;
pub mod expr;
pub mod factor;
pub mod factor_catalog;
pub mod layout;
pub mod registry;

pub use alpha::A;
pub use alpha_catalog::alpha_catalog;
pub use alpha_registry::{AlphaDescriptor, AlphaRegistry, alpha_registry};
pub use column_store::ColumnStore;
pub use compute_alpha::compute_alphas;
pub use compute_panel::{ComputePanelOptions, compute_panel};
pub use compute_sink::{ComputeResult, ComputeSummary};
pub use error::{QFactorsError, Result};
pub use expr::Expr;
pub use factor::{
    ColumnSpec, DType, FactorComputeFn, FactorDescriptor, FactorResult, ParamSpec, ParamValue,
    ResolvedFactor, default_output_columns,
};
pub use factor_catalog::factor_catalog;
pub use registry::{FactorRegistry, factor_registry};
