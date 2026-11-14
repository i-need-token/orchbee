//! Conclusion lines, column headings and empty states for check and list
//! views: the result comes first and every empty list says how to fill it.
//! Raw reason structures stay in the JSON documents.
use super::Locale;

impl super::CliText {
    /// The `config check` conclusion printed before the per-check rows.
    pub(crate) fn cli_check_verdict(self, total: usize, warnings: usize, failed: usize) -> String {
        match (self.locale, failed, warnings) {
            (Locale::EnUs, 0, 0) => format!("Configuration OK: all {total} checks passed.\n"),
            (Locale::ZhCn, 0, 0) => format!("配置正常：{total} 项检查全部通过。\n"),
            (Locale::EnUs, 0, _) => format!("Configuration usable with {warnings} warning(s).\n"),
            (Locale::ZhCn, 0, _) => format!("配置可用，有 {warnings} 项警告。\n"),
            (Locale::EnUs, _, _) => {
                format!("Configuration has problems: {failed} check(s) failed.\n")
            }
            (Locale::ZhCn, _, _) => format!("配置有问题：{failed} 项检查失败。\n"),
        }
    }

    pub(crate) const fn cli_check_status(self, status: &str) -> &str {
        match (self.locale, status.as_bytes()) {
            (Locale::EnUs, b"pass") => "ok",
            (Locale::ZhCn, b"pass") => "通过",
            (Locale::EnUs, b"warning") => "warning",
            (Locale::ZhCn, b"warning") => "警告",
            (Locale::EnUs, b"fail") => "failed",
            (Locale::ZhCn, b"fail") => "失败",
            (Locale::EnUs, b"not-applicable") => "n/a",
            (Locale::ZhCn, b"not-applicable") => "不适用",
            _ => status,
        }
    }

    /// Explains the `not-started` reason when a check reports it.
    pub(crate) const fn cli_check_not_started_note(self) -> &'static str {
        match self.locale {
            Locale::EnUs => {
                "This environment has not been started yet; `orchbee server start` creates its runtime base.\n"
            }
            Locale::ZhCn => "此环境尚未启动；`orchbee server start` 会创建其运行时基座。\n",
        }
    }

    pub(crate) const fn cli_check_headers(self) -> [&'static str; 3] {
        match self.locale {
            Locale::EnUs => ["CHECK", "STATUS", "DETAIL"],
            Locale::ZhCn => ["检查项", "状态", "详情"],
        }
    }

    /// Where every effective setting and raw check detail is available.
    pub(crate) const fn cli_check_json_hint(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Every effective setting: orchbee config check --format json\n",
            Locale::ZhCn => "全部有效设置：orchbee config check --format json\n",
        }
    }

    pub(crate) const fn cli_doctor_headers(self) -> [&'static str; 4] {
        match self.locale {
            Locale::EnUs => ["PROVIDER", "EXECUTABLE", "NODE", "HOOK"],
            Locale::ZhCn => ["提供方", "可执行文件", "NODE", "钩子"],
        }
    }

    pub(crate) const fn cli_doctor_json_hint(self) -> &'static str {
        match self.locale {
            Locale::EnUs => "Raw evidence: orchbee agent doctor --format json\n",
            Locale::ZhCn => "原始证据：orchbee agent doctor --format json\n",
        }
    }

    pub(crate) const fn cli_session_headers(self) -> [&'static str; 3] {
        match self.locale {
            Locale::EnUs => ["SESSION", "STATE", "RUNTIME DIRECTORY"],
            Locale::ZhCn => ["会话", "状态", "运行时目录"],
        }
    }

    pub(crate) const fn cli_session_empty(self) -> &'static str {
        match self.locale {
            Locale::EnUs => {
                "No sessions yet. `orchbee server start` creates the default session.\n"
            }
            Locale::ZhCn => "尚无会话。`orchbee server start` 会创建默认会话。\n",
        }
    }

    /// Explains the `absent` state when at least one listed session has it.
    pub(crate) const fn cli_session_absent_note(self) -> &'static str {
        match self.locale {
            Locale::EnUs => {
                "\"not created\" sessions are named but have no runtime yet; `orchbee server start --session <name>` starts one.\n"
            }
            Locale::ZhCn => {
                "“尚未创建”的会话已命名但还没有运行时；`orchbee server start --session <名称>` 可启动。\n"
            }
        }
    }

    pub(crate) const fn cli_command_headers(self) -> [&'static str; 5] {
        match self.locale {
            Locale::EnUs => ["COMMAND", "ACTION", "KEYS", "DESCRIPTION", "RUNS"],
            Locale::ZhCn => ["命令", "动作", "按键", "说明", "运行内容"],
        }
    }

    pub(crate) const fn cli_command_empty(self) -> &'static str {
        match self.locale {
            Locale::EnUs => {
                "No custom commands. Add a [[keys.command]] entry to the config file; `orchbee --default-config` shows the rules.\n"
            }
            Locale::ZhCn => {
                "没有自定义命令。在配置文件中添加 [[keys.command]] 条目；`orchbee --default-config` 列出规则。\n"
            }
        }
    }

    pub(crate) const fn cli_launch_profile_empty(self) -> &'static str {
        match self.locale {
            Locale::EnUs => {
                "No launch profiles. Create one with `orchbee launch-profile create --name <name> --exec <path>`.\n"
            }
            Locale::ZhCn => {
                "没有启动配置。使用 `orchbee launch-profile create --name <名称> --exec <路径>` 创建。\n"
            }
        }
    }

    pub(crate) const fn cli_schema_catalog_headers(self) -> [&'static str; 3] {
        match self.locale {
            Locale::EnUs => ["SCHEMA", "VERSIONS", "DESCRIPTION"],
            Locale::ZhCn => ["SCHEMA", "版本", "说明"],
        }
    }

    pub(crate) const fn cli_schema_catalog_footer(self) -> &'static str {
        match self.locale {
            Locale::EnUs => {
                "One schema: orchbee api schema <schema-id>. Every document: orchbee api schema --format json.\n"
            }
            Locale::ZhCn => {
                "单个 schema：orchbee api schema <schema-id>。全部文档：orchbee api schema --format json。\n"
            }
        }
    }
}
