//! What `workspace set-machine` says: where the Workspace now runs, what a dry run would do, and
//! why a change stopped. Every line names the Workspace and machines as the person knows them.
use super::{Locale, escape::unquoted};

/// A machine as `workspace set-machine` names it: this computer, or a saved machine's name.
#[derive(Clone, Copy)]
pub(crate) enum Place<'a> {
    Here,
    Saved(&'a str),
}

/// One line of `workspace set-machine`; `workspace` is the Workspace's name or the operand typed.
pub(crate) enum ChangeLine<'a> {
    /// The change finished: the Workspace runs on `to`, in `folder`, under the new identity `id`.
    Done {
        workspace: &'a str,
        to: Place<'a>,
        folder: &'a str,
        id: &'a str,
    },
    /// The first line of a dry run, above the terminals closing the Workspace would end.
    Preview {
        workspace: &'a str,
        from: Place<'a>,
        to: Place<'a>,
    },
    /// The Workspace already runs on the machine named.
    Same {
        workspace: &'a str,
        place: Place<'a>,
    },
    /// A saved machine the change needs is not connected.
    NotConnected { machine: &'a str },
    /// A Workspace in a linked worktree stays with its repository.
    Worktree,
    /// The Workspace's Project has an identity only its own Session knows.
    ProjectNotShareable { workspace: &'a str },
    /// The machine the Workspace goes to cannot take its Project's identity.
    TooOld {
        workspace: &'a str,
        place: Place<'a>,
    },
    /// Reading the Workspace where it runs failed; nothing changed.
    SourceFailed { workspace: &'a str },
    /// Sharing the Workspace's Project failed; nothing changed.
    ProjectFailed { workspace: &'a str },
    /// Creating the Workspace on the other machine failed; it still runs where it was.
    CreateFailed { workspace: &'a str },
    /// The new Workspace runs, and the old one could not be closed.
    CloseFailed { workspace: &'a str, id: &'a str },
}

impl super::CliText {
    pub(crate) fn cli_set_machine_help(self) -> super::HelpDetails {
        super::HelpDetails::new(
            "orchbee workspace set-machine <target> --to <machine> (--confirm | --dry-run) [--format text|json]",
            match self.locale {
                Locale::EnUs => {
                    "Move a workspace to another machine: it is created there with the same name and Project, then closed where it ran, which ends its terminals and the processes and Agents running in them. Files are not moved: the new workspace starts in the same folder relative to home when that folder exists there, otherwise in home, with one tab and one shell. --to names where it goes; the global --machine option, which never routes this command, names the machine that holds the workspace, this computer when omitted. Both must be connected or waiting for a login; for a login the move waits until someone logs in where Orchbee asks, and it connects no machine itself. A workspace in a linked worktree stays with its repository. --dry-run lists the terminals the move would end and changes nothing. The old workspace closes only once the new one runs, and a step that fails says what changed. '--format json' emits orchbee.workspace.set-machine."
                }
                Locale::ZhCn => {
                    "把工作区移到另一台机器：先在那台机器上以相同的名称和项目新建，再在原来的机器上关闭，这会结束其中的终端及其中运行的进程和智能体。文件不会被移动：那台机器上存在相对主目录的同一文件夹时，新工作区从该文件夹开始，否则从主目录开始，带一个标签页和一个 shell。--to 指定要移到的机器；全局选项 --machine 不会转发这条命令，而是指定工作区当前所在的机器，省略时为这台电脑。两台机器都须已连接或正在等待登录；需要登录时，移动会等到有人在 Orchbee 提示的地方登录后再继续，它本身不连接任何机器。链接工作树中的工作区跟着它的仓库，不能换机器。--dry-run 列出这次移动将结束的终端，不做任何更改。只有新工作区运行后才会关闭原来的工作区；某一步失败时会说明已改变了什么。'--format json' 输出 orchbee.workspace.set-machine。"
                }
            },
        )
    }

    pub(crate) fn cli_set_machine(self, line: &ChangeLine<'_>) -> String {
        let en = |place: Place<'_>| match place {
            Place::Here => "this computer".to_owned(),
            Place::Saved(label) => unquoted(label),
        };
        // A saved machine's name keeps a space on each side, which `tidy` drops before punctuation.
        let zh = |place: Place<'_>| match place {
            Place::Here => "这台电脑".to_owned(),
            Place::Saved(label) => format!(" {} ", unquoted(label)),
        };
        let text = match (self.locale, line) {
            (
                Locale::EnUs,
                ChangeLine::Done {
                    workspace,
                    to,
                    folder,
                    id,
                },
            ) => format!(
                "{} now runs on {}, in {folder}.\nNew workspace: {id}\n",
                unquoted(workspace),
                en(*to)
            ),
            (
                Locale::ZhCn,
                ChangeLine::Done {
                    workspace,
                    to,
                    folder,
                    id,
                },
            ) => format!(
                "{} 现在运行在{}，位于 {folder}。\n新工作区：{id}\n",
                unquoted(workspace),
                zh(*to)
            ),
            (
                Locale::EnUs,
                ChangeLine::Preview {
                    workspace,
                    from,
                    to,
                },
            ) => format!(
                "Dry run: with --confirm, {} moves from {} to {}, ending these terminals and the processes and Agents running in them. Files are not moved.\n",
                unquoted(workspace),
                en(*from),
                en(*to)
            ),
            (
                Locale::ZhCn,
                ChangeLine::Preview {
                    workspace,
                    from,
                    to,
                },
            ) => format!(
                "试运行：加上 --confirm 后，{} 会从{}移到{}，并结束以下终端及其中运行的进程和智能体。文件不会被移动。\n",
                unquoted(workspace),
                zh(*from),
                zh(*to)
            ),
            (Locale::EnUs, ChangeLine::Same { workspace, place }) => {
                format!("{} already runs on {}.\n", unquoted(workspace), en(*place))
            }
            (Locale::ZhCn, ChangeLine::Same { workspace, place }) => {
                format!("{} 已经运行在{}。\n", unquoted(workspace), zh(*place))
            }
            (Locale::EnUs, ChangeLine::NotConnected { machine }) => {
                format!("{} is not connected.\n", unquoted(machine))
            }
            (Locale::ZhCn, ChangeLine::NotConnected { machine }) => {
                format!("{} 没有连接。\n", unquoted(machine))
            }
            (Locale::EnUs, ChangeLine::Worktree) => {
                "A worktree stays with its repository.\n".to_owned()
            }
            (Locale::ZhCn, ChangeLine::Worktree) => "工作树跟着它的仓库，不能换机器。\n".to_owned(),
            (Locale::EnUs, ChangeLine::ProjectNotShareable { workspace }) => format!(
                "The Project of {} was made by an older Orchbee and can't be shared with another computer yet.\n",
                unquoted(workspace)
            ),
            (Locale::ZhCn, ChangeLine::ProjectNotShareable { workspace }) => format!(
                "{} 所在的项目是旧版 Orchbee 建的，暂时不能与另一台电脑共用。\n",
                unquoted(workspace)
            ),
            (Locale::EnUs, ChangeLine::TooOld { workspace, place }) => match place {
                Place::Here => format!(
                    "The Orchbee Session on this computer is too old to share the Project of {}. Restart it, then try again.\n",
                    unquoted(workspace)
                ),
                Place::Saved(label) => format!(
                    "Orchbee on {} is too old to share the Project of {}. Update it there, then try again.\n",
                    unquoted(label),
                    unquoted(workspace)
                ),
            },
            (Locale::ZhCn, ChangeLine::TooOld { workspace, place }) => match place {
                Place::Here => format!(
                    "这台电脑上的 Orchbee 会话版本太旧，无法共用 {} 所在的项目。请重启后再试。\n",
                    unquoted(workspace)
                ),
                Place::Saved(label) => format!(
                    "{} 上的 Orchbee 版本太旧，无法共用 {} 所在的项目。请在那边更新后再试。\n",
                    unquoted(label),
                    unquoted(workspace)
                ),
            },
            (Locale::EnUs, ChangeLine::SourceFailed { workspace }) => {
                format!("Could not read the workspace {}.\n", unquoted(workspace))
            }
            (Locale::ZhCn, ChangeLine::SourceFailed { workspace }) => {
                format!("无法读取工作区 {}。\n", unquoted(workspace))
            }
            (Locale::EnUs, ChangeLine::ProjectFailed { workspace }) => format!(
                "Could not share the Project of {}; nothing changed.\n",
                unquoted(workspace)
            ),
            (Locale::ZhCn, ChangeLine::ProjectFailed { workspace }) => format!(
                "无法共用 {} 所在的项目，没有做任何更改。\n",
                unquoted(workspace)
            ),
            (Locale::EnUs, ChangeLine::CreateFailed { workspace }) => format!(
                "{} still runs where it was; it was not moved.\n",
                unquoted(workspace)
            ),
            (Locale::ZhCn, ChangeLine::CreateFailed { workspace }) => {
                format!("{} 仍在原来的机器上运行，没有移动。\n", unquoted(workspace))
            }
            (Locale::EnUs, ChangeLine::CloseFailed { workspace, id }) => format!(
                "{} now also runs as workspace {id}, but the old one could not be closed.\n",
                unquoted(workspace)
            ),
            (Locale::ZhCn, ChangeLine::CloseFailed { workspace, id }) => format!(
                "{} 已新建为工作区 {id}，但原来的工作区没能关闭。\n",
                unquoted(workspace)
            ),
        };
        match self.locale {
            Locale::EnUs => text,
            Locale::ZhCn => tidy(&text),
        }
    }
}

/// Drops the space a saved machine's name brings before Chinese punctuation.
fn tidy(text: &str) -> String {
    text.replace(" 。", "。").replace(" ，", "，")
}
