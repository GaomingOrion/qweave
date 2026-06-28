use std::collections::BTreeSet;

use polars::prelude::*;

use crate::error::Result;
use crate::factor::{FactorDescriptor, ParamValue, default_output_columns};
use crate::registry::{FactorRegistry, factor_registry};

pub fn factor_catalog() -> Result<DataFrame> {
    factor_catalog_for_registry(factor_registry()?)
}

fn factor_catalog_for_registry(registry: &FactorRegistry) -> Result<DataFrame> {
    let mut descriptors = registry.descriptors().collect::<Vec<_>>();
    descriptors.sort_by_key(|descriptor| descriptor.factor_name);

    let param_names = collect_param_names(&descriptors);
    let mut columns = vec![
        Column::new(
            "factor_name".into(),
            descriptors
                .iter()
                .map(|descriptor| descriptor.factor_name)
                .collect::<Vec<_>>(),
        ),
        Column::new(
            "kernel_name".into(),
            descriptors
                .iter()
                .map(|descriptor| descriptor.kernel_name)
                .collect::<Vec<_>>(),
        ),
        Column::new(
            "window".into(),
            descriptors
                .iter()
                .map(|descriptor| descriptor.window as u32)
                .collect::<Vec<_>>(),
        ),
        list_string_column(
            "input_names",
            descriptors
                .iter()
                .map(|descriptor| {
                    descriptor
                        .inputs
                        .iter()
                        .map(|input| input.name.to_string())
                        .collect()
                })
                .collect(),
        ),
        list_string_column(
            "input_dtypes",
            descriptors
                .iter()
                .map(|descriptor| {
                    descriptor
                        .inputs
                        .iter()
                        .map(|input| input.dtype.name().to_string())
                        .collect()
                })
                .collect(),
        ),
        list_string_column(
            "output_names",
            descriptors
                .iter()
                .map(|descriptor| {
                    descriptor
                        .outputs
                        .iter()
                        .map(|output| output.name.to_string())
                        .collect()
                })
                .collect(),
        ),
        list_string_column(
            "output_dtypes",
            descriptors
                .iter()
                .map(|descriptor| {
                    descriptor
                        .outputs
                        .iter()
                        .map(|output| output.dtype.name().to_string())
                        .collect()
                })
                .collect(),
        ),
        list_string_column(
            "output_columns",
            descriptors
                .iter()
                .map(|descriptor| default_output_columns(descriptor))
                .collect(),
        ),
        Column::new(
            "n_outputs".into(),
            descriptors
                .iter()
                .map(|descriptor| descriptor.outputs.len() as u32)
                .collect::<Vec<_>>(),
        ),
        Column::new(
            "has_params".into(),
            descriptors
                .iter()
                .map(|descriptor| !descriptor.params.is_empty())
                .collect::<Vec<_>>(),
        ),
        Column::new(
            "param_set".into(),
            descriptors
                .iter()
                .map(|descriptor| descriptor.param_set)
                .collect::<Vec<_>>(),
        ),
    ];

    for param_name in param_names {
        columns.push(Column::new(
            format!("param_{param_name}").into(),
            descriptors
                .iter()
                .map(|descriptor| param_value(descriptor, param_name))
                .collect::<Vec<_>>(),
        ));
    }

    Ok(DataFrame::new_infer_height(columns)?)
}

fn collect_param_names(descriptors: &[&FactorDescriptor]) -> Vec<&'static str> {
    descriptors
        .iter()
        .flat_map(|descriptor| descriptor.params.iter().map(|param| param.name))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn param_value(descriptor: &FactorDescriptor, name: &str) -> Option<f64> {
    descriptor
        .params
        .iter()
        .find(|param| param.name == name)
        .map(|param| match param.value {
            ParamValue::F64(value) => value,
        })
}

pub(crate) fn list_string_column(name: &str, rows: Vec<Vec<String>>) -> Column {
    if rows.is_empty() {
        return Column::new_empty(name.into(), &DataType::List(Box::new(DataType::String)));
    }

    let values = rows
        .into_iter()
        .map(|row| {
            if row.is_empty() {
                Series::new_empty("".into(), &DataType::String)
            } else {
                Series::new("".into(), row)
            }
        })
        .collect::<Vec<_>>();
    Series::new(name.into(), values).into()
}

#[cfg(test)]
mod tests {
    use std::ops::Range;

    use super::*;
    use crate::column_store::ColumnStore;
    use crate::factor::{ColumnSpec, DType, FactorResult, ParamSpec, ResolvedFactor};

    static INPUTS: [ColumnSpec; 2] = [
        ColumnSpec {
            name: "open",
            dtype: DType::F64,
        },
        ColumnSpec {
            name: "volume",
            dtype: DType::U32,
        },
    ];
    static OUTPUTS: [ColumnSpec; 1] = [ColumnSpec {
        name: "ret",
        dtype: DType::F64,
    }];
    static PARAMS: [ParamSpec; 1] = [ParamSpec {
        name: "k",
        value: ParamValue::F64(1.5),
    }];

    fn descriptor() -> FactorDescriptor {
        FactorDescriptor {
            factor_name: "ret_k15",
            kernel_name: "ret",
            window: 60,
            inputs: &INPUTS,
            outputs: &OUTPUTS,
            param_set: Some("k15"),
            params: &PARAMS,
            compute,
        }
    }

    fn compute(
        _columns: &ColumnStore<'_>,
        _ranges: &[Option<Range<usize>>],
        _factor: &ResolvedFactor<'_>,
    ) -> Result<FactorResult> {
        Ok(Vec::new())
    }

    #[test]
    fn catalog_contains_filterable_factor_metadata() -> Result<()> {
        let registry = FactorRegistry::from_descriptors(vec![descriptor()])?;
        let catalog = factor_catalog_for_registry(&registry)?;

        assert_eq!(catalog.height(), 1);
        assert_eq!(
            catalog
                .column("factor_name")?
                .try_str()
                .expect("factor_name is string")
                .get(0),
            Some("ret_k15")
        );
        assert_eq!(
            catalog
                .column("param_k")?
                .try_f64()
                .expect("param_k is f64")
                .get(0),
            Some(1.5)
        );
        assert_eq!(
            catalog
                .column("input_names")?
                .as_materialized_series()
                .get(0)?
                .to_string(),
            "[\"open\", \"volume\"]"
        );

        Ok(())
    }
}
