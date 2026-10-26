//! Offline help-path grammar shared by recognition and help navigation. The catalog
//! declares only accepted paths; it does not inspect configuration or a running Session.

use super::{CommandPath, Family, HelpTopic};

/// One build-admitted help path and its typed presentation topic.
#[derive(Clone, Copy, Debug)]
pub(crate) struct HelpPath {
    words: &'static [&'static str],
    topic: HelpTopic,
}

impl HelpPath {
    const fn new(words: &'static [&'static str], topic: HelpTopic) -> Self {
        Self { words, topic }
    }

    /// Exact command words, without the executable or help flag.
    pub(crate) const fn words(self) -> &'static [&'static str] {
        self.words
    }

    /// The localized topic recognized for these exact words.
    pub(crate) const fn topic(self) -> HelpTopic {
        self.topic
    }
}

/// Every accepted help path in this build.
pub(crate) fn help_paths() -> &'static [HelpPath] {
    &HELP_PATHS
}

pub(super) fn topic(words: &[&str]) -> Option<HelpTopic> {
    HELP_PATHS
        .iter()
        .find(|path| path.words == words)
        .map(|path| path.topic)
}

/// A command's operands do not become help-path words. Families still require
/// an exact match so a misspelled verb cannot silently show its parent's help.
pub(super) fn invocation_topic(words: &[&str]) -> Option<HelpTopic> {
    HELP_PATHS
        .iter()
        .filter(|path| {
            words.starts_with(path.words)
                && (matches!(
                    path.topic,
                    HelpTopic::Command(_) | HelpTopic::Family(Family::Completion)
                ) || words == path.words)
        })
        .max_by_key(|path| path.words.len())
        .map(|path| path.topic)
}

pub(super) fn context_topic(words: &[&str]) -> Option<HelpTopic> {
    HELP_PATHS
        .iter()
        .filter(|path| words.starts_with(path.words))
        .max_by_key(|path| path.words.len())
        .map(|path| path.topic)
}

/// Directory presentation must not rename existing machine diagnostic contexts.
/// These two directory paths previously displayed their first command's help.
pub(super) const fn diagnostic_path(topic: HelpTopic) -> &'static str {
    match topic {
        HelpTopic::Family(Family::AgentView) => CommandPath::AgentViewGet.as_str(),
        HelpTopic::Family(Family::PanelGraphics) => CommandPath::PanelGraphicsSet.as_str(),
        HelpTopic::Family(family) => family.as_str(),
        HelpTopic::Command(command) => command.as_str(),
    }
}

static HELP_PATHS: std::sync::LazyLock<Vec<HelpPath>> = std::sync::LazyLock::new(|| {
    super::grammar::FAMILIES
        .iter()
        .map(|family| HelpPath::new(family.words, HelpTopic::Family(family.id)))
        .chain(
            super::grammar::COMMANDS
                .iter()
                .map(|command| HelpPath::new(command.words, HelpTopic::Command(command.id))),
        )
        .collect()
});
