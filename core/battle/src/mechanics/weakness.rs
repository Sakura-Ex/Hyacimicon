//! Natural and modifier-contributed weakness sets; no implicit resistance change.
use crate::elements::Element;
use serde::Serialize;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct WeaknessState {
    pub natural: BTreeSet<Element>,
    pub attached: BTreeSet<Element>,
    pub all: BTreeSet<Element>,
}

impl WeaknessState {
    pub fn compose(natural: &[Element], attached: impl IntoIterator<Item = Element>) -> Self {
        let natural: BTreeSet<_> = natural.iter().copied().collect();
        let attached: BTreeSet<_> = attached.into_iter().collect();
        let all = natural.union(&attached).copied().collect();
        Self {
            natural,
            attached,
            all,
        }
    }
}

pub(crate) fn validate_set(
    elements: &[Element],
    name: &str,
    allow_empty: bool,
) -> Result<(), String> {
    if (!allow_empty && elements.is_empty()) || elements.len() > 7 {
        return Err(format!("invalid {name} element count"));
    }
    if elements.iter().copied().collect::<BTreeSet<_>>().len() != elements.len() {
        return Err(format!("duplicate {name} element"));
    }
    Ok(())
}
