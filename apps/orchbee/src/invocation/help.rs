//! Offline help-path grammar shared by recognition, help navigation, and usage advice. The
//! catalog declares only accepted paths; it does not inspect configuration or a running Session.

use super::{
    CommandPath, FailureEnvelope, Family, HelpTopic, Options, ParseFailure, Request, UsageContext,
    UsageReason, grammar, reject_extras, usage_with,
};

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

/// The one recognized-help-path table shared by `help <path>` and a trailing
/// `-h`/`--help`: families map to their overview, complete command paths to
/// their command help. Anything else is not a help path at all.
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

pub(super) fn resolve_help_path(
    rest: &[&str],
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    // A recognized topic anchors its own help path; an unrecognized one keeps
    // the literal `orchbee help` display so no hint points at a fake topic.
    let context = match topic(rest) {
        Some(topic) => UsageContext::Path(diagnostic_path(topic)),
        None => UsageContext::Literal("orchbee help"),
    };
    reject_extras(options, context, envelope)?;
    if rest.is_empty() {
        return Ok(Request::RootHelp);
    }
    topic(rest).map(Request::Help).ok_or_else(|| {
        usage_with(
            UsageReason::UnknownHelpTopic {
                path: rest.join(" "),
            },
            envelope,
        )
    })
}

/// Every option flag usage advice may suggest in one context: the root offers every command flag
/// plus the root modes and prefix flags, a help path offers its command's flags or every command
/// flag under its family, and a literal context offers none.
pub(crate) fn usage_option_candidates(context: UsageContext) -> Vec<&'static str> {
    let mut flags = Vec::new();
    match context {
        UsageContext::Root => {
            for path in help_paths() {
                if let HelpTopic::Command(command) = path.topic() {
                    flags.extend(grammar::help_flags(command));
                }
            }
            flags.extend(grammar::ROOT_MODES.iter().map(|flag| flag.spelling));
            flags.extend(grammar::PREFIX_FLAGS);
        }
        UsageContext::Path(path) => {
            let words = path.split_whitespace().collect::<Vec<_>>();
            let Some(entry) = help_paths()
                .iter()
                .find(|entry| entry.words() == words.as_slice())
            else {
                return flags;
            };
            match entry.topic() {
                HelpTopic::Command(command) => flags.extend(grammar::help_flags(command)),
                HelpTopic::Family(_) => {
                    for child in help_paths() {
                        if child.words().starts_with(&words)
                            && let HelpTopic::Command(command) = child.topic()
                        {
                            flags.extend(grammar::help_flags(command));
                        }
                    }
                }
            }
        }
        UsageContext::Literal(_) => return flags,
    }
    flags.sort_unstable();
    flags.dedup();
    flags
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
