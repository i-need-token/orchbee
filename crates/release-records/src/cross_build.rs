//! The names one build hands a program of another release: hidden entries and switches on its
//! command line, and variables in its environment. Releases 0.1.0-beta.1 to 0.1.0-beta.24 read and
//! write each in an earlier spelling with a `-v1` or `_V1` suffix, 0.1.0-beta.25 only in the
//! current spelling, and every later build reads both, so a sender picks the spelling by the
//! release that receives the name. Each owner keeps its own names; this module owns only the rule.

#[cfg(test)]
mod tests;

/// A name one build hands a program of another release, in its current spelling and in the
/// earlier one that releases 0.1.0-beta.1 to 0.1.0-beta.24 read and write. A receiver accepts both
/// spellings for good; help, completion and the Agent guide name neither.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CrossBuildName {
    /// The spelling from 0.1.0-beta.25 on.
    pub current: &'static str,
    /// The spelling up to 0.1.0-beta.24.
    pub earlier: &'static str,
}

impl CrossBuildName {
    /// Whether `text` is either spelling.
    pub fn is<T: PartialEq<str> + ?Sized>(self, text: &T) -> bool {
        *text == *self.current || *text == *self.earlier
    }

    /// The spelling a program of the release `version` reads: the earlier one for 0.1.0-beta.1 to
    /// 0.1.0-beta.24, and the current one for every other version.
    pub fn for_release(self, version: &str) -> &'static str {
        if reads_earlier_spelling(version) {
            self.earlier
        } else {
            self.current
        }
    }
}

/// Whether the release `version` reads only earlier spellings: exactly `0.1.0-beta.1` to
/// `0.1.0-beta.24`, a closed set no later release joins.
fn reads_earlier_spelling(version: &str) -> bool {
    version
        .strip_prefix("0.1.0-beta.")
        .filter(|number| {
            !number.starts_with('0') && number.bytes().all(|byte| byte.is_ascii_digit())
        })
        .and_then(|number| number.parse::<u8>().ok())
        .is_some_and(|number| (1..=24).contains(&number))
}
