//! Per-provider invocation mode grammar and its classifier. Every grammar is data;
//! anything the grammar does not list is unclassified, never interactive.

use crate::AgentReportProvider;

/// Whether an invocation's arguments start the provider's interactive interface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AgentInvocationMode {
    Interactive,
    /// Version, help, print, exec, login, update, server, and similar forms.
    Noninteractive,
    /// The grammar does not cover the arguments.
    Unclassified,
}

/// One provider's documented argument grammar. `--help`, `-h`, and `--version` are
/// noninteractive for every provider. Options match before `=`; `--` ends options.
#[derive(Debug)]
pub struct AgentModeGrammar {
    /// Options that select a noninteractive form wherever they appear.
    pub noninteractive_options: &'static [&'static str],
    /// First positional words that select a noninteractive subcommand.
    pub noninteractive_commands: &'static [&'static str],
    /// First positional words that select an interactive subcommand.
    pub interactive_commands: &'static [&'static str],
    /// Whether only an interactive subcommand starts the interactive interface; without one,
    /// a bare launch and any other invocation stay unclassified.
    pub requires_command: bool,
    /// Interactive options without a value.
    pub interactive_flags: &'static [&'static str],
    /// Interactive options written bare or as `--name=value`; they never consume the next argument.
    pub interactive_optional_options: &'static [&'static str],
    /// Interactive options that consume the next argument unless written with `=`.
    pub interactive_options: &'static [&'static str],
    /// The mode of other positional arguments, such as an initial prompt or a project path.
    pub positional: AgentInvocationMode,
}

const COMMON_NONINTERACTIVE: [&str; 3] = ["--help", "-h", "--version"];

impl AgentReportProvider {
    /// Classifies the arguments after the program name with this provider's grammar.
    pub fn classify_invocation(self, args: &[String]) -> AgentInvocationMode {
        self.registry_entry().grammar.classify(args)
    }
}

impl AgentModeGrammar {
    pub fn classify(&self, args: &[String]) -> AgentInvocationMode {
        let mut mode = AgentInvocationMode::Interactive;
        let mut first_positional = true;
        let mut command = false;
        let mut index = 0;
        while let Some(arg) = args.get(index) {
            index += 1;
            if arg == "--" {
                if index < args.len() {
                    mode = worse(mode, self.positional);
                }
                break;
            }
            if arg.len() > 1 && arg.starts_with('-') {
                let (name, inline_value) = match arg.split_once('=') {
                    Some((name, _)) => (name, true),
                    None => (arg.as_str(), false),
                };
                if COMMON_NONINTERACTIVE.contains(&name)
                    || self.noninteractive_options.contains(&name)
                {
                    return AgentInvocationMode::Noninteractive;
                }
                if self.interactive_optional_options.contains(&name) {
                    continue;
                }
                if self.interactive_flags.contains(&name) && !inline_value {
                    continue;
                }
                if self.interactive_options.contains(&name) {
                    if !inline_value {
                        if index >= args.len() {
                            mode = AgentInvocationMode::Unclassified;
                        }
                        index += 1;
                    }
                    continue;
                }
                mode = AgentInvocationMode::Unclassified;
                continue;
            }
            if std::mem::take(&mut first_positional) {
                if self.noninteractive_commands.contains(&arg.as_str()) {
                    return AgentInvocationMode::Noninteractive;
                }
                if self.interactive_commands.contains(&arg.as_str()) {
                    command = true;
                    continue;
                }
            }
            mode = worse(mode, self.positional);
        }
        if self.requires_command && !command {
            return AgentInvocationMode::Unclassified;
        }
        mode
    }
}

fn worse(current: AgentInvocationMode, next: AgentInvocationMode) -> AgentInvocationMode {
    use AgentInvocationMode as M;
    match (current, next) {
        (M::Unclassified, _) | (_, M::Unclassified) => M::Unclassified,
        (M::Noninteractive, _) | (_, M::Noninteractive) => M::Noninteractive,
        (M::Interactive, M::Interactive) => M::Interactive,
    }
}

/// Grammar data from each provider's documented command-line reference. Entries are best
/// effort until a real corpus records the verified version range.
pub(super) mod grammar {
    use super::{AgentInvocationMode as M, AgentModeGrammar};

    const fn base(positional: M) -> AgentModeGrammar {
        AgentModeGrammar {
            noninteractive_options: &[],
            noninteractive_commands: &[],
            interactive_commands: &[],
            requires_command: false,
            interactive_flags: &[],
            interactive_optional_options: &[],
            interactive_options: &[],
            positional,
        }
    }

    /// Providers without recorded grammar: only a bare launch is interactive.
    pub const DEFAULT: AgentModeGrammar = AgentModeGrammar {
        noninteractive_options: &["-v", "-V"],
        ..base(M::Unclassified)
    };

    pub const CLAUDE: AgentModeGrammar = AgentModeGrammar {
        noninteractive_options: &["-p", "--print", "-v", "--output-format", "--input-format"],
        noninteractive_commands: &[
            "config",
            "mcp",
            "migrate-installer",
            "setup-token",
            "doctor",
            "update",
            "upgrade",
            "install",
            "plugin",
        ],
        interactive_flags: &[
            "-c",
            "--continue",
            "--dangerously-skip-permissions",
            "--verbose",
            "-d",
            "--debug",
            "--ide",
            "--fork-session",
        ],
        interactive_optional_options: &["-r", "--resume"],
        interactive_options: &[
            "--model",
            "--fallback-model",
            "--permission-mode",
            "--add-dir",
            "--allowedTools",
            "--allowed-tools",
            "--disallowedTools",
            "--disallowed-tools",
            "--settings",
            "--setting-sources",
            "--mcp-config",
            "--system-prompt",
            "--append-system-prompt",
            "--session-id",
            "--agent",
            "--agents",
            "--plugin-dir",
        ],
        ..base(M::Interactive)
    };

    /// `-p` selects a profile here, unlike the print option of other providers.
    pub const CODEX: AgentModeGrammar = AgentModeGrammar {
        noninteractive_options: &["-V"],
        noninteractive_commands: &[
            "exec",
            "e",
            "login",
            "logout",
            "mcp",
            "mcp-server",
            "app-server",
            "completion",
            "apply",
            "a",
            "sandbox",
            "debug",
            "help",
        ],
        interactive_commands: &["resume", "fork"],
        interactive_flags: &[
            "--oss",
            "--full-auto",
            "--dangerously-bypass-approvals-and-sandbox",
            "--yolo",
            "--search",
            "--last",
        ],
        interactive_options: &[
            "-m",
            "--model",
            "-c",
            "--config",
            "-p",
            "--profile",
            "-s",
            "--sandbox",
            "-a",
            "--ask-for-approval",
            "-C",
            "--cd",
            "-i",
            "--image",
            "--add-dir",
            "--local-provider",
            "--enable",
            "--disable",
        ],
        ..base(M::Interactive)
    };

    /// A positional query runs one-shot; `-i` starts the interactive interface with a prompt.
    pub const GEMINI: AgentModeGrammar = AgentModeGrammar {
        noninteractive_options: &["-v", "-p", "--prompt", "-l", "--list-extensions"],
        noninteractive_commands: &["mcp", "extensions"],
        interactive_flags: &["-y", "--yolo", "-d", "--debug", "-s", "--sandbox"],
        interactive_options: &[
            "-m",
            "--model",
            "-i",
            "--prompt-interactive",
            "--approval-mode",
            "--include-directories",
            "-e",
            "--extensions",
            "-r",
            "--resume",
        ],
        ..base(M::Noninteractive)
    };

    pub const CURSOR: AgentModeGrammar = AgentModeGrammar {
        noninteractive_options: &["-v", "-p", "--print", "--output-format"],
        noninteractive_commands: &[
            "login",
            "logout",
            "status",
            "whoami",
            "mcp",
            "update",
            "upgrade",
            "ls",
            "create-chat",
            "install-shell-integration",
            "uninstall-shell-integration",
            "help",
        ],
        interactive_commands: &["resume"],
        interactive_flags: &["-f", "--force"],
        interactive_options: &["-m", "--model", "--resume"],
        ..base(M::Interactive)
    };

    /// A positional argument is the project directory.
    pub const OPENCODE: AgentModeGrammar = AgentModeGrammar {
        noninteractive_options: &["-v"],
        noninteractive_commands: &[
            "run",
            "serve",
            "web",
            "acp",
            "auth",
            "models",
            "upgrade",
            "uninstall",
            "stats",
            "export",
            "import",
            "github",
            "mcp",
            "agent",
            "debug",
            "completion",
            "session",
            "pr",
        ],
        interactive_flags: &["-c", "--continue", "--print-logs"],
        interactive_options: &[
            "-m",
            "--model",
            "-s",
            "--session",
            "--prompt",
            "--agent",
            "--log-level",
        ],
        ..base(M::Interactive)
    };

    pub const COPILOT: AgentModeGrammar = AgentModeGrammar {
        noninteractive_options: &["-v", "-p", "--prompt"],
        noninteractive_commands: &["help"],
        interactive_flags: &[
            "--allow-all-tools",
            "--allow-all-paths",
            "--continue",
            "--banner",
            "--screen-reader",
        ],
        interactive_optional_options: &["-r", "--resume"],
        interactive_options: &[
            "-i",
            "--interactive",
            "--model",
            "--agent",
            "--add-dir",
            "--allow-tool",
            "--deny-tool",
            "--log-level",
            "--log-dir",
        ],
        ..base(M::Unclassified)
    };

    pub const PI: AgentModeGrammar = AgentModeGrammar {
        noninteractive_options: &["-v", "-p", "--print", "--mode"],
        interactive_flags: &["-c", "--continue", "-r", "--resume"],
        interactive_options: &["--model", "--provider", "--thinking", "--session"],
        ..base(M::Interactive)
    };

    /// `-r` is not listed: it is `--reasoning-effort` under `exec`.
    pub const DROID: AgentModeGrammar = AgentModeGrammar {
        noninteractive_options: &["-v"],
        noninteractive_commands: &["exec", "mcp"],
        interactive_optional_options: &["--resume"],
        ..base(M::Unclassified)
    };

    pub const HERMES: AgentModeGrammar = AgentModeGrammar {
        noninteractive_options: &["-q", "--query"],
        noninteractive_commands: &[
            "setup", "model", "config", "gateway", "update", "doctor", "version", "login", "tools",
        ],
        interactive_commands: &["chat"],
        interactive_options: &["-r", "--resume"],
        ..base(M::Unclassified)
    };

    pub const LETTA: AgentModeGrammar = AgentModeGrammar {
        noninteractive_options: &[
            "-v",
            "-p",
            "--print",
            "--prompt",
            "--json",
            "--stream-json",
            "--run",
            "--output-format",
            "--input-format",
        ],
        interactive_options: &["--conversation", "--conv", "-C", "--agent", "-a"],
        ..base(M::Unclassified)
    };

    pub const GROK: AgentModeGrammar = AgentModeGrammar {
        interactive_optional_options: &["-r", "--resume"],
        ..DEFAULT
    };

    pub const QODER: AgentModeGrammar = AgentModeGrammar {
        interactive_optional_options: &["-r", "--resume"],
        ..DEFAULT
    };

    pub const KIMI: AgentModeGrammar = AgentModeGrammar {
        interactive_optional_options: &["-S", "--session"],
        ..DEFAULT
    };

    pub const ANTIGRAVITY: AgentModeGrammar = AgentModeGrammar {
        interactive_options: &["--conversation"],
        ..DEFAULT
    };

    pub const DEVIN: AgentModeGrammar = AgentModeGrammar {
        interactive_options: &["-r", "--resume"],
        ..DEFAULT
    };

    pub const CLINE: AgentModeGrammar = AgentModeGrammar {
        interactive_options: &["--id"],
        ..DEFAULT
    };

    /// `kirocrew` without a command prints its usage; `chat` alone is the interactive mode.
    pub const KIROCREW: AgentModeGrammar = AgentModeGrammar {
        noninteractive_commands: &["config"],
        interactive_commands: &["chat"],
        requires_command: true,
        interactive_flags: &["--no-jail"],
        interactive_options: &["--agent"],
        ..base(M::Unclassified)
    };

    /// `tui [PROJECT]` opens the terminal chat; a bare launch states no interactive default.
    pub const QWENPAW: AgentModeGrammar = AgentModeGrammar {
        noninteractive_commands: &["clean", "plugin", "skills", "task"],
        interactive_commands: &["tui"],
        requires_command: true,
        interactive_options: &["--resume"],
        ..base(M::Interactive)
    };

    /// `chat` is the local terminal UI (`tui --local`); `tui` attaches it to a Gateway session.
    pub const OPENCLAW: AgentModeGrammar = AgentModeGrammar {
        noninteractive_commands: &["attach", "config", "hooks", "mcp", "sessions"],
        interactive_commands: &["chat", "tui"],
        requires_command: true,
        interactive_flags: &["--local"],
        ..base(M::Unclassified)
    };

    /// `chat` opens the REPL; `run` executes a single Task.
    pub const PENGUIN: AgentModeGrammar = AgentModeGrammar {
        noninteractive_commands: &["config", "plugin", "run"],
        interactive_commands: &["chat"],
        requires_command: true,
        interactive_optional_options: &["--resume"],
        ..base(M::Unclassified)
    };
}
