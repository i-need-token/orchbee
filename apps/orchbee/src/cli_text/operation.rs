//! Formats `operation` guidance, close results and lookup refusals without resuming or replaying
//! an operation.
use super::Locale;
use serde_json::Value;
impl super::CliText {
    pub(crate) fn operation_help(
        self,
        usage: &'static str,
        resume: bool,
    ) -> crate::cli_text::HelpDetails {
        crate::cli_text::HelpDetails::new(
            usage,
            match (self.locale, resume) {
                (Locale::EnUs, false) => {
                    "Use the request ID from a command's receipt; no family word is needed. The ID is looked up, in this order, in Session stop records, Workspace, Tab and Panel closes, Project close-all, Worktree operations, Client controls and Agent detection rule operations, and the first that keeps it answers with its own document (the JSON schema names which). One that cannot answer, for example with no running Session, ends the lookup with its failure. Shows what was kept about the operation; it does not run the operation again or look up current names. Finding no record (exit 3) does not prove that the operation had no effect."
                }
                (Locale::ZhCn, false) => {
                    "使用命令回执中的请求 ID，无需指明类别。依次在会话停止记录、工作区／标签页／面板关闭、项目全部关闭、工作树操作、客户端控制和智能体识别规则操作中查找该 ID，由第一个保存它的记录以其原有文档应答（JSON 的 schema 说明是哪一种）。某处无法应答（例如没有运行中的会话）时，以其失败结束查找。仅显示为该操作保存的情况，不会重新执行，也不按当前名称查找。找不到记录（退出码 3）不代表该操作没有产生效果。"
                }
                (Locale::EnUs, true) => {
                    "Use the original request ID and the generation number status printed, and add --confirm. The operation is found as operation status finds it, then its resume is sent once and refuses if the operation changed since that status. Workspace, Tab, Panel, Project and Worktree operations resume; a Session stop is retried with orchbee server stop --retry --request-id <id>, and Client controls and Agent detection rule operations cannot be resumed. A close resume waits up to five seconds and returns once it is submitted; --detach and --timeout (default 120000 ms) bound only a Project or Worktree resume. An unknown result is reported as unknown. Read status again before trying again."
                }
                (Locale::ZhCn, true) => {
                    "使用原始请求 ID 和 status 显示的代次编号，并加上 --confirm。按 operation status 的方式找到该操作后，只提交一次恢复；若操作在该次 status 之后已变化则拒绝。工作区、标签页、面板、项目和工作树操作可以恢复；会话停止请用 orchbee server stop --retry --request-id <id> 重试，客户端控制和智能体识别规则操作不能恢复。关闭操作的恢复最多等待五秒，提交后即返回；--detach 和 --timeout（默认 120000 毫秒）只约束项目或工作树的恢复。结果未知时如实报告为未知。再次尝试前先重新读取 status。"
                }
            },
        )
    }
    pub(crate) fn operation_result(self, result: &Value) -> String {
        let id = result["requestId"].as_str().unwrap_or("");
        let state = result["observation"]["outcome"]["status"]["kind"]
            .as_str()
            .or_else(|| result["observation"]["outcome"]["kind"].as_str())
            .unwrap_or("unavailable");
        let generation = &result["observation"]["outcome"]["generation"];
        let resume = result["resume"]["submission"]["outcome"]["kind"]
            .as_str()
            .or_else(|| result["resume"]["submission"]["kind"].as_str());
        match self.locale {
            Locale::EnUs => format!(
                "Operation {id}: {state}; generation {generation}; resume {}.\n",
                resume.unwrap_or("not-requested")
            ),
            Locale::ZhCn => format!(
                "操作 {id}：{state}；代次 {generation}；恢复 {}。\n",
                resume.unwrap_or("未请求")
            ),
        }
    }
    /// The text of the lookup that found the request ID nowhere.
    pub(crate) fn operation_not_recorded(self) -> String {
        match self.locale {
            Locale::EnUs => {
                "No operation is recorded under this request ID; that does not prove the operation had no effect.\n"
            }
            Locale::ZhCn => "没有以此请求 ID 记录的操作；这不代表该操作没有产生效果。\n",
        }
        .to_owned()
    }
    /// The text of a resume refused before submission; a Session stop names its own retry on
    /// Windows. A failed macOS or Linux attempt ends the Session's processes by force and waits
    /// for no retry, so there the stop is named for a status read.
    pub(crate) fn operation_resume_unsupported(self, stop: Option<&str>) -> String {
        match (self.locale, stop) {
            (Locale::EnUs, Some(id)) if cfg!(windows) => format!(
                "This request ID names a Session stop, which operation resume does not continue; nothing was submitted. Retry it with: orchbee server stop --retry --request-id {id}\n"
            ),
            (Locale::ZhCn, Some(id)) if cfg!(windows) => format!(
                "此请求 ID 对应会话停止，operation resume 不继续该操作；未提交任何请求。请用以下命令重试：orchbee server stop --retry --request-id {id}\n"
            ),
            (Locale::EnUs, Some(id)) => format!(
                "This request ID names a Session stop, which operation resume does not continue; nothing was submitted. Read it with: orchbee operation status {id}\n"
            ),
            (Locale::ZhCn, Some(id)) => format!(
                "此请求 ID 对应会话停止，operation resume 不继续该操作；未提交任何请求。请用以下命令查看：orchbee operation status {id}\n"
            ),
            (Locale::EnUs, None) => {
                "This operation cannot be resumed; nothing was submitted.\n".to_owned()
            }
            (Locale::ZhCn, None) => "此操作无法恢复；未提交任何请求。\n".to_owned(),
        }
    }
}
