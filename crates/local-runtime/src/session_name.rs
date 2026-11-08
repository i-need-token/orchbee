//! Validated Session names. A name is also the folder name of the Session's records and runtime,
//! so it admits only names that every supported file system spells as one folder, and letter case
//! never distinguishes two Sessions: `Work` and `work` name one Session.
use std::io;

const MAXIMUM_BYTES: usize = 64;
const DEFAULT: &str = "default";
/// Windows device names, refused in any letter case with or without an extension.
const DEVICES: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// One Session name in the spelling it was given. Equality ignores ASCII letter case.
#[derive(Clone, Debug)]
pub struct SessionName(String);

impl SessionName {
    /// 1–64 ASCII letters, digits, `.`, `_` and `-`, neither starting nor ending with `.` and not
    /// a Windows device name. Any spelling of `default` is the default Session, spelled `default`.
    pub fn parse(name: &str) -> io::Result<Self> {
        if !valid(name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid Session name",
            ));
        }
        if name.eq_ignore_ascii_case(DEFAULT) {
            return Ok(Self::default_session());
        }
        Ok(Self(name.to_owned()))
    }

    /// The Session every environment starts when none is named.
    pub fn default_session() -> Self {
        Self(DEFAULT.to_owned())
    }

    pub fn is_default(&self) -> bool {
        self.0 == DEFAULT
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PartialEq for SessionName {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq_ignore_ascii_case(&other.0)
    }
}

impl Eq for SessionName {}

/// Whether `name` is a Session name, which is also a valid Session folder name.
/// Cross-language contract: search for `session-name-grammar` before changing this rule, then
/// update every occurrence together.
pub(crate) fn valid(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name);
    !name.is_empty()
        && name.len() <= MAXIMUM_BYTES
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && !name.starts_with('.')
        && !name.ends_with('.')
        && !DEVICES
            .iter()
            .any(|device| stem.eq_ignore_ascii_case(device))
}
