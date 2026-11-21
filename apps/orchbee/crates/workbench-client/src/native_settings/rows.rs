//! The page's rows: the common settings, the launch-profile editor and About.
use super::choices::ColorTheme;
use super::metadata::{
    SettingAvailability, SettingControl, SettingEffect, SettingMetadata, SettingUnit,
};
use crate::localization::{Locale, Words, words};
use orchbee_color_themes::Polarity;
use orchbee_local_runtime::UserSettingsField;
use orchbee_workbench_protocol::UserSettingsValues;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Row {
    Language,
    Theme,
    /// The light slot's identity; the Themes category lists its themes as [`Row::ThemeOption`].
    LightTheme,
    DarkTheme,
    TerminalTheme,
    Foreground,
    Background,
    CopyOnSelect,
    ResumeAgents,
    RestoreOutput,
    Reminders,
    CompletionReminders,
    CompletionThreshold,
    Sound,
    Scrollback,
    /// Whether the Server installs new versions in the background (`update.automatic`).
    AutomaticUpdates,
    /// Whether the Server answers Claude Code's routine confirmations
    /// (`agents.auto_allow.claude`).
    AutoAllowClaude,
    /// Whether the Server answers Codex's routine confirmations
    /// (`agents.auto_allow.codex`).
    AutoAllowCodex,
    /// Whether the Server answers Gemini CLI's routine confirmations
    /// (`agents.auto_allow.gemini`).
    AutoAllowGemini,
    LaunchProfiles,
    /// Where a new terminal starts (`terminal.new_cwd`).
    NewTerminalFolder,
    /// The key pressed before a command key (`keys.prefix`).
    PrefixKey,
    /// One shipped theme of a Themes list; activating it puts it in use.
    ThemeOption(ColorTheme),
    Profile,
    ProfileName,
    CommandType,
    Executable,
    Login,
    Argument,
    ArgumentValue,
    AddArgument,
    DeleteArgument,
    SetDefault,
    AddProfile,
    DeleteProfile,
    /// Leaves a child page; leaving the launch-profile editor applies its catalog.
    Back,
    /// About: this Client's version.
    Version,
    /// About: Check for updates, or the Update control while a newer version is offered.
    Updates,
    /// Settings › Machines: opens Add Machine.
    AddMachine,
    /// Settings › Machines: filters the list, past the threshold.
    FindMachine,
    /// Settings › Machines: what this computer has free.
    ThisComputer,
    /// Settings › Machines: the saved machine at this place in the facts' order, which opens its
    /// page; its name is the machine's.
    Machine(u16),
    MachineName,
    MachineAddress,
    /// A machine's state, its sentence, and the one action it offers now.
    MachineStatus,
    /// What the machine has free, while it is online.
    MachineFree,
    /// Forgets the password saved for the machine.
    MachinePassword,
    /// Stops the machine's Session after a confirmation that states what stops.
    MachineStop,
    /// Removes the machine's record; nothing stops.
    MachineRemove,
}

/// Reads and writes the on/off value of a row's setting.
#[derive(Clone, Copy)]
pub(crate) struct Flag {
    pub(crate) read: fn(&UserSettingsValues) -> bool,
    pub(crate) write: fn(&mut UserSettingsValues, bool),
}

/// The [`Flag`] of one boolean field of the common values.
macro_rules! flag {
    ($field:ident) => {
        Flag {
            read: |values| values.$field,
            write: |values, on| values.$field = on,
        }
    };
}

/// What one row is: its words in both locales and, when it edits a setting, that setting.
#[derive(Clone, Copy)]
pub(crate) struct Declaration {
    name: Words,
    description: Option<Words>,
    applies: Option<Words>,
    setting: Option<SettingMetadata>,
    flag: Option<Flag>,
    text_key: Option<&'static str>,
}

/// A change that reaches terminals opened after it.
const NEW_TERMINALS: Words = words("new terminals", "新终端生效");

const fn row(en_us: &'static str, zh_cn: &'static str) -> Declaration {
    Declaration {
        name: words(en_us, zh_cn),
        description: None,
        applies: None,
        setting: None,
        flag: None,
        text_key: None,
    }
}

impl Declaration {
    /// The line under the row's name.
    const fn described(mut self, en_us: &'static str, zh_cn: &'static str) -> Self {
        self.description = Some(words(en_us, zh_cn));
        self
    }

    /// When a change takes effect, shown after the name of a row that does not apply at once.
    const fn applies(mut self, when: Words) -> Self {
        self.applies = Some(when);
        self
    }

    /// The common setting the row edits, available always unless [`Self::when`] says otherwise.
    const fn setting(
        mut self,
        key: UserSettingsField,
        control: SettingControl,
        effect: SettingEffect,
    ) -> Self {
        self.setting = Some(SettingMetadata {
            key,
            control,
            availability: SettingAvailability::Always,
            effect,
        });
        self
    }

    const fn when(mut self, availability: SettingAvailability) -> Self {
        if let Some(setting) = &mut self.setting {
            setting.availability = availability;
        }
        self
    }

    /// An on/off setting; the flag is all the page needs to show, read and flip it.
    const fn flag(self, key: UserSettingsField, effect: SettingEffect, flag: Flag) -> Self {
        let mut declared = self.setting(key, SettingControl::Boolean, effect);
        declared.flag = Some(flag);
        declared
    }

    /// The one text key the row edits, for a key the common values do not carry.
    const fn text_key(mut self, key: &'static str) -> Self {
        self.text_key = Some(key);
        self
    }
}

impl Row {
    /// The one declaration of a row. A new row is a variant above, its arm here, and its place
    /// in a group's list in `catalog`.
    fn declaration(self) -> Declaration {
        use SettingAvailability as A;
        use SettingControl as C;
        use SettingEffect as E;
        use UserSettingsField as F;
        match self {
            // A theme line carries its list's words and edits through the list, not as a setting.
            Self::ThemeOption(_) => Declaration {
                setting: None,
                ..self.setting_row().declaration()
            },
            Self::Language => row("Language", "语言")
                .described("Language for menus and messages.", "菜单和提示使用的语言。")
                .setting(F::Locale, C::FiniteChoice, E::CurrentApplication),
            Self::Theme => row("Application theme", "应用主题")
                .described(
                    "Light, dark, or follow the system. Pick each theme under Themes.",
                    "浅色、深色或跟随系统。浅色和深色各用哪套主题，在“主题”里选。",
                )
                .setting(F::ApplicationTheme, C::FiniteChoice, E::CurrentApplication),
            Self::LightTheme => row("Light theme", "浅色主题")
                .described(
                    "Colors for the interface and terminals when the appearance is light.",
                    "外观为浅色时，界面和终端使用的配色。",
                )
                .setting(F::LightTheme, C::FiniteChoice, E::CurrentApplication)
                .when(A::NamedThemes),
            Self::DarkTheme => row("Dark theme", "深色主题")
                .described(
                    "Colors for the interface and terminals when the appearance is dark.",
                    "外观为深色时，界面和终端使用的配色。",
                )
                .setting(F::DarkTheme, C::FiniteChoice, E::CurrentApplication)
                .when(A::NamedThemes),
            Self::TerminalTheme => row("Terminal colors", "终端配色")
                .described(
                    "Default text and background in terminals; program colors are kept.",
                    "终端默认的文字和背景颜色；程序自己指定的颜色不变。",
                )
                .setting(
                    F::TerminalDisplayTheme,
                    C::FiniteChoice,
                    E::CurrentTerminalPresentation,
                ),
            Self::Foreground => row("Text color", "文字颜色")
                .described(
                    "Terminal text color for Custom, as #RRGGBB.",
                    "“自定义”时终端的文字颜色，格式为 #RRGGBB。",
                )
                .setting(
                    F::CustomForeground,
                    C::Color,
                    E::CurrentTerminalPresentation,
                )
                .when(A::TerminalDisplayThemeCustom),
            Self::Background => row("Background color", "背景色")
                .described(
                    "Terminal background color for Custom, as #RRGGBB.",
                    "“自定义”时终端的背景色，格式为 #RRGGBB。",
                )
                .setting(
                    F::CustomBackground,
                    C::Color,
                    E::CurrentTerminalPresentation,
                )
                .when(A::TerminalDisplayThemeCustom),
            Self::CopyOnSelect => row("Copy on select", "选中即复制")
                .described(
                    "Copies text as soon as you finish selecting it.",
                    "选择完成后立即复制文字。",
                )
                .flag(
                    F::CopyOnSelect,
                    E::SubsequentSelection,
                    flag!(copy_on_select),
                ),
            Self::ResumeAgents => row(
                "Resume Agent conversations after restart",
                "重启后恢复智能体会话",
            )
            .described(
                "Reopens Claude Code on Windows; other Agents need a conversation ID.",
                "重启后恢复 Windows 上的 Claude Code；其他 Agent 需带会话 ID 启动。",
            )
            .flag(
                F::ResumeAgentsOnRestore,
                E::NextRestart,
                flag!(resume_agents_on_restore),
            ),
            Self::RestoreOutput => row(
                "Restore terminal output after restart",
                "重启后恢复终端输出",
            )
            .described(
                "Shows recent output after a restart. Kept locally; may hold secrets.",
                "重启后再次显示最近的输出。保存在本机，可能包含密钥等敏感信息。",
            )
            .flag(
                F::RestoreTerminalOutput,
                E::NextRestart,
                flag!(restore_terminal_output),
            ),
            Self::Reminders => row("Agent status notifications", "智能体状态通知")
                .described(
                    "Tells you when an Agent you can’t see finishes or needs you.",
                    "看不到的智能体完成或需要你时通知你。",
                )
                .setting(
                    F::AgentNotificationMode,
                    C::FiniteChoice,
                    E::FutureNotification,
                ),
            Self::CompletionReminders => row("Command completion reminders", "命令完成提醒")
                .described(
                    "Reminds you when a long command ends. Command text is never shown.",
                    "长命令结束时提醒你；不会显示命令内容。",
                )
                .setting(
                    F::CompletionReminderMode,
                    C::FiniteChoice,
                    E::FutureNotification,
                ),
            Self::CompletionThreshold => row("Minimum command duration", "最短命令时长")
                .described(
                    "Shorter commands don’t remind you.",
                    "比这更短的命令不提醒。",
                )
                .setting(
                    F::CompletionReminderMinimumSeconds,
                    C::BoundedInteger {
                        minimum: 1,
                        maximum: 3_600,
                        small_step: 1,
                        large_step: 10,
                        unit: SettingUnit::Seconds,
                    },
                    E::FutureNotification,
                ),
            Self::Sound => row("Notification sound", "通知声音")
                .described(
                    "Plays a sound with notifications and reminders.",
                    "通知和提醒时播放声音。",
                )
                .flag(F::SoundEnabled, E::FutureNotification, flag!(sound_enabled)),
            Self::Scrollback => row("Terminal history", "终端历史")
                .described(
                    "How much output each terminal keeps for scrolling back.",
                    "每个终端保留多少输出用于回看。",
                )
                .setting(
                    F::ScrollbackBytes,
                    C::BoundedInteger {
                        minimum: 0,
                        maximum: 1_000_000_000,
                        small_step: 1_000_000,
                        large_step: 10_000_000,
                        unit: SettingUnit::Bytes,
                    },
                    E::NewTerminal,
                )
                .applies(NEW_TERMINALS),
            Self::AutomaticUpdates => row("Automatic updates", "自动更新")
                .described(
                    "Installs in the background; nothing restarts until you choose Update.",
                    "在后台下载并安装新版本。点击“更新”前不会重启任何内容。",
                )
                .flag(
                    F::AutomaticUpdates,
                    E::NextUpdateCheck,
                    flag!(automatic_updates),
                ),
            // The Agent's product name, the same in every language.
            Self::AutoAllowClaude => row("Claude Code", "Claude Code")
                .described(
                    "Orchbee checks the command shown, not what a script does.",
                    "Orchbee 只检查显示的命令，不检查脚本做了什么。",
                )
                .flag(
                    F::AutoAllowClaude,
                    E::AgentConfirmations,
                    flag!(auto_allow_claude),
                ),
            Self::AutoAllowCodex => row("Codex", "Codex")
                .described(
                    "Orchbee checks the command shown, not what a script does.",
                    "Orchbee 只检查显示的命令，不检查脚本做了什么。",
                )
                .flag(
                    F::AutoAllowCodex,
                    E::AgentConfirmations,
                    flag!(auto_allow_codex),
                ),
            Self::AutoAllowGemini => row("Gemini CLI", "Gemini CLI")
                .described(
                    "Orchbee checks the command shown, not what a script does.",
                    "Orchbee 只检查显示的命令，不检查脚本做了什么。",
                )
                .flag(
                    F::AutoAllowGemini,
                    E::AgentConfirmations,
                    flag!(auto_allow_gemini),
                ),
            Self::LaunchProfiles => row("Default profile", "默认启动配置")
                .described(
                    "Edit profiles and choose the one new terminals use.",
                    "编辑启动配置，并选择新终端使用哪一个。",
                )
                .setting(F::LaunchProfiles, C::Catalog, E::NewDefaultLaunch)
                .applies(NEW_TERMINALS),
            Self::NewTerminalFolder => row("New terminal folder", "新终端文件夹")
                .described(
                    "Applies to terminals the orchbee command creates.",
                    "适用于 orchbee 命令新建的终端。",
                )
                .text_key("terminal.new_cwd")
                .applies(NEW_TERMINALS),
            Self::PrefixKey => row("Prefix key", "前缀键")
                .described(
                    "Press it first, then a command key.",
                    "先按它，再按命令键。",
                )
                .text_key("keys.prefix"),
            Self::Profile => row("Profile", "启动配置"),
            Self::ProfileName => row("Name", "名称"),
            Self::CommandType => row("Command", "命令"),
            Self::Executable => row("Executable (absolute path)", "可执行文件（绝对路径）"),
            Self::Login => row("Login shell", "登录 Shell"),
            Self::Argument => row("Argument", "参数"),
            Self::ArgumentValue => row("Literal argument value", "参数原文"),
            Self::AddArgument => row("Add argument", "添加参数"),
            Self::DeleteArgument => row("Remove argument", "移除参数"),
            Self::SetDefault => row("Use as default", "设为默认"),
            Self::AddProfile => row("Add profile", "添加启动配置"),
            Self::DeleteProfile => row("Delete profile", "删除启动配置"),
            Self::Back => row("Back", "返回"),
            Self::Version => row("Version", "版本"),
            Self::Updates => row("Updates", "更新").described(
                "Orchbee also checks every hour while automatic updates are on.",
                "自动更新开启时，Orchbee 也会每小时检查一次。",
            ),
            // Settings › Machines edits no setting; a machine's facts describe the other rows.
            Self::AddMachine => row("Add machine…", "添加机器…"),
            Self::FindMachine => row("Find a machine", "查找机器"),
            Self::ThisComputer => row("This computer", "这台电脑"),
            Self::Machine(_) => row("Machine", "机器"),
            Self::MachineName => row("Name", "名称").described(
                "Shown on its Workspaces and in commands.",
                "显示在它的工作区上和命令里。",
            ),
            Self::MachineAddress => {
                row("Address", "地址").described("What ssh connects to.", "ssh 连接的目标。")
            }
            Self::MachineStatus => row("Status", "状态"),
            Self::MachineFree => row("Free now", "当前空闲"),
            Self::MachinePassword => row("Saved password", "已保存的密码").described(
                "Kept in the system password store.",
                "存放在系统的密码存放处。",
            ),
            Self::MachineStop => row("Stop its terminals", "停止它的终端").described(
                "Ends every terminal and Agent there; the machine stays.",
                "结束那里的所有终端和智能体；机器仍保留。",
            ),
            Self::MachineRemove => row("Remove machine", "移除机器"),
        }
    }

    /// The row whose setting this row belongs to: a theme line's list slot, else the row itself.
    pub(crate) fn setting_row(self) -> Self {
        match self {
            Self::ThemeOption(theme) => match theme.theme().polarity() {
                Polarity::Light => Self::LightTheme,
                Polarity::Dark => Self::DarkTheme,
            },
            row => row,
        }
    }

    pub(crate) fn name(self, locale: Locale) -> &'static str {
        locale.words(self.declaration().name)
    }

    /// The line under the row's name; empty for a row without one.
    pub(crate) fn description(self, locale: Locale) -> &'static str {
        match self {
            // About's version row describes the build, which the running process knows.
            Self::Version => locale.about_build(orchbee_local_runtime::environment_kind()),
            row => row
                .declaration()
                .description
                .map_or("", |description| locale.words(description)),
        }
    }

    /// When a change takes effect, for a row that does not take effect at once.
    pub(crate) fn applies(self, locale: Locale) -> Option<&'static str> {
        self.declaration()
            .applies
            .map(|applies| locale.words(applies))
    }

    /// The common setting this row edits.
    pub(crate) fn setting(self) -> Option<SettingMetadata> {
        self.declaration().setting
    }

    /// The on/off value this row edits.
    pub(crate) fn flag(self) -> Option<Flag> {
        self.declaration().flag
    }

    /// The key of a row that edits one text key the common values do not carry.
    pub(crate) fn text_key(self) -> Option<&'static str> {
        self.declaration().text_key
    }

    /// The longest text draft this row admits: a setting control's own bound, or the editor
    /// bound of a launch-profile text row.
    pub(crate) fn text_limit(self) -> Option<usize> {
        if let Some(setting) = self.setting() {
            return match setting.control {
                SettingControl::Color => Some(7),
                SettingControl::BoundedInteger { maximum, .. } => Some(maximum.to_string().len()),
                _ => None,
            };
        }
        match self {
            Self::ProfileName => Some(512),
            Self::Executable | Self::ArgumentValue => Some(32 * 1024),
            Self::NewTerminalFolder => Some(4096),
            Self::PrefixKey => Some(64),
            Self::MachineName => Some(crate::machines::LABEL_LIMIT),
            // A filter of 128 characters, each up to four bytes.
            Self::FindMachine => Some(512),
            _ => None,
        }
    }
}

/// The categories, or a child page that opens from one of their rows and returns to it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Page {
    General,
    Profiles,
    /// One saved machine's page, opened from its row on Settings › Machines.
    Machine,
}

impl Page {
    /// A child page's rows in order; the categories list theirs through the catalog, and a
    /// machine's page through its facts.
    pub(crate) const fn rows(self) -> &'static [Row] {
        match self {
            Self::General | Self::Machine => &[],
            Self::Profiles => &[
                Row::Profile,
                Row::ProfileName,
                Row::CommandType,
                Row::Executable,
                Row::Login,
                Row::Argument,
                Row::ArgumentValue,
                Row::AddArgument,
                Row::DeleteArgument,
                Row::SetDefault,
                Row::AddProfile,
                Row::DeleteProfile,
                Row::Back,
            ],
        }
    }

    pub(crate) const fn is_child(self) -> bool {
        !matches!(self, Self::General)
    }

    /// A page of one-line rows, the launch-profile editor; the others frame two-line rows.
    pub(crate) const fn one_line_rows(self) -> bool {
        matches!(self, Self::Profiles)
    }

    /// The row that opened this child page, whose name titles it.
    pub(crate) const fn opener(self) -> Row {
        match self {
            Self::General | Self::Profiles => Row::LaunchProfiles,
            Self::Machine => Row::ThisComputer,
        }
    }
}
