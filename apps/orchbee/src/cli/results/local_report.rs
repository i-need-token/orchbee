//! Local inspection reports that never reach a Session: `config check` rows and the
//! `server diagnose` document. Their command owners collect them; the text presenter
//! renders them.

use serde_json::{Map, Value};

/// One `config check` row: the stable check name, its status token, and machine details.
pub(crate) struct CheckReport {
    pub(crate) name: &'static str,
    pub(crate) status: &'static str,
    pub(crate) details: Map<String, Value>,
}

/// The collected diagnose product: the machine envelope plus the row table
/// and advisory tokens the text renderer shares with it.
pub(crate) struct DiagnoseDocument {
    pub(crate) envelope: Value,
    pub(crate) rows: Vec<(String, Map<String, Value>)>,
    pub(crate) advisories: Vec<&'static str>,
}
