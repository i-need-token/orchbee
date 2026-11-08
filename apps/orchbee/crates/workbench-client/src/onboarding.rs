//! Native-profile education state. Only an explicit dismissal writes storage.
use crate::client_view::{ClientAction, PrefixAction};
use orchbee_local_runtime::{DataRoot, PrivateCache, StateRecord};
use orchbee_local_runtime::{RecordNotice, VersionedRecord};
use serde::{Deserialize, Serialize};
use std::{io, path::PathBuf};
const RECORD: StateRecord = StateRecord::Onboarding;
const FILE: &str = RECORD.file_name();
const MAX_BYTES: usize = 1024;
#[cfg(test)]
const DISMISSED: &[u8] = b"{\"version\":1,\"dismissed\":true}\n";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Workspace,
    Agent,
    Menu,
    Help,
    Dismiss,
}
impl From<Action> for crate::localization::OnboardingLabel {
    fn from(action: Action) -> Self {
        match action {
            Action::Workspace => Self::Workspace,
            Action::Agent => Self::Agent,
            Action::Menu => Self::Menu,
            Action::Help => Self::Help,
            Action::Dismiss => Self::Dismiss,
        }
    }
}

impl Action {
    pub(crate) const ALL: [Self; 5] = [
        Self::Workspace,
        Self::Agent,
        Self::Menu,
        Self::Help,
        Self::Dismiss,
    ];
    pub(crate) fn client(self) -> ClientAction {
        match self {
            Self::Workspace => ClientAction::RequestWorkspaceCreate,
            Self::Agent => ClientAction::ToggleAgentBoard,
            Self::Menu => ClientAction::OpenActionMenu,
            Self::Help => ClientAction::OpenShortcutHelp,
            Self::Dismiss => ClientAction::DismissOnboarding,
        }
    }
    pub(crate) fn binding(self) -> PrefixAction {
        match self {
            Self::Agent => PrefixAction::BoardToggle,
            Self::Help => PrefixAction::ShowHelp,
            _ => PrefixAction::Client(self.client()),
        }
    }
    pub(crate) fn needs_session(self) -> bool {
        matches!(self, Self::Workspace | Self::Agent | Self::Menu)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct State {
    pub(crate) visible: bool,
    pub(crate) read_failed: bool,
    root: Option<PathBuf>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Document {
    version: u8,
    /// Fields a later release added, kept unread and written back unchanged; two installed
    /// versions read this record at once.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    extensions: serde_json::Map<String, serde_json::Value>,
    dismissed: bool,
}
/// The stored record: absent, current, or one whose version this build cannot use, which works as
/// no record and is never rewritten, so a dismissal then lasts only until Orchbee closes.
enum Stored {
    Absent,
    Current(Document),
    Unusable,
}
fn read(bytes: Option<Vec<u8>>, directory: Option<&PathBuf>) -> io::Result<Stored> {
    let Some(bytes) = bytes else {
        return Ok(Stored::Absent);
    };
    if bytes.len() > MAX_BYTES {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let opening =
        orchbee_local_runtime::open_record(VersionedRecord::Onboarding, &bytes, |bytes| {
            strict(bytes).is_some()
        });
    if let Some((problem, version)) = opening.problem() {
        if let Some(directory) = directory {
            orchbee_local_runtime::report_record_notice(&RecordNotice {
                record: VersionedRecord::Onboarding,
                problem,
                version,
                path: directory.join(FILE),
            });
        }
        return Ok(Stored::Unusable);
    }
    let bytes = match opening {
        orchbee_local_runtime::RecordOpening::Migrated { bytes, .. } => bytes,
        _ => bytes,
    };
    strict(&bytes)
        .map(Stored::Current)
        .ok_or_else(|| io::ErrorKind::InvalidData.into())
}
fn strict(bytes: &[u8]) -> Option<Document> {
    serde_json::from_slice::<Document>(bytes)
        .ok()
        .filter(|document| document.version == 1)
}
#[cfg(test)]
fn decode(bytes: Option<Vec<u8>>) -> io::Result<bool> {
    Ok(matches!(read(bytes, None)?, Stored::Current(document) if document.dismissed))
}
impl State {
    pub fn load() -> Self {
        let root = DataRoot::resolve(|name| std::env::var_os(name))
            .ok()
            .map(|root| root.state());
        let result = root
            .as_ref()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
            .and_then(|directory| {
                read(
                    PrivateCache::read_existing(directory, FILE, MAX_BYTES)?,
                    Some(directory),
                )
            })
            .map(|stored| matches!(stored, Stored::Current(document) if document.dismissed));
        Self {
            visible: !matches!(result, Ok(true)),
            read_failed: result.is_err(),
            root,
        }
    }
    pub(crate) fn dismiss(&mut self) -> io::Result<()> {
        if !self.visible {
            return Ok(());
        }
        // Hide locally even if storage refuses. The notice explains that a restart may show it again.
        self.visible = false;
        let root = self.root.as_ref().ok_or(io::ErrorKind::NotFound)?;
        let mut cache = PrivateCache::acquire(root, RECORD.name())?;
        let mut document = match read(cache.read(FILE, MAX_BYTES)?, Some(root))? {
            Stored::Unusable => return Ok(()),
            Stored::Current(document) if document.dismissed => return Ok(()),
            Stored::Current(document) => document,
            Stored::Absent => Document {
                version: 1,
                extensions: serde_json::Map::new(),
                dismissed: false,
            },
        };
        document.dismissed = true;
        let mut bytes = serde_json::to_vec(&document).map_err(io::Error::other)?;
        bytes.push(b'\n');
        cache.replace(FILE, &bytes)?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_and_versioned_state_are_distinct_from_malformed_data() {
        assert!(!decode(None).unwrap());
        assert!(decode(Some(DISMISSED.to_vec())).unwrap());
        for bytes in [
            b"{}".as_slice(),
            b"{\"version\":1,\"dismissed\":true,\"extra\":0}",
            b"broken",
        ] {
            assert!(decode(Some(bytes.to_vec())).is_err());
        }
        assert!(decode(Some(vec![b' '; MAX_BYTES + 1])).is_err());
    }
    #[test]
    fn failed_save_hides_only_the_local_card_and_reports_failure() {
        let mut state = State {
            visible: true,
            ..State::default()
        };
        assert!(state.dismiss().is_err());
        assert!(!state.visible);
        assert!(state.dismiss().is_ok());
    }
    #[test]
    fn entry_actions_preserve_existing_owners() {
        assert_eq!(Action::Agent.client(), ClientAction::ToggleAgentBoard);
        assert_eq!(
            Action::Workspace.client(),
            ClientAction::RequestWorkspaceCreate
        );
        assert_eq!(Action::Help.binding(), PrefixAction::ShowHelp);
    }
}

#[cfg(all(test, windows))]
mod storage_tests {
    use super::*;
    struct Root(PathBuf);
    impl Drop for Root {
        fn drop(&mut self) {
            if !self.0.exists() {
                return;
            }
            let path = self.0.canonicalize().unwrap();
            assert_eq!(
                path.parent(),
                Some(std::env::temp_dir().canonicalize().unwrap().as_path())
            );
            assert!(
                path.file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .starts_with("orchbee-onboarding-test-")
            );
            std::fs::remove_dir_all(path).unwrap();
        }
    }
    #[test]
    fn dismissal_survives_sessions_and_corruption_is_never_overwritten() {
        // This fixture owns its private root and releases it after the last cache handle drops.
        let id = crate::machines::ProfileId::generate().unwrap();
        let root =
            Root(std::env::temp_dir().join(format!("orchbee-onboarding-test-{}", id.as_str())));
        let directory = root.0.join("profile");
        let read = || decode(PrivateCache::read_existing(&directory, FILE, MAX_BYTES).unwrap());
        assert!(!read().unwrap());
        assert!(!root.0.exists());
        let mut state = State {
            visible: true,
            read_failed: false,
            root: Some(directory.clone()),
        };
        state.dismiss().unwrap();
        assert!(read().unwrap());
        let mut concurrent = State {
            visible: true,
            read_failed: false,
            root: Some(directory.clone()),
        };
        concurrent.dismiss().unwrap();
        let mut cache = PrivateCache::acquire(&directory, RECORD.name()).unwrap();
        cache.replace(FILE, b"broken").unwrap();
        drop(cache);
        let mut corrupted = State {
            visible: true,
            read_failed: true,
            root: Some(directory.clone()),
        };
        assert!(corrupted.dismiss().is_err());
        assert_eq!(std::fs::read(directory.join(FILE)).unwrap(), b"broken");
        // A newer record reads as no dismissal, and dismissing it again hides the notice only
        // locally: the record stays byte-identical so the newer version keeps it.
        let newer = b"{\"version\":2,\"dismissed\":true,\"later\":[1]}";
        let mut cache = PrivateCache::acquire(&directory, RECORD.name()).unwrap();
        cache.replace(FILE, newer).unwrap();
        drop(cache);
        assert!(!read().unwrap());
        let mut kept = State {
            visible: true,
            read_failed: false,
            root: Some(directory.clone()),
        };
        kept.dismiss().unwrap();
        assert!(!kept.visible);
        assert_eq!(std::fs::read(directory.join(FILE)).unwrap(), newer);
    }
}
