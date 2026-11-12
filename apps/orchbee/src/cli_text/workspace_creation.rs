//! Formats Workspace preparation failures and creation receipts without choosing paths or launching processes.
use super::Locale;
use serde_json::Value;
impl super::CliText {
    pub(crate) fn cli_workspace_preparation_failure(
        self,
        kind: crate::cli_outcome::FailureKind,
    ) -> &'static str {
        use crate::cli_outcome::FailureKind;
        match (self.locale, kind) {
            (Locale::EnUs, FailureKind::NotFound) => {
                "No usable source Workspace or primary terminal was found.\n"
            }
            (Locale::ZhCn, FailureKind::NotFound) => "未找到可用的来源工作区或主终端。\n",
            (Locale::EnUs, FailureKind::ServiceUnavailable) => {
                "The directory or Client state needed to create the Workspace is unavailable.\n"
            }
            (Locale::ZhCn, FailureKind::ServiceUnavailable) => {
                "无法读取创建工作区所需的目录或客户端状态。\n"
            }
            (Locale::EnUs, FailureKind::ProtocolInvalid) => {
                "The source directory or Client observation is invalid.\n"
            }
            (Locale::ZhCn, FailureKind::ProtocolInvalid) => "来源目录或客户端观测无效。\n",
            (Locale::EnUs, FailureKind::TransportLoss) => {
                "The connection was lost while preparing Workspace creation.\n"
            }
            (Locale::ZhCn, FailureKind::TransportLoss) => "准备创建工作区时连接中断。\n",
            (Locale::EnUs, FailureKind::Timeout) => "Workspace creation preparation timed out.\n",
            (Locale::ZhCn, FailureKind::Timeout) => "准备创建工作区时超时。\n",
            (Locale::EnUs, _) => "Workspace creation preparation was cancelled.\n",
            (Locale::ZhCn, _) => "已取消工作区创建准备。\n",
        }
    }

    pub(crate) fn cli_workspace_creation(self, document: &Value) -> String {
        let document = &document["result"];
        let result = &document["workspace"]["result"];
        let complete = document["outcome"] == "completed";
        let mut text = if result.is_object() {
            self.cli_receipt(result)
        } else {
            String::new()
        };
        if let Some(hint) = result["label"]
            .as_str()
            .and_then(|label| self.cli_label_target_hint(label))
        {
            text.push_str(hint);
        }
        if !complete {
            // Without a committed Workspace receipt a refusal applied nothing;
            // only a stage after the commit leaves a partial creation. An
            // unknown outcome may have committed, so it keeps the stage report.
            let created_nothing =
                document["workspace"].is_null() && document["failure"]["kind"] != "outcome-unknown";
            if created_nothing {
                text.push_str(match self.locale {
                    Locale::EnUs => "Workspace not created.\n",
                    Locale::ZhCn => "未创建工作区。\n",
                });
            } else {
                let stage =
                    crate::cli_text::escape::escape(document["stage"].as_str().unwrap_or("—"));
                text.push_str(&match self.locale {
                    Locale::EnUs => format!("Workspace creation incomplete at stage {stage}.\n"),
                    Locale::ZhCn => format!("工作区创建未完成，停在阶段 {stage}。\n"),
                });
            }
            // Name the exact directory a known directory refusal concerns.
            if document["failure"]["details"]["reason"] == "directory-unavailable"
                && let Some(path) = document["directory"]["path"].as_str()
            {
                let path = crate::cli_text::escape::quoted(path);
                text.push_str(&match self.locale {
                    Locale::EnUs => format!("Directory: {path}\n"),
                    Locale::ZhCn => format!("目录：{path}\n"),
                });
            }
        }
        if !document["focus"].is_null() {
            text.push_str(&self.cli_client_control(&document["focus"]));
        }
        text
    }
}
