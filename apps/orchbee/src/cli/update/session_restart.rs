//! The restart each running Session of this environment needs after an update, read by one
//! `query` per Session against the store's active release. Best effort: a Session that does not
//! answer in time, or a catalog that cannot be read, is left out and fails nothing.
use super::restart_tier::{self, Restart, Tier};
use crate::{cli_text::CliText, localization::Locale};
use orchbee_local_runtime::{ModeCheckedLocalSessionRendezvous as Rendezvous, SessionCatalog};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

/// How long the reading of every Session may take together.
const BUDGET: Duration = Duration::from_secs(5);

pub(super) struct SessionRestart {
    name: String,
    restart: Restart,
}

impl SessionRestart {
    pub(super) fn json(&self) -> Value {
        json!({"name": self.name, "restart": self.restart.json()})
    }

    /// One line for a Session that needs a restart, naming how to start it.
    pub(super) fn line(&self, locale: Locale) -> Option<String> {
        (self.restart.tier != Tier::None).then(|| {
            CliText::new(locale)
                .cli_update_session_restart(&self.name, self.restart.tier == Tier::Required)
        })
    }
}

/// Every running Session of this environment that answers within [`BUDGET`], with its restart.
pub(super) fn observe(wake: Option<orchbee_local_ipc::LocalIo<'_>>) -> Vec<SessionRestart> {
    let deadline = Instant::now() + BUDGET;
    let Ok((base, _)) = crate::local_activation::runtime_location() else {
        return Vec::new();
    };
    if crate::local_activation::runtime_base_unstarted().unwrap_or(true) {
        return Vec::new();
    }
    let Ok(root) = orchbee_local_runtime::DataRoot::resolve(|name| std::env::var_os(name)) else {
        return Vec::new();
    };
    let Ok(entries) = SessionCatalog::list(&base, &root, deadline) else {
        return Vec::new();
    };
    entries
        .into_iter()
        .filter(|entry| !entry.deleting && !entry.unreadable && !entry.conflicted)
        .filter_map(|entry| {
            let session = Rendezvous::from_runtime_directory(&entry.runtime).ok()?;
            let result =
                super::super::server_query::read(session.client_socket_path(), deadline, wake)
                    .ok()?;
            Some(SessionRestart {
                name: entry.name.as_str().to_owned(),
                restart: restart_tier::active(&result)?,
            })
        })
        .collect()
}
