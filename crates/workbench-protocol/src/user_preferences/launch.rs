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
impl UserLaunchProfiles {
    pub(super) fn validate(&self) -> Result<(), PreferencesFailure> {
        fn text(value: &str, maximum: usize) -> bool {
            value.len() <= maximum && !value.contains('\0')
        }
        fn id(value: &str) -> bool {
            (1..=128).contains(&value.len())
                && value.as_bytes()[0].is_ascii_alphanumeric()
                && value.bytes().all(|byte| {
                    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-')
                })
        }
        let mut ids = BTreeSet::new();
        if self.version != 1
            || !(1..=64).contains(&self.profiles.len())
            || !id(&self.default_profile_id)
        {
            return Err(PreferencesFailure::InvalidChanges);
        }
        for profile in &self.profiles {
            let name_valid = profile
                .name
                .as_ref()
                .map_or(profile.id == "system-default", |name| {
                    !name.is_empty() && name.trim() == name && text(name, 512)
                });
            if !id(&profile.id) || !ids.insert(&profile.id) || !name_valid {
                return Err(PreferencesFailure::InvalidChanges);
            }
            if let UserLaunchCommand::CustomExecutable {
                executable,
                arguments,
            } = &profile.command
                && (executable.is_empty()
                    || !text(executable, 32 * 1024)
                    || arguments.len() > 128
                    || arguments.iter().any(|argument| !text(argument, 32 * 1024)))
            {
                return Err(PreferencesFailure::InvalidChanges);
            }
        }
        if !ids.contains(&self.default_profile_id)
            || serde_json::to_vec(self).map_or(true, |bytes| bytes.len() > 1024 * 1024)
        {
            return Err(PreferencesFailure::InvalidChanges);
        }
        Ok(())
    }
}
