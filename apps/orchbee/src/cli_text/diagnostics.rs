//! `diagnostics create` help, receipt and failure text.
use super::Locale;
use crate::cli_text::escape::diagnostic_token;

pub(super) const HELP_DIAGNOSTICS_CREATE_EN: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee diagnostics create --output <path> [--format text|json]",
        "Write one diagnostic bundle, a ZIP file, to <path> so you can send it yourself to\nwhoever helps you. Nothing is uploaded, and nothing stays in the data folder.\nThe bundle holds product and build versions with the running Sessions, the\noperating system, architecture, locale and terminal, config check output,\nconfig.toml and preferences with every command, argument, environment value,\nhome path and URL credential replaced by a placeholder, the Session list with\nstates, Agent integration status when the default Session is running, and the\nlogs of the last 7 days. It never holds terminal output, typed input,\ncheckpoints, layouts, plugin data or saved machines.\nThe parent folder must exist; an existing regular file is replaced only after\nthe bundle is complete. Windows only. Exit 0 on success, 1 when the file cannot\nbe written, 2 on usage error.",
    );

pub(super) const HELP_DIAGNOSTICS_CREATE_ZH: crate::cli_text::HelpDetails =
    crate::cli_text::HelpDetails::new(
        "orchbee diagnostics create --output <path> [--format text|json]",
        "将一个诊断包（ZIP 文件）写入 <path>，由你自己发给提供帮助的人。不会上传任何内容，数据文件夹中也不会留下文件。\n诊断包包含产品与构建版本及正在运行的会话、操作系统、架构、语言和终端、config check 输出、config.toml 与偏好设置（其中所有命令、参数、环境变量值、主目录路径和 URL 凭据都替换为占位符）、会话列表及其状态、默认会话运行时的 Agent 集成状态，以及最近 7 天的日志。不包含终端输出、键入内容、检查点、布局、插件数据或已保存的机器。\n父目录必须存在；诊断包完整生成后才会替换已有普通文件。仅限 Windows。成功退出 0，无法写入文件退出 1，用法错误退出 2。",
    );

impl super::CliText {
    pub(crate) fn diagnostics_created(self, path: &str, files: usize, bytes: usize) -> String {
        let path = diagnostic_token(path);
        let kibibytes = bytes.div_ceil(1024);
        match self.locale {
            Locale::EnUs => format!(
                "Diagnostic bundle written to {path} ({files} files, {kibibytes} KiB).\nNothing was uploaded; review the bundle before you send it.\n"
            ),
            Locale::ZhCn => format!(
                "诊断包已写入 {path}（{files} 个文件，{kibibytes} KiB）。\n未上传任何内容；发送前请先查看诊断包。\n"
            ),
        }
    }

    pub(crate) fn diagnostics_failure(
        self,
        stage: &str,
        path: &str,
        residue: Option<&str>,
    ) -> String {
        let action = match (self.locale, stage) {
            (Locale::EnUs, "destination") => "checking the destination",
            (Locale::EnUs, "create-temporary") => "creating a temporary file",
            (Locale::EnUs, "write-temporary") => "writing the temporary file",
            (Locale::EnUs, "sync-temporary") => "flushing the temporary file",
            (Locale::EnUs, _) => "replacing the destination",
            (Locale::ZhCn, "destination") => "检查目标文件",
            (Locale::ZhCn, "create-temporary") => "创建临时文件",
            (Locale::ZhCn, "write-temporary") => "写入临时文件",
            (Locale::ZhCn, "sync-temporary") => "刷写临时文件",
            (Locale::ZhCn, _) => "替换目标文件",
        };
        let path = diagnostic_token(path);
        let mut text = match self.locale {
            Locale::EnUs => {
                format!("Could not write the diagnostic bundle to {path} while {action}.\n")
            }
            Locale::ZhCn => format!("写入诊断包到 {path} 失败：无法{action}。\n"),
        };
        if let Some(residue) = residue {
            let residue = diagnostic_token(residue);
            text.push_str(&match self.locale {
                Locale::EnUs => format!("Temporary file could not be removed: {residue}.\n"),
                Locale::ZhCn => format!("临时文件未能删除：{residue}。\n"),
            });
        }
        text.push_str(match self.locale {
            Locale::EnUs => "Run 'orchbee help diagnostics create' for usage.\n",
            Locale::ZhCn => "用法请运行 'orchbee help diagnostics create'。\n",
        });
        text
    }
}
