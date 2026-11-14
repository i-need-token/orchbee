//! Formats retained-operation guidance and results without resuming or replaying an operation.
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
                    "Use the request ID from the command's receipt. Shows what the Server kept about that operation; it does not run the operation again or look up current names. server status looks for a saved stop record first, and only when there is none for an Agent detection rule operation. Finding no record does not prove that the operation had no effect."
                }
                (Locale::ZhCn, false) => {
                    "使用命令回执中的请求 ID。显示 Server 为该操作保存的情况，不会重新执行，也不按当前名称查找。server status 先查找已保存的停止记录，确认没有时再查找智能体识别规则操作。找不到记录不代表该操作没有产生效果。"
                }
                (Locale::EnUs, true) => {
                    "Use the original request ID and the generation number status printed, and add --confirm. Resume is sent once and refuses if the operation changed since that status. A close resume waits up to five seconds and reports an unknown result as unknown. Read status again before trying again."
                }
                (Locale::ZhCn, true) => {
                    "使用原始请求 ID 和 status 显示的代次编号，并加上 --confirm。恢复只提交一次；若操作在该次 status 之后已变化则拒绝。关闭操作的恢复最多等待五秒，结果未知时如实报告为未知。再次尝试前先重新读取 status。"
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
}
