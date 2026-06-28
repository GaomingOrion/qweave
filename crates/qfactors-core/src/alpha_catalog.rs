use std::collections::BTreeSet;

use polars::prelude::*;

use crate::alpha_registry::{AlphaRegistry, alpha_registry};
use crate::error::Result;
use crate::expr::{collect_fields, lookback_depth};
use crate::factor_catalog::list_string_column;

pub fn alpha_catalog() -> Result<DataFrame> {
    alpha_catalog_for_registry(alpha_registry()?)
}

fn alpha_catalog_for_registry(registry: &AlphaRegistry) -> Result<DataFrame> {
    let mut rows = registry
        .descriptors()
        .map(|descriptor| {
            let expr = (descriptor.build)();
            let mut fields = BTreeSet::new();
            collect_fields(&expr, &mut fields);
            AlphaCatalogRow {
                name: descriptor.name,
                expression: expr.to_string(),
                input_fields: fields.into_iter().collect(),
                lookback: lookback_depth(&expr),
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by_key(|row| row.name);

    Ok(DataFrame::new_infer_height(vec![
        Column::new(
            "alpha_name".into(),
            rows.iter().map(|row| row.name).collect::<Vec<_>>(),
        ),
        Column::new(
            "expression".into(),
            rows.iter()
                .map(|row| row.expression.as_str())
                .collect::<Vec<_>>(),
        ),
        list_string_column(
            "input_fields",
            rows.iter().map(|row| row.input_fields.clone()).collect(),
        ),
        Column::new(
            "n_inputs".into(),
            rows.iter()
                .map(|row| row.input_fields.len() as u32)
                .collect::<Vec<_>>(),
        ),
        Column::new(
            "lookback".into(),
            rows.iter()
                .map(|row| row.lookback as u32)
                .collect::<Vec<_>>(),
        ),
    ])?)
}

struct AlphaCatalogRow {
    name: &'static str,
    expression: String,
    input_fields: Vec<String>,
    lookback: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::alpha_registry::AlphaDescriptor;
    use crate::expr::Expr;

    fn alpha_b() -> Expr {
        Expr::Rank(Box::new(Expr::Covariance(
            Box::new(Expr::Rank(Box::new(Expr::Field("close".to_string())))),
            Box::new(Expr::Rank(Box::new(Expr::Field("volume".to_string())))),
            5,
        )))
    }

    fn alpha_a() -> Expr {
        Expr::GroupNeutralize(
            Box::new(Expr::Field("close".to_string())),
            Box::new(Expr::Field("industry".to_string())),
        )
    }

    fn descriptor_b() -> AlphaDescriptor {
        AlphaDescriptor {
            name: "beta_alpha",
            build: alpha_b,
        }
    }

    fn descriptor_a() -> AlphaDescriptor {
        AlphaDescriptor {
            name: "alpha_alpha",
            build: alpha_a,
        }
    }

    #[test]
    fn catalog_contains_filterable_alpha_metadata() -> Result<()> {
        let registry = AlphaRegistry::from_descriptors(vec![descriptor_b(), descriptor_a()])?;
        let catalog = alpha_catalog_for_registry(&registry)?;

        assert_eq!(catalog.height(), 2);
        assert_eq!(
            catalog
                .column("alpha_name")?
                .try_str()
                .expect("alpha_name is string")
                .iter()
                .collect::<Vec<_>>(),
            [Some("alpha_alpha"), Some("beta_alpha")]
        );
        assert_eq!(
            catalog
                .column("input_fields")?
                .as_materialized_series()
                .get(0)?
                .to_string(),
            "[\"close\", \"industry\"]"
        );
        assert_eq!(
            catalog
                .column("lookback")?
                .try_u32()
                .expect("lookback is u32")
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            [0, 4]
        );
        assert_eq!(
            catalog
                .column("expression")?
                .try_str()
                .expect("expression is string")
                .get(1),
            Some("rank(covariance(rank(field(close)), rank(field(volume)), 5))")
        );

        Ok(())
    }
}
