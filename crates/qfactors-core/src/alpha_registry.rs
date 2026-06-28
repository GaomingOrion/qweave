use std::collections::HashMap;
use std::sync::OnceLock;

use linkme::distributed_slice;

use crate::error::{QFactorsError, Result};
use crate::expr::Expr;

#[derive(Debug, Clone)]
pub struct AlphaDescriptor {
    pub name: &'static str,
    pub build: fn() -> Expr,
}

#[distributed_slice]
pub static ALPHA_DESCRIPTORS: [fn() -> AlphaDescriptor];

#[derive(Debug)]
pub struct AlphaRegistry {
    alphas: HashMap<&'static str, AlphaDescriptor>,
}

impl AlphaRegistry {
    pub fn from_descriptors(descriptors: Vec<AlphaDescriptor>) -> Result<Self> {
        let mut alphas = HashMap::with_capacity(descriptors.len());
        for descriptor in descriptors {
            let name = descriptor.name;
            if alphas.insert(name, descriptor).is_some() {
                return Err(QFactorsError::DuplicateFactorName(name.to_string()));
            }
        }
        Ok(Self { alphas })
    }

    pub fn get(&self, name: &str) -> Option<&AlphaDescriptor> {
        self.alphas.get(name)
    }

    pub fn descriptors(&self) -> impl Iterator<Item = &AlphaDescriptor> {
        self.alphas.values()
    }
}

static REGISTRY: OnceLock<AlphaRegistry> = OnceLock::new();

pub fn alpha_registry() -> Result<&'static AlphaRegistry> {
    if let Some(registry) = REGISTRY.get() {
        return Ok(registry);
    }

    let descriptors = ALPHA_DESCRIPTORS
        .iter()
        .map(|factory| factory())
        .collect::<Vec<_>>();
    let registry = AlphaRegistry::from_descriptors(descriptors)?;
    let _ = REGISTRY.set(registry);
    Ok(REGISTRY.get().expect("registry was set above"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build() -> Expr {
        Expr::Const(1.0)
    }

    fn descriptor() -> AlphaDescriptor {
        AlphaDescriptor {
            name: "alpha",
            build,
        }
    }

    #[test]
    fn registry_rejects_duplicate_alpha_names() {
        let err = AlphaRegistry::from_descriptors(vec![descriptor(), descriptor()]).unwrap_err();

        assert!(matches!(err, QFactorsError::DuplicateFactorName(name) if name == "alpha"));
    }
}
