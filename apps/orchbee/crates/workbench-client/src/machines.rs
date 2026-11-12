//! Saved endpoint identity and catalog policy; enabled does not imply connected.
mod store;
use orchbee_local_runtime::SessionName;
use serde_json::{Value, json};
pub use store::Store;

pub(crate) const MAX_PROFILES: usize = 64;
const MAX_REVISION: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileId(String);

/// The machine a selection names: this host or one saved endpoint.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Choice {
    Local,
    Remote(ProfileId),
}
impl ProfileId {
    pub fn parse(value: &str) -> Option<Self> {
        (value.len() == 32
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            && value.bytes().any(|b| b != b'0'))
        .then(|| Self(value.to_owned()))
    }
    pub fn generate() -> Result<Self, Error> {
        let mut bytes = [0_u8; 16];
        getrandom::fill(&mut bytes).map_err(|_| Error::Storage)?;
        Self::parse(&bytes.iter().map(|b| format!("{b:02x}")).collect::<String>())
            .ok_or(Error::Storage)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// How Orchbee reaches a saved machine. The record names only the kind and its target; the
/// person's own configuration for that kind (for SSH, `~/.ssh/config`, the agent and
/// `known_hosts`) supplies everything else, so no credential, port, key path or remote platform is
/// ever stored here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Connection {
    /// An OpenSSH destination admitted by [`valid_target`].
    Ssh { target: String },
    /// A kind this build does not know, written by a later release. It is kept exactly as read and
    /// written back unchanged; the machine reads as unsupported and is never contacted.
    Unsupported(serde_json::Map<String, Value>),
}
impl Connection {
    pub fn ssh(target: &str) -> Option<Self> {
        valid_target(target).then(|| Self::Ssh {
            target: target.to_owned(),
        })
    }
    /// The OpenSSH destination, when this build knows how to reach the machine.
    pub fn ssh_target(&self) -> Option<&str> {
        match self {
            Self::Ssh { target } => Some(target),
            Self::Unsupported(_) => None,
        }
    }
    pub const fn supported(&self) -> bool {
        matches!(self, Self::Ssh { .. })
    }
    fn parse(value: &Value) -> Option<Self> {
        let object = value.as_object()?;
        match object.get("kind")?.as_str()? {
            "ssh" if keys(value, &["kind", "target"]) => Self::ssh(object.get("target")?.as_str()?),
            "ssh" => None,
            kind if valid_kind(kind) => Some(Self::Unsupported(object.clone())),
            _ => None,
        }
    }
    /// The persisted form: an unsupported kind is written back exactly as it was read.
    fn record(&self) -> Value {
        match self {
            Self::Ssh { target } => json!({"kind":"ssh","target":target}),
            Self::Unsupported(object) => Value::Object(object.clone()),
        }
    }
    /// The public form in `orchbee.machine`: a kind this build does not know is the shared
    /// `{ "kind": "unsupported" }` shape, never the unread content.
    pub fn json(&self) -> Value {
        match self {
            Self::Ssh { target } => json!({"kind":"ssh","target":target}),
            Self::Unsupported(_) => json!({"kind":"unsupported"}),
        }
    }
}
/// A connection kind's spelling: the handshake name grammar, so a later kind stays a plain word.
fn valid_kind(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// Whether Orchbee installed the remote copy and keeps it in step with this computer, or found
/// one someone else installed and leaves it alone.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteInstall {
    Managed,
    Manual,
}
impl RemoteInstall {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::Manual => "manual",
        }
    }
    fn parse(value: &str) -> Option<Self> {
        match value {
            "managed" => Some(Self::Managed),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Draft {
    pub label: String,
    pub connection: Connection,
    pub session: SessionName,
}
impl Draft {
    /// An SSH machine draft.
    pub fn parse(label: &str, target: &str, session: &str) -> Option<Self> {
        if !valid_label(label) {
            return None;
        }
        Some(Self {
            label: label.into(),
            connection: Connection::ssh(target)?,
            session: SessionName::parse(session).ok()?,
        })
    }
    pub fn ssh_target(&self) -> Option<&str> {
        self.connection.ssh_target()
    }
}
pub fn valid_label(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}
/// The label `machine add` saves when none is given: the host part of an admitted target, so
/// `luna@10.0.0.8` becomes `10.0.0.8` and `ssh://u@[2001:db8::1]:22` becomes `2001:db8::1`.
pub fn default_label(target: &str) -> String {
    let authority = target.strip_prefix("ssh://").unwrap_or(target);
    let host = authority
        .split_once('@')
        .map_or(authority, |(_, host)| host);
    let host = match host.strip_prefix('[') {
        Some(rest) => rest.split_once(']').map_or(rest, |(address, _)| address),
        None => host.split_once(':').map_or(host, |(host, _)| host),
    };
    host.to_owned()
}
/// Deliberately admits SSH destinations, never option fragments or passwords.
pub fn valid_target(value: &str) -> bool {
    if value.is_empty() || value.len() > 1024 || !value.is_ascii() {
        return false;
    }
    let (uri, authority) = match value.strip_prefix("ssh://") {
        Some(rest) => (true, rest),
        None => (false, value),
    };
    let host = if let Some((user, host)) = authority.split_once('@') {
        if user.is_empty()
            || user.starts_with('-')
            || !user
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.+\\-".contains(&b))
        {
            return false;
        }
        host
    } else {
        authority
    };
    let (host, port) = if let Some(rest) = host.strip_prefix('[') {
        let Some((address, suffix)) = rest.split_once(']') else {
            return false;
        };
        if address.parse::<std::net::Ipv6Addr>().is_err() {
            return false;
        }
        (address, suffix)
    } else {
        let (host, port) = host
            .find(':')
            .map_or((host, ""), |index| (&host[..index], &host[index..]));
        if host.is_empty()
            || host.starts_with('-')
            || !host
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return false;
        }
        (host, port)
    };
    if host.is_empty() {
        return false;
    }
    if port.is_empty() {
        return true;
    }
    uri && port.strip_prefix(':').is_some_and(|p| {
        !p.is_empty()
            && p.bytes().all(|b| b.is_ascii_digit())
            && p.parse::<u16>().is_ok_and(|n| n != 0)
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Profile {
    pub id: ProfileId,
    pub draft: Draft,
    pub enabled: bool,
    pub remote_install: RemoteInstall,
    /// Fields a later release added to this machine, kept unread and written back unchanged.
    pub extensions: serde_json::Map<String, Value>,
}
impl Profile {
    /// A newly added machine: enabled, with a remote copy someone else installed. Until Orchbee
    /// installs remotes itself, an add succeeds only against an existing installation.
    pub fn new(id: ProfileId, draft: Draft) -> Self {
        Self {
            id,
            draft,
            enabled: true,
            remote_install: RemoteInstall::Manual,
            extensions: serde_json::Map::new(),
        }
    }
    /// The machine object of `orchbee.machine`.
    pub fn json(&self) -> Value {
        json!({"id":self.id.as_str(),"label":self.draft.label,"connection":self.draft.connection.json(),
            "session":self.draft.session.as_str(),"enabled":self.enabled,
            "remoteInstall":self.remote_install.as_str()})
    }
    fn record(&self) -> Value {
        json!({"id":self.id.as_str(),"label":self.draft.label,"connection":self.draft.connection.record(),
            "session":self.draft.session.as_str(),"enabled":self.enabled,
            "remoteInstall":self.remote_install.as_str(),"extensions":self.extensions})
    }
    fn parse(value: &Value) -> Option<Self> {
        let mut value = value.clone();
        let extensions = match value.as_object_mut()?.remove("extensions") {
            None => serde_json::Map::new(),
            Some(Value::Object(extensions)) => extensions,
            Some(_) => return None,
        };
        if !keys(
            &value,
            &[
                "id",
                "label",
                "connection",
                "session",
                "enabled",
                "remoteInstall",
            ],
        ) {
            return None;
        }
        let label = value["label"].as_str().filter(|label| valid_label(label))?;
        Some(Self {
            id: ProfileId::parse(value["id"].as_str()?)?,
            draft: Draft {
                label: label.to_owned(),
                connection: Connection::parse(&value["connection"])?,
                session: SessionName::parse(value["session"].as_str()?).ok()?,
            },
            enabled: value["enabled"].as_bool()?,
            remote_install: RemoteInstall::parse(value["remoteInstall"].as_str()?)?,
            extensions,
        })
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Catalog {
    pub revision: u64,
    pub profiles: Vec<Profile>,
    /// Fields a later release added, kept unread and written back unchanged; two installed
    /// versions read this record at once.
    pub extensions: serde_json::Map<String, Value>,
}
#[derive(Clone, Debug)]
pub enum Edit {
    Add { id: ProfileId, draft: Draft },
    Rename { id: ProfileId, label: String },
    Remove(ProfileId),
    Enable { id: ProfileId, enabled: bool },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Configuration,
    InvalidCatalog,
    Storage,
    Busy,
    NotFound,
    /// A label several machines share; only the ID selects one of them.
    Ambiguous,
    Capacity,
    RevisionExhausted,
}
impl Catalog {
    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        let mut value: Value = serde_json::from_slice(bytes).ok()?;
        let extensions = match value.as_object_mut()?.remove("extensions") {
            None => serde_json::Map::new(),
            Some(Value::Object(extensions)) => extensions,
            Some(_) => return None,
        };
        if !keys(&value, &["version", "revision", "profiles"]) || value["version"] != 1 {
            return None;
        }
        let revision = value["revision"].as_u64().filter(|v| *v <= MAX_REVISION)?;
        let raw = value["profiles"]
            .as_array()
            .filter(|p| p.len() <= MAX_PROFILES)?;
        let mut profiles = Vec::<Profile>::new();
        for value in raw {
            let profile = Profile::parse(value)?;
            if profiles.iter().any(|p| p.id == profile.id) {
                return None;
            }
            profiles.push(profile);
        }
        Some(Self {
            revision,
            profiles,
            extensions,
        })
    }
    pub(crate) fn json(&self) -> Value {
        json!({"version":1,"revision":self.revision,
            "profiles":self.profiles.iter().map(Profile::record).collect::<Vec<_>>(),
            "extensions":self.extensions})
    }
    /// The machine an `<id|label>` operand names: the exact ID first, otherwise the one machine
    /// with that exact label. Several machines with that label are ambiguous; nothing falls back.
    pub fn resolve(&self, selector: &str) -> Result<&Profile, Error> {
        if let Some(profile) = self.profiles.iter().find(|p| p.id.as_str() == selector) {
            return Ok(profile);
        }
        let mut matches = self.profiles.iter().filter(|p| p.draft.label == selector);
        let first = matches.next().ok_or(Error::NotFound)?;
        if matches.next().is_some() {
            return Err(Error::Ambiguous);
        }
        Ok(first)
    }
    /// Pure candidate construction: any refusal leaves the original untouched.
    pub fn edit(&self, edit: &Edit) -> Result<(Self, bool), Error> {
        let mut next = self.clone();
        match edit {
            Edit::Add { id, draft } => {
                if self.profiles.len() == MAX_PROFILES {
                    return Err(Error::Capacity);
                }
                if self.profiles.iter().any(|p| p.id == *id) {
                    return Err(Error::InvalidCatalog);
                }
                next.profiles.push(Profile::new(id.clone(), draft.clone()));
            }
            Edit::Rename { id, label } => {
                if !valid_label(label) {
                    return Err(Error::InvalidCatalog);
                }
                next.profile_mut(id)?.draft.label.clone_from(label);
            }
            Edit::Remove(id) => {
                let index = next
                    .profiles
                    .iter()
                    .position(|p| p.id == *id)
                    .ok_or(Error::NotFound)?;
                next.profiles.remove(index);
            }
            Edit::Enable { id, enabled } => next.profile_mut(id)?.enabled = *enabled,
        }
        let changed = self.profiles != next.profiles;
        if changed {
            next.revision = self
                .revision
                .checked_add(1)
                .filter(|r| *r <= MAX_REVISION)
                .ok_or(Error::RevisionExhausted)?;
        }
        Ok((next, changed))
    }
    fn profile_mut(&mut self, id: &ProfileId) -> Result<&mut Profile, Error> {
        self.profiles
            .iter_mut()
            .find(|p| p.id == *id)
            .ok_or(Error::NotFound)
    }
}
fn keys(value: &Value, expected: &[&str]) -> bool {
    value
        .as_object()
        .is_some_and(|o| o.len() == expected.len() && expected.iter().all(|k| o.contains_key(*k)))
}
#[cfg(test)]
mod tests;
