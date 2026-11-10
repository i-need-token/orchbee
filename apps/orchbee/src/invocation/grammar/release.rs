//! Command surfaces a release build withholds until their features are polished (ORC-398). The
//! commands stay compiled and development and test builds offer them; a release leaves them out of
//! help, completion and the Agent guide, and refuses an invocation that reaches one.

/// A feature whose command surface a release build withholds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Withheld {
    Plugin,
    Popup,
    Update,
    Channel,
}

impl Withheld {
    /// The stable machine token a refusal reports.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Plugin => "plugin",
            Self::Popup => "popup",
            Self::Update => "update",
            Self::Channel => "channel",
        }
    }
}

/// Whether this build withholds unreleased command surfaces: the release environment does. Unit
/// tests describe the development grammar on every host, as the TUI's tests compose every
/// capability.
pub(crate) fn withholds_unreleased() -> bool {
    !cfg!(test) && orchbee_local_runtime::is_release()
}

/// The withheld feature these leading command words reach, in a build that withholds it. `help`
/// before the words reaches the same feature.
pub(crate) fn withheld(words: &[&str]) -> Option<Withheld> {
    withholds_unreleased().then(|| reached(words)).flatten()
}

/// Whether discovery offers this command path in this build.
pub(crate) fn offered(words: &[&str]) -> bool {
    withheld(words).is_none()
}

fn reached(words: &[&str]) -> Option<Withheld> {
    let words = words.strip_prefix(&["help"]).unwrap_or(words);
    match words {
        ["plugin", ..] => Some(Withheld::Plugin),
        ["popup", ..] => Some(Withheld::Popup),
        ["update", ..] => Some(Withheld::Update),
        ["channel", ..] => Some(Withheld::Channel),
        _ => None,
    }
}
