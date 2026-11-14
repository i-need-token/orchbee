//! Local inspection reports that never reach a Session: `config check` rows, which `doctor`
//! also folds. Their command owners collect them; the text presenter renders them.

use serde_json::{Map, Value};

/// One `config check` row: the stable check name, its status token, and machine details.
pub(crate) struct CheckReport {
    pub(crate) name: &'static str,
    pub(crate) status: &'static str,
    pub(crate) details: Map<String, Value>,
}
