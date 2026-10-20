use super::Locale;
use serde_json::Value;
impl Locale {
    pub(crate) fn operation_help(
        self,
        usage: &'static str,
        resume: bool,
    ) -> crate::localization::HelpDetails {
        crate::localization::HelpDetails::new(
            usage,
            match (self, resume) {
                (Self::EnUs, false) => {
                    "Use the exact request ID from the mutation receipt. Reads retained state without repeating the operation or resolving current resource labels. Server status checks persistent stop receipts first, then manifest operations after a definite miss. A missing record does not prove that an earlier effect never happened."
                }
                (Self::ZhCn, false) => {
                    "使用修改回执中的精确请求 ID。读取保留状态，不重复操作，也不按当前资源标签解析。Server 状态先检查持久化停止回执，确认不存在后再查询 manifest 操作。缺少记录不证明以前没有发生效果。"
                }
                (Self::EnUs, true) => {
                    "Use the original request ID and the generation returned by status. --confirm is required. Submit one fenced resume; stale generations refuse. Close resume has one five-second budget and preserves unknown submission. Read status again before another attempt."
                }
                (Self::ZhCn, true) => {
                    "使用原始请求 ID 和 status 返回的 generation，必须指定 --confirm。只提交一次带围栏恢复，拒绝过期代次。关闭恢复共用五秒预算，并保留提交结果未知状态。再次尝试前先读取 status。"
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
        match self {
            Self::EnUs => format!(
                "Operation {id}: {state}; generation {generation}; resume {}.\n",
                resume.unwrap_or("not-requested")
            ),
            Self::ZhCn => format!(
                "操作 {id}：{state}；代次 {generation}；恢复 {}。\n",
                resume.unwrap_or("未请求")
            ),
        }
    }
}
