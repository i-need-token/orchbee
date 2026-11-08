//! Formats the release confirmation prompt and refusals for `server stop`, `session stop` and
//! `session delete` (ORC-382). The admission owner supplies the Session name and the command a
//! person runs by hand; nothing here decides admission.
use super::Locale;

impl super::CliText {
    /// The console prompt; `delete` selects the verb.
    pub(crate) fn cli_destructive_prompt(self, name: &str, delete: bool) -> String {
        let name = super::escape::escape(name);
        match (self.locale, delete) {
            (Locale::EnUs, false) => format!("Type the Session name {name} to stop it: "),
            (Locale::EnUs, true) => format!("Type the Session name {name} to delete it: "),
            (Locale::ZhCn, false) => format!("输入会话名称 {name} 以停止该会话："),
            (Locale::ZhCn, true) => format!("输入会话名称 {name} 以删除该会话："),
        }
    }

    /// A refusal for one `reason` token of the `confirmation` stage. `hand` is the command a person
    /// runs in a console; `name` is the target Session for a name mismatch.
    pub(crate) fn cli_destructive_refusal(self, reason: &str, hand: &str, name: &str) -> String {
        let name = super::escape::escape(name);
        let hand = super::escape::escape(hand);
        match (self.locale, reason) {
            (Locale::EnUs, "confirm-option") => format!(
                "The release build does not accept --confirm here. Nothing was changed. Run {hand} yourself in a console and type the Session name when asked.\n"
            ),
            (Locale::ZhCn, "confirm-option") => format!(
                "正式版在此不接受 --confirm，未做任何改动。请自己在控制台运行 {hand}，并按提示输入会话名称。\n"
            ),
            (Locale::EnUs, "not-interactive") => format!(
                "The release build runs this only in an interactive console, never from an Agent, script or pipe. Nothing was changed. Run {hand} yourself in a console and type the Session name when asked.\n"
            ),
            (Locale::ZhCn, "not-interactive") => format!(
                "正式版只在交互式控制台中执行此命令，不接受智能体、脚本或管道调用，未做任何改动。请自己在控制台运行 {hand}，并按提示输入会话名称。\n"
            ),
            (Locale::EnUs, "session-unknown") => format!(
                "Cannot tell which Session this would stop. Nothing was changed. Run {hand} in a console, naming the Session.\n"
            ),
            (Locale::ZhCn, "session-unknown") => format!(
                "无法确定要停止哪个会话，未做任何改动。请在控制台运行 {hand} 并指明会话。\n"
            ),
            (Locale::EnUs, _) => format!(
                "The typed name does not match Session {name}. Nothing was changed. Run {hand} again and type the Session name exactly.\n"
            ),
            (Locale::ZhCn, _) => format!(
                "输入的名称与会话 {name} 不符，未做任何改动。请重新运行 {hand}，并准确输入会话名称。\n"
            ),
        }
    }
}
