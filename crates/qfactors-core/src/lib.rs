pub mod column_store;
pub mod compute_panel;
pub mod compute_sink;
pub mod error;
pub mod factor;
pub mod factor_catalog;
pub mod group;
pub mod obs_range_cache;
pub mod prepared_panel;
pub mod registry;

pub use column_store::ColumnStore;
pub use compute_panel::compute_panel;
pub use compute_sink::{ComputeResult, ComputeSummary};
pub use error::{QFactorsError, Result};
pub use factor::{
    ColumnSpec, DType, FactorComputeFn, FactorDescriptor, FactorResult, ParamSpec, ParamValue,
    ResolvedFactor, default_output_columns,
};
pub use factor_catalog::factor_catalog;
pub use group::GroupInfo;
pub use obs_range_cache::ObsRangeCache;
pub use prepared_panel::{
    GROUP_ID_COL, NullPolicy, PreparePanelOptions, PreparedObservation, PreparedPanel, TIME_ORD_COL,
};
pub use registry::{FactorRegistry, factor_registry};
