//! Launch-profile values retain the existing Desktop catalog grammar and bounds.
use super::PreferencesFailure;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The launch command contains literal arguments, never a command string to interpret.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, tag = "kind", rename_all = "kebab-case")]
pub enum UserLaunchCommand {
    SystemShell {
        login: bool,
    },
    CustomExecutable {
        executable: String,
        arguments: Vec<String>,
    },
}
/// A null name is the localized built-in label, allowed only for system-default.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UserLaunchProfile {
    pub id: String,
    #[serde(deserialize_with = "required_name")]
    pub name: Option<String>,
    pub command: UserLaunchCommand,
}
/// The first per-profile field that made a catalog invalid, in validation order.
///
/// Surfaces use this projection to name the exact value to correct. Catalog
/// structure that no single profile field owns (version, count, identifier, or
/// default identity) keeps the generic failure without a field projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserLaunchField {
    Name,
    Executable,
    /// Zero-based index of the literal argument to correct.
    Argument(usize),
}

enum ValidationFailure {
    Structure,
    Field(usize, UserLaunchField),
}
/// Collection ownership remains independent from the settings surface presenting its default.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct UserLaunchProfiles {
    pub version: u8,
    pub default_profile_id: String,
    pub profiles: Vec<UserLaunchProfile>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct WireProfiles {
    version: u8,
    default_profile_id: String,
    profiles: Vec<UserLaunchProfile>,
}
impl<'de> Deserialize<'de> for UserLaunchProfiles {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let wire = WireProfiles::deserialize(deserializer)?;
        let catalog = Self {
            version: wire.version,
            default_profile_id: wire.default_profile_id,
            profiles: wire.profiles,
        };
        catalog
            .validate()
            .map_err(|_| serde::de::Error::custom("invalid launch-profile catalog"))?;
        Ok(catalog)
    }
}
fn required_name<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Option::<String>::deserialize(deserializer)
}
impl Default for UserLaunchProfiles {
    fn default() -> Self {
        Self {
            version: 1,
            default_profile_id: "system-default".into(),
            profiles: vec![UserLaunchProfile {
                id: "system-default".into(),
                name: None,
                command: UserLaunchCommand::SystemShell { login: false },
            }],
        }
    }
}
fn bounded_text(value: &str, maximum: usize) -> bool {
    value.len() <= maximum && !value.contains('\0')
}

fn profile_id(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-'))
}

impl UserLaunchProfiles {
    /// The first profile field that fails validation, in `validate`'s per-profile order.
    ///
    /// Structural failures without a single owning field return `None`, and
    /// surfaces keep their generic invalid-value copy. This is an in-memory
    /// diagnostic; the serialized catalog and refusal vocabulary are unchanged.
    pub fn invalid_field(&self) -> Option<(usize, UserLaunchField)> {
        match self.validation() {
            Err(ValidationFailure::Field(index, field)) => Some((index, field)),
            // Structural catalog failures have no single profile field to focus.
            _ => None,
        }
    }

    fn profile_field(profile: &UserLaunchProfile) -> Option<UserLaunchField> {
        let name_valid = profile
            .name
            .as_ref()
            .map_or(profile.id == "system-default", |name| {
                !name.is_empty() && name.trim() == name && bounded_text(name, 512)
            });
        if !name_valid {
            return Some(UserLaunchField::Name);
        }
        if let UserLaunchCommand::CustomExecutable {
            executable,
            arguments,
        } = &profile.command
        {
            if executable.is_empty() || !bounded_text(executable, 32 * 1024) {
                return Some(UserLaunchField::Executable);
            }
            if let Some(index) = arguments
                .iter()
                .position(|argument| !bounded_text(argument, 32 * 1024))
            {
                return Some(UserLaunchField::Argument(index));
            }
        }
        None
    }

    pub(super) fn validate(&self) -> Result<(), PreferencesFailure> {
        self.validation()
            .map_err(|_| PreferencesFailure::InvalidChanges)
    }

    fn validation(&self) -> Result<(), ValidationFailure> {
        let mut ids = BTreeSet::new();
        if self.version != 1
            || !(1..=64).contains(&self.profiles.len())
            || !profile_id(&self.default_profile_id)
        {
            return Err(ValidationFailure::Structure);
        }
        // Establish all collection identities and bounds before naming a field.
        for profile in &self.profiles {
            if !profile_id(&profile.id)
                || !ids.insert(&profile.id)
                || matches!(&profile.command, UserLaunchCommand::CustomExecutable { arguments, .. } if arguments.len() > 128)
            {
                return Err(ValidationFailure::Structure);
            }
        }
        if !ids.contains(&self.default_profile_id)
            || serde_json::to_vec(self).map_or(true, |bytes| bytes.len() > 1024 * 1024)
        {
            return Err(ValidationFailure::Structure);
        }
        for (index, profile) in self.profiles.iter().enumerate() {
            if let Some(field) = Self::profile_field(profile) {
                return Err(ValidationFailure::Field(index, field));
            }
        }
        Ok(())
    }
}
