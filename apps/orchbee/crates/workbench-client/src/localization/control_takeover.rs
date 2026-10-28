use super::Locale;
use orchbee_client_server_protocol::RequestId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TakeoverStatus {
    Pending,
    Refused,
    Unknown,
    Granted,
    WaitingForFrame,
    Ready,
    Lost,
}

impl Locale {
    pub(crate) fn takeover_status(self, status: TakeoverStatus, id: &RequestId) -> String {
        let text = match (self, status) {
            (Self::EnUs, TakeoverStatus::Pending) => "Taking control",
            (Self::ZhCn, TakeoverStatus::Pending) => "正在接管",
            (Self::EnUs, TakeoverStatus::WaitingForFrame) => {
                "Control granted · waiting for terminal display"
            }
            (Self::ZhCn, TakeoverStatus::WaitingForFrame) => "已获控制权 · 正在等待终端画面",
            (Self::EnUs, TakeoverStatus::Ready) => "Control acquired · using this pane's size",
            (Self::ZhCn, TakeoverStatus::Ready) => "已接管 · 使用本面板尺寸",
            (Self::EnUs, TakeoverStatus::Refused) => "Control request refused or not submitted",
            (Self::ZhCn, TakeoverStatus::Refused) => "接管被拒绝或未提交",
            (Self::EnUs, TakeoverStatus::Unknown) => {
                "Control outcome unknown · no replay · exit and reconnect"
            }
            (Self::ZhCn, TakeoverStatus::Unknown) => "接管结果未知 · 不会重放 · 退出后重新连接",
            (Self::EnUs, TakeoverStatus::Granted) => {
                "Control grant received; display synchronization incomplete · exit and reconnect"
            }
            (Self::ZhCn, TakeoverStatus::Granted) => {
                "已收到控制权回执；画面同步未完成 · 退出后重新连接"
            }
            (Self::EnUs, TakeoverStatus::Lost) => "Control changed again · view only",
            (Self::ZhCn, TakeoverStatus::Lost) => "控制权再次变化 · 仅查看",
        };
        if matches!(status, TakeoverStatus::Unknown | TakeoverStatus::Granted) {
            format!("{text} · {id}")
        } else {
            text.to_owned()
        }
    }
}
