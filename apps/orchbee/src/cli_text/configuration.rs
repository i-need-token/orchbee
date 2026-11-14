//! Localized configuration command help and resolved-value output; does not read configuration.

use super::Locale;

pub(super) const HELP_CONFIG_CHECK_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee config check [--format text|json]",
        "Run the local checks and report each as pass/warning/fail/not-applicable.\nText shows locale and user-config; runtime-base, runtime-directory, cohort,\nsockets and records appear only when they fail, with the reason. --format json\nreports every check. Exit 0 when no check\nfails, including when user-config has admitted field warnings; exit 1 on a\nfatal check. Windows user-config reads {config-file} or the\nabsolute ORCHBEE_CONFIG_FILE override. Unreadable, oversized, non-UTF-8 or\ninvalid TOML and an invalid supported environment override are fatal. Unknown\nfields warn and are ignored; an invalid supported leaf warns and uses its safe\ndefault. A bad key binding disables only that explicit action.\nSupported settings include terminal defaults/cwd, worktree root, update policy, keys, title, notifications and both sidebar profiles.\nJSON v1 includes\nbounded diagnostics and their exact omitted count.\nconfig reload atomically publishes admitted settings and diagnostics. Explicit creation arguments override admitted\nsettings. A missing default file uses defaults; a missing explicit file fails.",
    );

pub(super) const HELP_CONFIG_CHECK_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee config check [--format text|json]",
        "运行本地检查，逐项报告 pass/warning/fail/not-applicable。文本列出 locale 和\nuser-config；runtime-base、runtime-directory、cohort、sockets 和 records\n仅在失败时连同原因列出。--format json 报告全部检查。只要没有检查失败就退出 0，包括 user-config 仅含已接纳字段警告时；致命检查退出 1。Windows\nuser-config 读取 {config-file} 或 ORCHBEE_CONFIG_FILE 指定的绝对文件。文件不可读、过大、非 UTF-8、TOML 无效或已支持的环境覆盖无效均为致命错误。未知字段会警告并忽略；已支持叶字段无效时会警告并使用其安全默认值。错误快捷键只禁用对应的显式动作。支持的设置包括终端默认值/cwd、worktree 根目录、更新策略、按键、标题、通知与两种边栏 profile。JSON v1 包含有界诊断及精确省略数。\nconfig reload 原子发布已接纳设置和诊断。显式创建参数覆盖已接纳设置。默认文件不存在时使用默认值；显式文件不存在则失败。",
    );

pub(super) const HELP_CONFIG_DEFAULT_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee config default",
        "Print the default configuration as TOML from this build. Reads no settings, starts no
session and takes no other argument.",
    );

pub(super) const HELP_CONFIG_DEFAULT_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee config default",
        "以 TOML 输出本版本的默认配置。不读取设置，不启动会话，也不接受其他参数。",
    );

/// Help copy names the configuration file this environment reads by default with this placeholder,
/// in every locale; [`fill_environment_placeholders`] fills it where the copy enters a help page.
pub(crate) const CONFIGURATION_FILE_PLACEHOLDER: &str = "{config-file}";

/// Fills the environment placeholders in help copy from this build's environment.
pub(crate) fn fill_environment_placeholders(copy: &str) -> String {
    copy.replace(CONFIGURATION_FILE_PLACEHOLDER, &configuration_file())
}

/// The configuration file this environment reads by default, `config.toml` in its data root,
/// spelled with the home variable instead of a resolved path: `%USERPROFILE%\.<stem>\config.toml`
/// on Windows, `~/.<stem>/config.toml` elsewhere. The data root owner derives it from that
/// reference, so help reads no environment and never spells the layout.
fn configuration_file() -> String {
    #[cfg(windows)]
    let home = format!("%{}%", orchbee_local_runtime::DataRoot::home_variable());
    #[cfg(not(windows))]
    let home = "~".to_owned();
    orchbee_local_runtime::DataRoot::under_home(std::path::Path::new(&home))
        .config_file()
        .to_string_lossy()
        .into_owned()
}

impl super::CliText {
    /// The configuration file `config check` read, as a copyable path.
    pub(crate) fn cli_config_file_line(self, path: &str) -> String {
        match self.locale {
            Locale::EnUs => format!("Config file: {path}\n"),
            Locale::ZhCn => format!("配置文件：{path}\n"),
        }
    }

    /// What is wrong with a configuration file that does not parse, and the fix; it follows a
    /// `<path>:<line>:<column>` prefix.
    pub(crate) fn cli_configuration_syntax(self, reason: &str, line: u64) -> String {
        match (self.locale, reason) {
            (Locale::EnUs, "invalid-utf8") => {
                format!("not valid UTF-8. Fix line {line}, save the file as UTF-8 and try again.")
            }
            (Locale::ZhCn, "invalid-utf8") => {
                format!("不是有效的 UTF-8。请修正第 {line} 行，以 UTF-8 保存文件后重试。")
            }
            (Locale::EnUs, _) => format!("not valid TOML. Fix line {line} and try again."),
            (Locale::ZhCn, _) => format!("不是有效的 TOML。请修正第 {line} 行后重试。"),
        }
    }

    /// The line for a configuration refusal whose reason has its own fix, from the closed
    /// `{field, reason, line?, column?}` details; `None` leaves the generic message.
    pub(crate) fn cli_configuration_refusal(
        self,
        reason: &str,
        position: Option<(u64, u64)>,
    ) -> Option<String> {
        const MIB: u64 = orchbee_local_runtime::MAXIMUM_USER_SETTINGS_BYTES / (1024 * 1024);
        let message = match (self.locale, reason, position) {
            (locale, "invalid-toml" | "invalid-utf8", Some((line, column))) => {
                let problem = self.cli_configuration_syntax(reason, line);
                match locale {
                    Locale::EnUs => {
                        format!("Configuration file line {line}, column {column}: {problem}")
                    }
                    Locale::ZhCn => format!("配置文件第 {line} 行第 {column} 列：{problem}"),
                }
            }
            (Locale::EnUs, "read-only", _) => "The configuration file is read-only, so nothing was saved. Clear its read-only attribute and try again.".to_owned(),
            (Locale::ZhCn, "read-only", _) => "配置文件为只读，未保存任何内容。请清除其只读属性后重试。".to_owned(),
            (Locale::EnUs, "file-in-use", _) => "Another program has the configuration file open, so nothing was saved. Close the editor holding it and try again.".to_owned(),
            (Locale::ZhCn, "file-in-use", _) => "另一个程序正打开配置文件，未保存任何内容。请关闭占用它的编辑器后重试。".to_owned(),
            (Locale::EnUs, "locked", _) => "Another Orchbee change to the configuration file did not finish in time, so nothing was saved. Try again in a moment.".to_owned(),
            (Locale::ZhCn, "locked", _) => "另一个 Orchbee 对配置文件的修改未及时完成，未保存任何内容。请稍后重试。".to_owned(),
            (Locale::EnUs, "changed", _) => "The configuration file changed while it was being saved, so nothing was saved. Try again.".to_owned(),
            (Locale::ZhCn, "changed", _) => "保存期间配置文件被修改，未保存任何内容。请重试。".to_owned(),
            (Locale::EnUs, "too-large", _) => format!("The configuration file is over the {MIB} MiB limit. Remove settings you do not need and try again."),
            (Locale::ZhCn, "too-large", _) => format!("配置文件超过 {MIB} MiB 上限。请删除不需要的设置后重试。"),
            _ => return None,
        };
        Some(message + "\n")
    }

    pub(crate) fn cli_resolved_executable(self, path: &str) -> String {
        match self.locale {
            Locale::EnUs => format!("Resolved executable: {path}\n"),
            Locale::ZhCn => format!("已解析的可执行程序：{path}\n"),
        }
    }
}
