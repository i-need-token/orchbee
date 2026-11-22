//! A Workspace's Change machine: the list's own words, the question a move asks before it stops a
//! program, and what the status line says while it runs and when it did not just land. Names
//! arrive sanitized; an empty machine name stands for this computer.

use super::Locale;
use crate::machine_change::Notice;

/// Names listed before the rest are only counted, as the command line counts them.
const NAMED: usize = 3;

impl Locale {
    /// This computer's row in a list of machines.
    pub(crate) const fn machine_change_here(self) -> &'static str {
        match self {
            Self::EnUs => "This computer",
            Self::ZhCn => "这台电脑",
        }
    }

    /// The query row of a long list.
    pub(crate) const fn machine_change_find(self) -> &'static str {
        match self {
            Self::EnUs => "Find a machine",
            Self::ZhCn => "查找机器",
        }
    }

    pub(crate) const fn machine_change_no_match(self) -> &'static str {
        match self {
            Self::EnUs => "No machine matches.",
            Self::ZhCn => "没有匹配的机器。",
        }
    }

    pub(crate) const fn machine_move_title(self) -> &'static str {
        match self {
            Self::EnUs => "Move Workspace?",
            Self::ZhCn => "移动工作区？",
        }
    }

    pub(crate) const fn machine_move_files(self) -> &'static str {
        match self {
            Self::EnUs => "Files are not moved.",
            Self::ZhCn => "文件不会被移动。",
        }
    }

    pub(crate) const fn machine_move_confirm(self) -> &'static str {
        match self {
            Self::EnUs => "Move",
            Self::ZhCn => "移动",
        }
    }

    /// The pending word while a move runs.
    pub(crate) const fn machine_moving(self) -> &'static str {
        match self {
            Self::EnUs => "Moving…",
            Self::ZhCn => "正在移动…",
        }
    }

    /// What moving the Workspace to `to` would stop: `busy` counts the terminals that may run a
    /// program and names those it can; `None` when what runs in them could not be read.
    pub(crate) fn machine_move_sentence(
        self,
        to: &str,
        busy: Option<(usize, &[String])>,
    ) -> String {
        let place = match (self, to.is_empty()) {
            (Self::EnUs, true) => "this computer".to_owned(),
            (Self::EnUs, false) => to.to_owned(),
            (Self::ZhCn, true) => "这台电脑".to_owned(),
            (Self::ZhCn, false) => format!(" {to} "),
        };
        match (self, busy) {
            (Self::EnUs, Some((count, []))) => format!(
                "Moving it to {place} closes {count} {} that may be running a program.",
                terminals(count)
            ),
            (Self::EnUs, Some((count, names))) => format!(
                "Moving it to {place} closes {count} {}: {}.",
                terminals(count),
                self.move_names(count, names)
            ),
            (Self::ZhCn, Some((count, []))) => {
                format!("移到{place}会关闭 {count} 个可能正在运行程序的终端。")
            }
            (Self::ZhCn, Some((count, names))) => format!(
                "移到{place}会关闭 {count} 个终端：{}。",
                self.move_names(count, names)
            ),
            (Self::EnUs, None) => format!(
                "Moving it to {place} closes its terminals; Orchbee couldn't check whether programs are running in them."
            ),
            (Self::ZhCn, None) => {
                format!("移到{place}会关闭其中的终端；Orchbee 无法确认其中是否有程序在运行。")
            }
        }
    }

    /// `names` for a sentence about `count` terminals: three named, the rest counted.
    fn move_names(self, count: usize, names: &[String]) -> String {
        let shown: Vec<&str> = names.iter().take(NAMED).map(String::as_str).collect();
        let rest = count.saturating_sub(shown.len());
        match (self, rest) {
            (Self::EnUs, 0) => shown.join(", "),
            (Self::ZhCn, 0) => shown.join("、"),
            (Self::EnUs, rest) => format!("{} and {rest} more", shown.join(", ")),
            (Self::ZhCn, rest) => format!("{} 以及另外 {rest} 个", shown.join("、")),
        }
    }

    /// A move's notice: its complete form, then shorter forms for a narrow status line.
    pub(crate) fn machine_move_notice(self, notice: &Notice) -> Vec<String> {
        match (self, notice) {
            (Self::EnUs, Notice::FolderMissing { machine, folder }) => vec![
                if machine.is_empty() {
                    format!("This computer has no {folder}; it started in the home folder.")
                } else {
                    format!("{machine} has no {folder}; it started in the home folder.")
                },
                "It started in the home folder.".to_owned(),
            ],
            (Self::ZhCn, Notice::FolderMissing { machine, folder }) => vec![
                if machine.is_empty() {
                    format!("这台电脑上没有 {folder}，已从主文件夹开始。")
                } else {
                    format!("{machine} 上没有 {folder}，已从主文件夹开始。")
                },
                "已从主文件夹开始。".to_owned(),
            ],
            (Self::EnUs, Notice::NotConnected { machine }) => vec![
                format!("{machine} is not connected."),
                "That machine is not connected.".to_owned(),
            ],
            (Self::ZhCn, Notice::NotConnected { machine }) => vec![
                format!("{machine} 没有连接。"),
                "那台机器没有连接。".to_owned(),
            ],
            (Self::EnUs, Notice::Worktree) => {
                vec!["A worktree stays with its repository.".to_owned()]
            }
            (Self::ZhCn, Notice::Worktree) => vec!["工作树跟着它的仓库，不能换机器。".to_owned()],
            (Self::EnUs, Notice::CloseFailed { machine }) => vec![
                if machine.is_empty() {
                    "It now runs on this computer too; the old one couldn't be closed.".to_owned()
                } else {
                    format!("It now runs on {machine} too; the old one couldn't be closed.")
                },
                "The old Workspace couldn't be closed.".to_owned(),
            ],
            (Self::ZhCn, Notice::CloseFailed { machine }) => vec![
                if machine.is_empty() {
                    "它现在也在这台电脑上运行，但原来的没能关闭。".to_owned()
                } else {
                    format!("它现在也在 {machine} 上运行，但原来的没能关闭。")
                },
                "原来的工作区没能关闭。".to_owned(),
            ],
            (Self::EnUs, Notice::Failed) => vec!["Couldn't move the Workspace.".to_owned()],
            (Self::ZhCn, Notice::Failed) => vec!["没能移动工作区。".to_owned()],
            (_, Notice::Unknown) => vec![self.outcome_unknown().to_owned()],
        }
    }
}

const fn terminals(count: usize) -> &'static str {
    if count == 1 { "terminal" } else { "terminals" }
}
