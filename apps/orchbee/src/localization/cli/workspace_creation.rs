use super::Locale;
use serde_json::Value;
impl Locale {
    #[cfg(windows)]
    pub(crate) fn cli_workspace_preparation_failure(
        self,
        kind: crate::cli::FailureKind,
    ) -> &'static str {
        use crate::cli::FailureKind;
        match (self, kind) {
            (Self::EnUs, FailureKind::NotFound) => {
                "No usable source Workspace or primary terminal was found.\n"
            }
            (Self::ZhCn, FailureKind::NotFound) => "未找到可用的来源工作区或主终端。\n",
            (Self::EnUs, FailureKind::ServiceUnavailable) => {
                "The directory or Client state needed to create the Workspace is unavailable.\n"
            }
            (Self::ZhCn, FailureKind::ServiceUnavailable) => {
                "无法读取创建工作区所需的目录或客户端状态。\n"
            }
            (Self::EnUs, FailureKind::ProtocolInvalid) => {
                "The source directory or Client observation is invalid.\n"
            }
            (Self::ZhCn, FailureKind::ProtocolInvalid) => "来源目录或客户端观测无效。\n",
            (Self::EnUs, FailureKind::TransportLoss) => {
                "The connection was lost while preparing Workspace creation.\n"
            }
            (Self::ZhCn, FailureKind::TransportLoss) => "准备创建工作区时连接中断。\n",
            (Self::EnUs, FailureKind::Timeout) => "Workspace creation preparation timed out.\n",
            (Self::ZhCn, FailureKind::Timeout) => "准备创建工作区时超时。\n",
            (Self::EnUs, _) => "Workspace creation preparation was cancelled.\n",
            (Self::ZhCn, _) => "已取消工作区创建准备。\n",
        }
    }

    #[cfg(windows)]
    pub(crate) fn cli_workspace_creation(self, document: &Value) -> String {
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
            let stage = crate::cli::escape::escape(document["stage"].as_str().unwrap_or("—"));
            text.push_str(&match self {
                Self::EnUs => format!("Workspace creation incomplete at stage {stage}.\n"),
                Self::ZhCn => format!("工作区创建未完成，停在阶段 {stage}。\n"),
            });
            // Name the exact directory a known directory refusal concerns.
            if document["failure"]["details"]["reason"] == "directory-unavailable"
                && let Some(path) = document["directory"]["path"].as_str()
            {
                let path = crate::cli::escape::quoted(path);
                text.push_str(&match self {
                    Self::EnUs => format!("Directory: {path}\n"),
                    Self::ZhCn => format!("目录：{path}\n"),
                });
            }
        }
        if !document["focus"].is_null() {
            text.push_str(&self.cli_client_control(&document["focus"]));
        }
        text
    }
}
