//! Static shell interpreters project the platform-independent invocation descriptors.
mod elvish;
mod fish;
mod grammar;
mod metadata;
mod posix;
mod powershell;
use crate::invocation::{CompletionShell, grammar as authority};
use orchbee_agent_observation_protocol::AgentReportProvider;
use std::sync::LazyLock;
static GLOBAL_FLAGS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    authority::OPTIONS
        .iter()
        .filter(|option| option.role == authority::OptionRole::Global)
        .map(|option| option.spelling)
        .collect()
});
static HELP_TOPICS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    let mut words = authority::FAMILIES
        .iter()
        .map(|family| family.words)
        .chain(offered_commands().map(|command| command.words))
        .filter(|words| words.len() == 1 && authority::offered(words))
        .map(|words| words[0])
        .collect::<Vec<_>>();
    words.sort_unstable();
    words.dedup();
    words
});
static FAMILY_VERBS: LazyLock<Vec<(&'static str, &'static [&'static str])>> = LazyLock::new(|| {
    HELP_TOPICS
        .iter()
        .copied()
        .filter(|path| !authority::children(path).is_empty())
        .map(|path| (path, authority::children(path)))
        .collect()
});
static HELP_CHILDREN: LazyLock<Vec<(&'static str, &'static [&'static str])>> =
    LazyLock::new(|| FAMILY_VERBS.to_vec());
static HELP_LEVEL3: LazyLock<Vec<(&'static str, &'static [&'static str])>> = LazyLock::new(|| {
    authority::FAMILIES
        .iter()
        .filter(|family| family.words.len() == 2 && authority::offered(family.words))
        .map(|family| (family.key, authority::children(family.key)))
        .collect()
});
static REPEATABLE_FLAGS: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    authority::OPTIONS
        .iter()
        .filter(|option| option.repeatable)
        .map(|option| option.spelling)
        .collect()
});
/// The commands this build offers; a release withholds some, which completion never names.
fn offered_commands() -> impl Iterator<Item = &'static authority::CommandSpec> {
    authority::COMMANDS
        .iter()
        .filter(|command| authority::offered(command.words))
}
fn agent_target_values() -> Vec<String> {
    AgentReportProvider::ALL
        .into_iter()
        .map(|provider| format!("@agent:{}", provider.native_provider_id()))
        .collect()
}

pub(crate) fn render(shell: CompletionShell) -> String {
    match shell {
        CompletionShell::Bash | CompletionShell::Zsh => posix::render(shell),
        CompletionShell::Fish => fish::render(),
        CompletionShell::PowerShell => powershell::render(&command_name()),
        CompletionShell::Elvish => elvish::render(&command_name()),
    }
}

/// The name a shell invokes this executable by: the running file's own stem, so a development or
/// test Client such as `ob-dev-<digest>.exe` completes under its name rather than release's. A
/// stem outside the portable command spelling falls back to the cohort Client's.
fn command_name() -> String {
    let portable = |name: &str| {
        !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    };
    std::env::current_exe()
        .ok()
        .and_then(|path| path.file_stem()?.to_str().map(str::to_owned))
        .filter(|name| portable(name))
        .unwrap_or_else(|| {
            let client = orchbee_local_runtime::CohortArtifact::Client.file_name();
            client.strip_suffix(".exe").unwrap_or(&client).to_owned()
        })
}
#[cfg(test)]
mod tests;
