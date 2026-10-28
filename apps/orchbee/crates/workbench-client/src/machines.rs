//! Saved endpoint identity and catalog policy; enabled does not imply connected.
mod store;
use orchbee_local_runtime::SessionName;
use serde_json::{Value, json};
pub use store::Store;

pub(crate) const MAX_PROFILES: usize = 64;
const MAX_REVISION: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProfileId(String);
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Draft {
    pub label: String,
    pub target: String,
    pub session: SessionName,
}
impl Draft {
    pub fn parse(label: &str, target: &str, session: &str) -> Option<Self> {
        if !valid_label(label) || !valid_target(target) {
            return None;
        }
        Some(Self {
            label: label.into(),
            target: target.into(),
            session: SessionName::parse(session).ok()?,
        })
    }
}
pub fn valid_label(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
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
}
impl Profile {
    pub fn json(&self) -> Value {
        json!({"id":self.id.as_str(),"label":self.draft.label,"target":self.draft.target,
            "session":self.draft.session.as_str(),"enabled":self.enabled})
    }
    fn parse(value: &Value) -> Option<Self> {
        if !keys(value, &["id", "label", "target", "session", "enabled"]) {
            return None;
        }
        Some(Self {
            id: ProfileId::parse(value["id"].as_str()?)?,
            draft: Draft::parse(
                value["label"].as_str()?,
                value["target"].as_str()?,
                value["session"].as_str()?,
            )?,
            enabled: value["enabled"].as_bool()?,
        })
    }
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Catalog {
    pub revision: u64,
    pub profiles: Vec<Profile>,
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
    Capacity,
    RevisionExhausted,
}
impl Catalog {
    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        let value: Value = serde_json::from_slice(bytes).ok()?;
        if !keys(&value, &["schemaVersion", "revision", "profiles"]) || value["schemaVersion"] != 1
        {
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
        Some(Self { revision, profiles })
    }
    pub(crate) fn json(&self) -> Value {
        json!({"schemaVersion":1,"revision":self.revision,"profiles":self.profiles.iter().map(Profile::json).collect::<Vec<_>>()})
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
                next.profiles.push(Profile {
                    id: id.clone(),
                    draft: draft.clone(),
                    enabled: true,
                });
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
