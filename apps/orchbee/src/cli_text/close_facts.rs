//! What a Workspace's idle check found, as a close or a move names it: the terminals that may run
//! a program, by the names a person sees, at most three before a count of the rest.
use super::{Locale, escape::unquoted};

/// Names listed before the rest are only counted.
const NAMED: usize = 3;

/// Whether closing a Workspace would stop a program.
#[derive(Clone, Copy)]
pub(crate) enum Stops<'a> {
    /// No terminal runs a program.
    Idle,
    /// These terminals run a program, or could not be shown idle.
    Busy(&'a [String]),
    /// What runs could not be checked.
    Unread,
}

impl super::CliText {
    /// The line below a close `--dry-run`'s opening line, about the terminals listed after it.
    pub(crate) fn cli_close_facts(self, stops: Stops<'_>) -> String {
        match (self.locale, stops) {
            (Locale::EnUs, Stops::Idle) => "None of them is running a program.\n".to_owned(),
            (Locale::ZhCn, Stops::Idle) => "其中没有终端在运行程序。\n".to_owned(),
            (Locale::EnUs, Stops::Busy(names)) => format!(
                "{} of them may be running a program: {}.\n",
                names.len(),
                self.stop_names(names)
            ),
            (Locale::ZhCn, Stops::Busy(names)) => format!(
                "其中 {} 个可能正在运行程序：{}。\n",
                names.len(),
                self.stop_names(names)
            ),
            (Locale::EnUs, Stops::Unread) => {
                "Orchbee couldn't check whether programs are running in them.\n".to_owned()
            }
            (Locale::ZhCn, Stops::Unread) => "Orchbee 无法确认其中是否有程序在运行。\n".to_owned(),
        }
    }

    /// `names` joined for a sentence, the ones past the third only counted.
    pub(super) fn stop_names(self, names: &[String]) -> String {
        let shown: Vec<String> = names
            .iter()
            .take(NAMED)
            .map(|name| unquoted(name))
            .collect();
        let rest = names.len().saturating_sub(NAMED);
        match (self.locale, rest) {
            (Locale::EnUs, 0) => shown.join(", "),
            (Locale::ZhCn, 0) => shown.join("、"),
            (Locale::EnUs, rest) => format!("{} and {rest} more", shown.join(", ")),
            (Locale::ZhCn, rest) => format!("{} 以及另外 {rest} 个", shown.join("、")),
        }
    }
}
