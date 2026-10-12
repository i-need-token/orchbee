pub(super) const HELP_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel create-terminal [<panel>|--panel <target>|--current] [--client <id>] [--child|--presentation <id>] [--label <label>] [--exec <absolute-path> --arg <value>...|--profile <selector>] [--cwd <path|follow|home|current|workspace>] [--env NAME=VALUE] [--cols <n>] [--rows <n>] [--cell-width <n>] [--cell-height <n>] [--timeout <ms>] [--format text|json]",
    "Start an empty primary terminal slot, bind an exact empty member with --presentation,\nor append a new child with --child (at most 16). --label names only a new child.\nExplicit Panel targets work without a TUI. Omitted targets use the validated caller\nor one Ready Client; --client selects that Client. Client focus is unchanged.\nArguments and environment values remain literal. Default total timeout: 30000 ms.\nPending means accepted, not yet started. Uncertain requests are never retried;\nuse JSON receipts to retain exact request and member identities. Windows only.",
);
pub(super) const HELP_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee panel create-terminal [<panel>|--panel <target>|--current] [--client <id>] [--child|--presentation <id>] [--label <label>] [--exec <absolute-path> --arg <value>...|--profile <selector>] [--cwd <path|follow|home|current|workspace>] [--env NAME=VALUE] [--cols <n>] [--rows <n>] [--cell-width <n>] [--cell-height <n>] [--timeout <ms>] [--format text|json]",
    "启动空主终端位；--presentation 绑定精确空成员；--child 追加新子终端（最多 16 个）。\n--label 仅命名新子项。显式 Panel 目标无需 TUI，省略目标使用经过验证的调用者或唯一 Ready Client；--client 指定该 Client。不改变 Client 焦点。参数和环境变量保持字面值，总超时默认 30000 ms。Pending 表示已接受，尚未确认启动。不确定请求不会重试；JSON 回执保留精确请求及成员身份。仅支持 Windows。",
);
impl super::Locale {
    pub(crate) fn cli_panel_terminal_create(self, document: &serde_json::Value) -> String {
        let outcome = document["result"]["kind"].as_str().unwrap_or_default();
        let terminal = document["result"]["terminalOutcome"]["kind"].as_str();
        let text = match (self, outcome, terminal) {
            (Self::EnUs, "panel-terminal-pending", _) => {
                "Terminal creation accepted; launch pending"
            }
            (Self::ZhCn, "panel-terminal-pending", _) => "已接受终端创建，等待启动结果",
            (Self::EnUs, "panel-terminal-created", Some("started")) => "Terminal started",
            (Self::ZhCn, "panel-terminal-created", Some("started")) => "终端已启动",
            (Self::EnUs, _, _) => "Terminal creation incomplete; inspect the retained receipt",
            (Self::ZhCn, _, _) => "终端创建未完成，请检查保留的回执",
        };
        let (request, member) = match self {
            Self::EnUs => ("Request", "Presentation"),
            Self::ZhCn => ("请求", "终端成员"),
        };
        let mut result = format!(
            "{text}\n{request}: {}\n",
            document["source"]["requestId"].as_str().unwrap_or_default()
        );
        if let Some(id) = document["result"]["presentationId"].as_str() {
            result.push_str(&format!("{member}: {id}\n"));
        }
        result
    }
}
