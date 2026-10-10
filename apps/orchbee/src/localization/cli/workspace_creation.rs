use super::Locale;
use serde_json::Value;
impl Locale {
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

    pub(crate) fn cli_default_workspace_label(self) -> &'static str {
        match self {
            Self::EnUs => "Workspace",
            Self::ZhCn => "工作区",
        }
    }
    pub(crate) fn cli_workspace_creation(self, document: &Value) -> String {
        let mut text = match self {
            Self::EnUs => "Workspace creation\n",
            Self::ZhCn => "创建工作区\n",
        }
        .to_owned();
        for key in ["workspace"] {
            if document[key].is_null() {
                continue;
            }
            let fields = match self {
                Self::EnUs => [
                    ("projectId", "Project"),
                    ("workspaceId", "Workspace"),
                    ("label", "Label"),
                    ("revision", "Revision"),
                ],
                Self::ZhCn => [
                    ("projectId", "项目"),
                    ("workspaceId", "工作区"),
                    ("label", "名称"),
                    ("revision", "修订"),
                ],
            };
            for (field, label) in fields {
                if field == "projectId"
                    && document[key]["result"]
                        .get(field)
                        .is_some_and(Value::is_null)
                {
                    text.push_str(&format!("{label}: {}\n", self.cli_ungrouped()));
                    continue;
                }
                if let Some(value) = document[key]["result"][field].as_str() {
                    text.push_str(&format!("{label}: {}\n", crate::cli::escape::escape(value)));
                }
            }
            if let Some(value) = document[key]["result"]["terminal"]["outcome"].as_str() {
                let label = match self {
                    Self::EnUs => "Terminal",
                    Self::ZhCn => "终端",
                };
                text.push_str(&format!("{label}: {}\n", crate::cli::escape::escape(value)));
            }
        }
        let value = |key| crate::cli::escape::escape(document[key].as_str().unwrap_or("—"));
        let stage = value("stage");
        let outcome = value("outcome");
        match self {
            Self::EnUs => text.push_str(&format!("Stage: {stage}; outcome: {outcome}\n")),
            Self::ZhCn => text.push_str(&format!("阶段：{stage}；结果：{outcome}\n")),
        }
        if !document["focus"].is_null() {
            text.push_str(&self.cli_client_control(&document["focus"]));
        }
        text
    }
}
