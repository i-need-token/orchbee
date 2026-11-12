//! Localized refusal of host effects; unavailable capabilities never imply an unknown commit.
use super::Locale;
use crate::host_capabilities::Capability;

impl Locale {
    /// A session-dependent adapter refuses for this session (no desktop, a remote login, or an
    /// outer terminal without the feature), not for the platform.
    pub(crate) fn unsupported_capability(self, capability: Capability) -> String {
        let name = self.capability_name(capability);
        match (self, capability.session_dependent()) {
            (Self::EnUs, false) => format!("{name} is not supported on this platform"),
            (Self::ZhCn, false) => format!("此平台尚不支持{name}"),
            (Self::EnUs, true) => format!("{name} isn't available in this session"),
            (Self::ZhCn, true) => format!("当前会话不支持{name}"),
        }
    }

    /// The status line's shorter form of the refusal, which leaves the platform or session
    /// implied.
    pub(crate) fn unsupported_capability_shorter(self, capability: Capability) -> String {
        let name = self.capability_name(capability);
        match (self, capability.session_dependent()) {
            (Self::EnUs, false) => format!("{name} not supported"),
            (Self::ZhCn, false) => format!("不支持{name}"),
            (Self::EnUs, true) => format!("{name} unavailable"),
            (Self::ZhCn, true) => format!("{name}不可用"),
        }
    }

    const fn capability_name(self, capability: Capability) -> &'static str {
        match (self, capability) {
            (Self::EnUs, Capability::Clipboard) => "System clipboard",
            (Self::ZhCn, Capability::Clipboard) => "系统剪贴板",
            (Self::EnUs, Capability::Browser) => "Opening links in a browser",
            (Self::ZhCn, Capability::Browser) => "在浏览器中打开链接",
            (Self::EnUs, Capability::SavedMachines) => "Saved remote machines",
            (Self::ZhCn, Capability::SavedMachines) => "已保存的远程机器",
            (Self::EnUs, Capability::DirectoryInput) => "Local directory creation",
            (Self::ZhCn, Capability::DirectoryInput) => "从本地目录创建工作区",
            (Self::EnUs, Capability::AgentLaunch) => "Local Agent launch",
            (Self::ZhCn, Capability::AgentLaunch) => "启动本地智能体",
            (Self::EnUs, Capability::PositionStorage) => "Saving the Client position",
            (Self::ZhCn, Capability::PositionStorage) => "保存客户端位置",
            (Self::EnUs, Capability::ConfigurationFile) => "Local configuration files",
            (Self::ZhCn, Capability::ConfigurationFile) => "本地配置文件",
            (Self::EnUs, Capability::SystemNotification) => "System notifications",
            (Self::ZhCn, Capability::SystemNotification) => "系统通知",
            (Self::EnUs, Capability::WindowTitle) => "Setting the host window title",
            (Self::ZhCn, Capability::WindowTitle) => "设置宿主窗口标题",
            (Self::EnUs, Capability::Graphics) => "Terminal graphics",
            (Self::ZhCn, Capability::Graphics) => "终端图形",
        }
    }
}
