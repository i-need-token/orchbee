//! Localized configuration command help and resolved-value output; does not read configuration.

use super::Locale;

pub(super) const HELP_CONFIG_CHECK_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee config check [--format text|json]",
        "Run the five local checks (runtime-base, runtime-directory, cohort, locale,\nsockets) and report pass/warning/fail/not-applicable. Exit 0 when no check\nfails, including when user-config has admitted field warnings; exit 1 on a\nfatal check. Windows user-config reads APPDATA/Orchbee/config.toml or the\nabsolute ORCHBEE_CONFIG_FILE override. Unreadable, oversized, non-UTF-8 or\ninvalid TOML and an invalid supported environment override are fatal. Unknown\nfields warn and are ignored; an invalid supported leaf warns and uses its safe\ndefault. A bad key binding disables only that explicit action.\nSupported settings include terminal defaults/cwd, worktree root, update policy, keys, title, notifications and both sidebar profiles.\nJSON v1 includes\nbounded diagnostics and their exact omitted count.\nserver reload-config atomically publishes admitted settings and diagnostics. Explicit creation arguments override admitted\nsettings. A missing default file uses defaults; a missing explicit file fails.",
    );

pub(super) const HELP_CONFIG_CHECK_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee config check [--format text|json]",
        "运行五项本地检查（runtime-base、runtime-directory、cohort、locale、\nsockets），逐项报告 pass/warning/fail/not-applicable。只要没有检查失败就退出 0，包括 user-config 仅含已接纳字段警告时；致命检查退出 1。Windows\nuser-config 读取 APPDATA/Orchbee/config.toml 或 ORCHBEE_CONFIG_FILE 指定的绝对文件。文件不可读、过大、非 UTF-8、TOML 无效或已支持的环境覆盖无效均为致命错误。未知字段会警告并忽略；已支持叶字段无效时会警告并使用其安全默认值。错误快捷键只禁用对应的显式动作。支持的设置包括终端默认值/cwd、worktree 根目录、更新策略、按键、标题、通知与两种侧栏 profile。JSON v1 包含有界诊断及精确省略数。\nserver reload-config 原子发布已接纳设置和诊断。显式创建参数覆盖已接纳设置。默认文件不存在时使用默认值；显式文件不存在则失败。",
    );

pub(super) const HELP_CONFIG_RESET_KEYS_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee config reset-keys [--format text|json]",
        "Back up APPDATA/Orchbee/config.toml (or ORCHBEE_CONFIG_FILE) and remove\ncustom keys. Preserve other settings and comments outside the removed tables.\nMissing file or no keys succeeds without writing. Invalid TOML or an unsafe\nedit fails unchanged. Windows requires a private parent and an exclusively\nopened regular file. Keep the reported backup until you no longer need to\nrestore the configuration. A failed restore reports recoveryRequired=true.\nDoes not connect, launch, or reload a Client. Exit 0 on success/no change,\n1 on failure, 2 on usage error.",
    );

pub(super) const HELP_CONFIG_RESET_KEYS_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee config reset-keys [--format text|json]",
        "备份 APPDATA/Orchbee/config.toml（或 ORCHBEE_CONFIG_FILE 指定文件），并移除自定义键位。保留其他设置及被移除表之外的注释。文件不存在或无 keys\n时成功且不写入；TOML 无效或无法安全编辑时保持不变并失败。Windows 要求私有父目录和可独占打开的普通文件。请保留回执中的备份，直到不再需要恢复配置；恢复失败时报告 recoveryRequired=true。不连接、启动或重载 Client。成功或无变更退出 0，失败退出 1，用法错误退出 2。",
    );

impl Locale {
    /// The configuration file `config check` read, as a copyable path.
    pub(crate) fn cli_config_file_line(self, path: &str) -> String {
        match self {
            Self::EnUs => format!("Config file: {path}\n"),
            Self::ZhCn => format!("配置文件：{path}\n"),
        }
    }

    pub(crate) fn cli_resolved_executable(self, path: &str) -> String {
        match self {
            Self::EnUs => format!("Resolved executable: {path}\n"),
            Self::ZhCn => format!("已解析的可执行程序：{path}\n"),
        }
    }
}
