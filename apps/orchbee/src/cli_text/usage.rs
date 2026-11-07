//! Localized invocation rejection reasons; the invocation grammar owns admission.

use super::Locale;

mod advice;

impl super::CliText {
    /// The localized `invalid-usage` stderr diagnostic: one concrete cause
    /// plus a help hint that always names a real path. `None` and the
    /// `Invalid` fallback keep the original generic text.
    pub(super) fn cli_usage(self, usage: Option<&crate::invocation::UsageReason>) -> String {
        use crate::invocation::{ExpectedValue, SendSourceCause, UsageContext, UsageReason};
        let Some(reason) = usage else {
            return self.locale.usage_failure().to_owned();
        };
        let echo = |value: &str| crate::cli_text::escape::diagnostic_token(value);
        let context = |context: UsageContext| match context {
            UsageContext::Root => "orchbee".to_owned(),
            UsageContext::Path(path) => format!("orchbee {path}"),
            UsageContext::Literal(literal) => literal.to_owned(),
        };
        let hint = |context: UsageContext| match context {
            UsageContext::Path(path) => format!("orchbee {path} --help"),
            UsageContext::Root | UsageContext::Literal(_) => "orchbee --help".to_owned(),
        };
        let expected = |expected: ExpectedValue| match (self.locale, expected) {
            (Locale::EnUs, ExpectedValue::DeclaredValue) => {
                "a value declared in this command's help"
            }
            (Locale::ZhCn, ExpectedValue::DeclaredValue) => "此命令帮助中列出的取值",
            (Locale::EnUs, ExpectedValue::TerminalViewportPoint) => {
                "viewport ROW:COL (row/column 0..65535)"
            }
            (Locale::ZhCn, ExpectedValue::TerminalViewportPoint) => {
                "视口 ROW:COL（行／列 0..65535）"
            }
            (Locale::EnUs, ExpectedValue::TerminalSelectionPoint) => {
                "ROW:COL (row 0..4294967295, column 0..65535)"
            }
            (Locale::ZhCn, ExpectedValue::TerminalSelectionPoint) => {
                "ROW:COL（行 0..4294967295，列 0..65535）"
            }
            (Locale::EnUs, ExpectedValue::TerminalSelectionSpan) => {
                "a selection spanning at most 4096 rows"
            }
            (Locale::ZhCn, ExpectedValue::TerminalSelectionSpan) => "最多跨 4096 行的选区",
            (_, ExpectedValue::TerminalContentRevision) => "0..18446744073709551615",
            (_, ExpectedValue::TerminalContentGeneration) => "1..18446744073709551615",
            (Locale::EnUs, ExpectedValue::TerminalCopyMotion) => {
                "one of the ten motions listed by help terminal copy-motion"
            }
            (Locale::ZhCn, ExpectedValue::TerminalCopyMotion) => {
                "help terminal copy-motion 列出的十种移动之一"
            }
            (Locale::EnUs, ExpectedValue::TerminalCopyQuery) => {
                "a literal query of at most 4096 UTF-8 bytes"
            }
            (Locale::ZhCn, ExpectedValue::TerminalCopyQuery) => "最多 4096 UTF-8 字节的字面量查询",
            (_, ExpectedValue::TerminalCopyDirection) => "forward|backward",
            (Locale::EnUs, ExpectedValue::TerminalCopyPrevious) => {
                "ROW:COL-ROW:COL (inclusive buffer endpoints)"
            }
            (Locale::ZhCn, ExpectedValue::TerminalCopyPrevious) => {
                "ROW:COL-ROW:COL（包含缓冲区端点）"
            }

            (Locale::EnUs, ExpectedValue::PluginActionId) => {
                "an action ID or plugin.action ID (up to 241 ASCII characters)"
            }
            (Locale::ZhCn, ExpectedValue::PluginActionId) => {
                "action ID 或 plugin.action ID（最多 241 个 ASCII 字符）"
            }
            (Locale::EnUs, ExpectedValue::GraphicsSpecification) => {
                "a closed graphics spec JSON object, @regular-file path, or - for noninteractive stdin"
            }
            (Locale::ZhCn, ExpectedValue::GraphicsSpecification) => {
                "封闭图形 spec JSON 对象、@普通文件路径，或用于非交互 stdin 的 -"
            }
            (Locale::EnUs, ExpectedValue::EventsSpecification) => {
                "a valid event name, closed filter/cursor JSON, or the documented bounded event option"
            }
            (Locale::ZhCn, ExpectedValue::EventsSpecification) => {
                "有效事件名、封闭 filter/cursor JSON，或帮助中声明的有界事件参数"
            }
            (Locale::EnUs, ExpectedValue::ConfiguredCommandId) => {
                "an installed opaque command ID from command list"
            }
            (Locale::ZhCn, ExpectedValue::ConfiguredCommandId) => {
                "command list 显示的已安装不透明命令 ID"
            }
            (Locale::EnUs, ExpectedValue::PluginContext) => {
                "a bounded plugin context JSON object (16 KiB)"
            }
            (Locale::ZhCn, ExpectedValue::PluginContext) => "有界插件上下文 JSON 对象（16 KiB）",
            (_, ExpectedValue::PluginLogLimit) => "1..200",
            (_, ExpectedValue::PluginPopupSize) => "0..65535 | 1%..100%",
            (_, ExpectedValue::PluginPanePlacement) => "tab|split|zoomed|overlay|popup",
            (_, ExpectedValue::PluginPaneDirection) => "left|right|up|down",
            (_, ExpectedValue::PluginGithubSource) => "owner/repo[/subdir]",
            (Locale::EnUs, ExpectedValue::PluginGitReference) => "a Git branch, tag or commit",
            (Locale::ZhCn, ExpectedValue::PluginGitReference) => "Git 分支、标签或提交",

            (Locale::EnUs, ExpectedValue::PluginId) => {
                "1-120 ASCII letters, digits, colon, dot, underscore or hyphen"
            }
            (Locale::ZhCn, ExpectedValue::PluginId) => {
                "1-120 个 ASCII 字母、数字、冒号、点、下划线或连字符"
            }
            (Locale::EnUs, ExpectedValue::WindowTitle) => {
                "nonempty title after removing controls and trimming"
            }
            (Locale::ZhCn, ExpectedValue::WindowTitle) => "移除控制字符并 trim 后非空的标题",
            (Locale::EnUs, ExpectedValue::NotificationTitle) => "a nonempty notification title",
            (Locale::ZhCn, ExpectedValue::NotificationTitle) => "非空通知标题",
            (_, ExpectedValue::NotificationPosition) => {
                "top-left, top-right, bottom-left, bottom-right"
            }
            (_, ExpectedValue::NotificationSound) => "none, done, request",
            (Locale::EnUs, ExpectedValue::ReportAgent) => "a nonempty Agent label",
            (Locale::ZhCn, ExpectedValue::ReportAgent) => "非空智能体标签",
            (Locale::EnUs, ExpectedValue::ReportState) => {
                "idle, working, blocked, done, error, or unknown"
            }
            (Locale::ZhCn, ExpectedValue::ReportState) => {
                "idle、working、blocked、done、error 或 unknown"
            }
            (Locale::EnUs, ExpectedValue::ReportSource) => "a nonempty report source",
            (Locale::ZhCn, ExpectedValue::ReportSource) => "非空上报来源",
            (Locale::EnUs, ExpectedValue::ReportSize) => {
                "an Agent report within the 16 KiB request limit"
            }
            (Locale::ZhCn, ExpectedValue::ReportSize) => "完整请求不超过 16 KiB 的智能体上报",
            (Locale::EnUs, ExpectedValue::MetadataAgent) => {
                "a nonempty Agent label, at most 80 Unicode scalars without control characters"
            }
            (Locale::ZhCn, ExpectedValue::MetadataAgent) => {
                "非空智能体标签，至多 80 个 Unicode 标量且不含控制字符"
            }
            (Locale::EnUs, ExpectedValue::MetadataStateLabel) => {
                "STATE=TEXT with nonempty text; state is idle, working, blocked, done, error or unknown"
            }
            (Locale::ZhCn, ExpectedValue::MetadataStateLabel) => {
                "状态=非空文本；状态为 idle、working、blocked、done、error 或 unknown"
            }
            (Locale::EnUs, ExpectedValue::MetadataPresentation) => {
                "at least one metadata change, without setting and clearing the same field"
            }
            (Locale::ZhCn, ExpectedValue::MetadataPresentation) => {
                "至少一个元数据变更，且不能同时设置和清除同一字段"
            }
            (Locale::EnUs, ExpectedValue::MetadataSource) => {
                "1–80 ASCII letters, digits, colon, dot, underscore or hyphen"
            }
            (Locale::ZhCn, ExpectedValue::MetadataSource) => {
                "1–80 个 ASCII 字母、数字、冒号、点、下划线或连字符"
            }
            (Locale::EnUs, ExpectedValue::MetadataToken) => {
                "a 1–32 character ASCII token key (letters/digits/_/-), with =VALUE for --token"
            }
            (Locale::ZhCn, ExpectedValue::MetadataToken) => {
                "1–32 个 ASCII 字母／数字／下划线／连字符组成的键；--token 需使用 键=值"
            }
            (Locale::EnUs, ExpectedValue::MetadataPatchSize) => {
                "at most 16 distinct token keys per report"
            }
            (Locale::ZhCn, ExpectedValue::MetadataPatchSize) => "每次上报至多 16 个不同 token 键",
            (Locale::EnUs, ExpectedValue::MetadataSequence) => {
                "an unsigned 64-bit decimal sequence"
            }
            (Locale::ZhCn, ExpectedValue::MetadataSequence) => "无符号 64 位十进制序号",
            (Locale::EnUs, ExpectedValue::MetadataTtl) => "a TTL from 1 to 86400000 milliseconds",
            (Locale::ZhCn, ExpectedValue::MetadataTtl) => "1–86400000 毫秒的 TTL",

            (Locale::EnUs, ExpectedValue::OutputFilePath) => {
                "a literal file path of 1–4096 UTF-8 bytes"
            }
            (Locale::ZhCn, ExpectedValue::OutputFilePath) => "1–4096 UTF-8 字节的字面文件路径",
            (Locale::EnUs, ExpectedValue::PresentationId) => {
                "a nonzero 32-character lowercase hexadecimal Presentation ID"
            }
            (Locale::ZhCn, ExpectedValue::PresentationId) => {
                "非零的 32 位小写十六进制 Presentation 标识"
            }
            (Locale::EnUs, ExpectedValue::ClientId) => {
                "a nonzero 32-character lowercase hexadecimal Client ID"
            }
            (Locale::ZhCn, ExpectedValue::ClientId) => "非零的 32 位小写十六进制客户端标识",
            (Locale::EnUs, ExpectedValue::ClientOperationId) => {
                "a nonzero 32-character lowercase hexadecimal Client control operation ID"
            }
            (Locale::ZhCn, ExpectedValue::ClientOperationId) => {
                "非零的 32 位小写十六进制客户端控制操作标识"
            }
            (Locale::EnUs, ExpectedValue::WorktreeReference) => {
                "a nonempty Git reference of at most 1024 UTF-8 bytes without a leading dash or control characters"
            }
            (Locale::ZhCn, ExpectedValue::WorktreeReference) => {
                "至多 1024 UTF-8 字节、无前导短横线或控制字符的非空 Git 引用"
            }
            (Locale::EnUs, ExpectedValue::WorktreeOperationId) => {
                "a nonzero 32-character lowercase hexadecimal operation ID"
            }
            (Locale::ZhCn, ExpectedValue::WorktreeOperationId) => {
                "非零的 32 位小写十六进制操作标识"
            }
            (Locale::EnUs, ExpectedValue::WorktreeAttempt) => {
                "a decimal attempt number from 1 to 64"
            }
            (Locale::ZhCn, ExpectedValue::WorktreeAttempt) => "1–64 的十进制尝试序号",
            (Locale::EnUs, ExpectedValue::ServerStopRequestId) => {
                "a nonzero 32-character lowercase hexadecimal stop request ID"
            }
            (Locale::ZhCn, ExpectedValue::ServerStopRequestId) => {
                "非零的 32 位小写十六进制停止请求标识"
            }
            (Locale::EnUs, ExpectedValue::Format) => "'text' or 'json'",
            (Locale::ZhCn, ExpectedValue::Format) => "'text' 或 'json'",
            (Locale::EnUs, ExpectedValue::RightClick) => "'orchbee' or 'panel'",
            (Locale::ZhCn, ExpectedValue::RightClick) => "'orchbee' 或 'panel'",
            (Locale::EnUs, ExpectedValue::PanelLabel) => {
                "at most 512 Unicode characters; empty text clears the title"
            }
            (Locale::ZhCn, ExpectedValue::PanelLabel) => "最多 512 个 Unicode 字符；空文本清除标题",
            (Locale::EnUs, ExpectedValue::Label) => "a nonempty label of at most 512 characters",
            (Locale::ZhCn, ExpectedValue::Label) => "1–512 个字符的非空标签",
            (Locale::EnUs, ExpectedValue::LayoutSavedAt) => {
                "a save time listed by orchbee layout history"
            }
            (Locale::ZhCn, ExpectedValue::LayoutSavedAt) => "orchbee layout history 列出的保存时间",
            (Locale::EnUs, ExpectedValue::Dimension) => "a decimal integer from 0 to 65535",
            (Locale::ZhCn, ExpectedValue::Dimension) => "0–65535 的十进制整数",
            (Locale::EnUs, ExpectedValue::WorkspaceInsertIndex) => {
                "an integer insertion boundary from 0 to 1023"
            }
            (Locale::ZhCn, ExpectedValue::WorkspaceInsertIndex) => "0 到 1023 的整数插入边界",
            (Locale::EnUs, ExpectedValue::WorkspaceBlockSize) => "1 to 1023 Workspace targets",
            (Locale::ZhCn, ExpectedValue::WorkspaceBlockSize) => "1 到 1023 个工作区目标",
            (Locale::EnUs, ExpectedValue::TabInsertIndex) => {
                "an integer from 0 to 256 (boundary in the original Tab order)"
            }
            (Locale::ZhCn, ExpectedValue::TabInsertIndex) => {
                "0 到 256 的整数（原始标签页顺序中的插入边界）"
            }
            (Locale::EnUs, ExpectedValue::TabDirection) => "'previous', 'next', 'first', or 'last'",
            (Locale::ZhCn, ExpectedValue::TabDirection) => "'previous'、'next'、'first' 或 'last'",
            (Locale::EnUs, ExpectedValue::FiniteNumber) => "a finite number",
            (Locale::ZhCn, ExpectedValue::FiniteNumber) => "有限数值",
            (Locale::EnUs, ExpectedValue::SplitPath) => {
                "'root' or a dot-separated binary path of at most 16 steps (for example 0.1)"
            }
            (Locale::ZhCn, ExpectedValue::SplitPath) => {
                "'root' 或至多 16 步的点分二元路径（例如 0.1）"
            }
            (Locale::EnUs, ExpectedValue::LayoutDirection) => "'left', 'right', 'up', or 'down'",
            (Locale::ZhCn, ExpectedValue::LayoutDirection) => "'left'、'right'、'up' 或 'down'",
            (Locale::EnUs, ExpectedValue::PanelDirection) => "'left', 'right', 'up', or 'down'",
            (Locale::ZhCn, ExpectedValue::PanelDirection) => "'left'、'right'、'up' 或 'down'",
            (Locale::EnUs, ExpectedValue::Shell) => {
                "'bash', 'zsh', 'fish', 'powershell', or 'elvish'"
            }
            (Locale::ZhCn, ExpectedValue::Shell) => {
                "'bash'、'zsh'、'fish'、'powershell' 或 'elvish'"
            }
            (Locale::EnUs, ExpectedValue::DirectoryPath) => {
                "a literal absolute path of at most 4,096 UTF-8 bytes"
            }
            (Locale::ZhCn, ExpectedValue::DirectoryPath) => "不超过 4,096 UTF-8 字节的字面绝对路径",
            (Locale::EnUs, ExpectedValue::WorktreePath) => {
                "an absolute path of at most 4,096 UTF-8 bytes; Windows also accepts relative paths and ~"
            }
            (Locale::ZhCn, ExpectedValue::WorktreePath) => {
                "不超过 4,096 UTF-8 字节的绝对路径；Windows 也接受相对路径及 ~"
            }
            (Locale::EnUs, ExpectedValue::MatchText) => {
                "nonempty literal text of at most 4096 bytes"
            }
            (Locale::ZhCn, ExpectedValue::MatchText) => "至多 4096 字节的非空字面文本",
            (Locale::EnUs, ExpectedValue::RegexPattern) => {
                "a nonempty regular expression of at most 1024 bytes"
            }
            (Locale::ZhCn, ExpectedValue::RegexPattern) => "至多 1024 字节的非空正则表达式",
            (Locale::EnUs, ExpectedValue::OutputSource) => {
                "visible, recent, recent-unwrapped, or detection"
            }
            (Locale::ZhCn, ExpectedValue::OutputSource) => {
                "visible、recent、recent-unwrapped 或 detection"
            }
            (Locale::EnUs, ExpectedValue::OutputLines) => {
                "an unsigned decimal integer from 0 to 4294967295 (clamped to 1000)"
            }
            (Locale::ZhCn, ExpectedValue::OutputLines) => {
                "0–4294967295 的无符号十进制整数（钳制到 1000）"
            }
            (Locale::EnUs, ExpectedValue::AgentStartTimeout) => {
                "a decimal integer from 3001 to 300000"
            }
            (Locale::ZhCn, ExpectedValue::AgentStartTimeout) => "3001–300000 的十进制整数",
            (_, ExpectedValue::AgentStartProvider) => self.cli_agent_start_provider_choices(),
            (Locale::EnUs, ExpectedValue::AgentStartArgument) => {
                "an argument without control characters"
            }
            (Locale::ZhCn, ExpectedValue::AgentStartArgument) => "不含控制字符的参数",
            (Locale::EnUs, ExpectedValue::WaitTimeout) => "a decimal integer from 1 to 3600000",
            (Locale::EnUs, ExpectedValue::OutputCursor) => {
                "now or an output cursor returned by this terminal"
            }
            (Locale::ZhCn, ExpectedValue::OutputCursor) => "now 或此终端返回的输出游标",
            (Locale::ZhCn, ExpectedValue::WaitTimeout) => "1–3600000 的十进制整数",
            (Locale::EnUs, ExpectedValue::SendText) => {
                "nonempty text of at most 4096 bytes including the '--enter' CR"
            }
            (Locale::ZhCn, ExpectedValue::SendText) => {
                "非空文本，计入 '--enter' 的回车后不超过 4096 字节"
            }
            (Locale::EnUs, ExpectedValue::SendKey) => "one of the accepted '--key' names",
            (Locale::ZhCn, ExpectedValue::SendKey) => "'--key' 接受的按键名之一",
            (Locale::EnUs, ExpectedValue::MachineId) => {
                "a nonzero 32-character lowercase hexadecimal machine profile ID"
            }
            (Locale::ZhCn, ExpectedValue::MachineId) => "非零、32 字符的小写十六进制机器配置 ID",
            (Locale::EnUs, ExpectedValue::MachineLabel) => {
                "a nonblank label of at most 128 UTF-8 bytes without control characters"
            }
            (Locale::ZhCn, ExpectedValue::MachineLabel) => {
                "非空白、不含控制字符且最多 128 UTF-8 字节的标签"
            }
            (Locale::EnUs, ExpectedValue::SshTarget) => {
                "an SSH alias, [user@]host, or ssh://[user@]host[:port] without a password"
            }
            (Locale::ZhCn, ExpectedValue::SshTarget) => {
                "不含密码的 SSH 别名、[user@]host 或 ssh://[user@]host[:port]"
            }
            (Locale::EnUs, ExpectedValue::SessionName) => {
                "1–64 ASCII letters, digits, '.', '_' or '-'; excluding '.' and '..'"
            }
            (Locale::ZhCn, ExpectedValue::SessionName) => {
                "1–64 个 ASCII 字母、数字、'.'、'_' 或 '-'；排除 '.' 和 '..'"
            }
            (Locale::EnUs, ExpectedValue::MouseMode) => "'off' or 'on'",
            (Locale::ZhCn, ExpectedValue::MouseMode) => "'off' 或 'on'",
            (Locale::EnUs, ExpectedValue::DirectoryId) => {
                "a nonzero 32-character lowercase hexadecimal directory ID"
            }
            (Locale::ZhCn, ExpectedValue::DirectoryId) => "非零的 32 位小写十六进制目录 ID",
            (Locale::EnUs, ExpectedValue::WorkspaceId) => {
                "a nonzero 32-character lowercase hexadecimal workspace ID"
            }
            (Locale::ZhCn, ExpectedValue::WorkspaceId) => "非零的 32 位小写十六进制 workspace ID",
            (Locale::EnUs, ExpectedValue::ManifestOperationId) => {
                "a nonzero 32-character lowercase hexadecimal operation ID"
            }
            (Locale::ZhCn, ExpectedValue::ManifestOperationId) => "非零的 32 位小写十六进制操作 ID",
            (Locale::EnUs, ExpectedValue::CloseRequestId) => {
                "the close operation's own nonzero 32-character lowercase hexadecimal request ID"
            }
            (Locale::ZhCn, ExpectedValue::CloseRequestId) => {
                "该 close 操作自身的非零 32 位小写十六进制 request ID"
            }
            (Locale::EnUs, ExpectedValue::CloseGeneration) => {
                "a nonzero decimal attempt generation without a leading zero"
            }
            (Locale::ZhCn, ExpectedValue::CloseGeneration) => {
                "无前导零的非零十进制 attempt generation"
            }
            (Locale::EnUs, ExpectedValue::ExecutableName) => {
                "an absolute executable path or one filename to resolve on PATH"
            }
            (Locale::ZhCn, ExpectedValue::ExecutableName) => {
                "绝对可执行路径或在 PATH 中查找的单个文件名"
            }
            (Locale::EnUs, ExpectedValue::ExecutableOnPath) => {
                "an executable present in an absolute PATH directory (Windows: .exe or .com)"
            }
            (Locale::ZhCn, ExpectedValue::ExecutableOnPath) => {
                "绝对 PATH 目录中存在的可执行文件（Windows：.exe 或 .com）"
            }
            (Locale::EnUs, ExpectedValue::Executable) => "a nonempty absolute executable path",
            (Locale::ZhCn, ExpectedValue::Executable) => "非空的绝对可执行文件路径",
            (Locale::EnUs, ExpectedValue::LaunchEnvironment) => {
                "NAME=VALUE; names use ASCII letters, digits and underscores (1..128 bytes, no leading digit); at most 32 entries and 1,024 UTF-8 bytes total; no NUL, case-insensitive duplicates, ORCHBEE_*, TERM, COLORTERM or TERM_PROGRAM"
            }
            (Locale::ZhCn, ExpectedValue::LaunchEnvironment) => {
                "NAME=VALUE；名称使用 ASCII 字母、数字和下划线（1..128 字节，不能以数字开头）；至多 32 项且合计不超过 1,024 UTF-8 字节；不能包含 NUL、忽略大小写后重复的名称、ORCHBEE_*、TERM、COLORTERM 或 TERM_PROGRAM"
            }
            (Locale::EnUs, ExpectedValue::LaunchArguments) => {
                "at most 128 arguments totaling at most 2,048 UTF-8 bytes with '--exec'"
            }
            (Locale::ZhCn, ExpectedValue::LaunchArguments) => {
                "配合 '--exec' 使用，至多 128 个参数且合计不超过 2,048 UTF-8 字节"
            }
            (Locale::EnUs, ExpectedValue::AgentName) => "a name matching [a-z][a-z0-9_-]{0,31}",
            (Locale::ZhCn, ExpectedValue::AgentName) => "符合 [a-z][a-z0-9_-]{0,31} 的名称",
            (Locale::EnUs, ExpectedValue::AgentSessionId) => {
                "an agent session identity (1–128 ASCII letters, digits, '.', '_', ':', '-')"
            }
            (Locale::ZhCn, ExpectedValue::AgentSessionId) => {
                "一个智能体会话身份（1–128 个 ASCII 字母、数字、'.'、'_'、':'、'-'）"
            }
            (Locale::EnUs, ExpectedValue::AgentTarget) => {
                "an Agent session ID or name, a Panel ID, or @panel:0x<hex>; for a terminal, pass the Panel that shows it"
            }
            (Locale::ZhCn, ExpectedValue::AgentTarget) => {
                "智能体会话 ID 或名称、面板 ID 或 @panel:0x<hex>；如要指定终端，请传入显示它的面板"
            }
            (Locale::EnUs, ExpectedValue::AgentListLimit) => "a decimal integer from 1 to 100",
            (Locale::ZhCn, ExpectedValue::AgentListLimit) => "1–100 的十进制整数",
            (Locale::EnUs, ExpectedValue::AgentCursor) => {
                "the unchanged cursor returned by the same command and backend"
            }
            (Locale::ZhCn, ExpectedValue::AgentCursor) => "同一命令和后端返回的原样游标",
            (Locale::EnUs, ExpectedValue::AgentViewSpec) => {
                "a bounded Agent view JSON specification"
            }
            (Locale::ZhCn, ExpectedValue::AgentViewSpec) => "有界的智能体视图 JSON 规格",
            (Locale::EnUs, ExpectedValue::AgentViewSource) => {
                "a source of 1–120 ASCII letters, digits or :._-"
            }
            (Locale::ZhCn, ExpectedValue::AgentViewSource) => {
                "1–120 个 ASCII 字母数字或 :._- 字符的来源"
            }
            (Locale::EnUs, ExpectedValue::AgentWaitUntil) => {
                "one of the fourteen 'agent wait --until' conditions"
            }
            (Locale::ZhCn, ExpectedValue::AgentWaitUntil) => {
                "'agent wait --until' 的十四个条件名之一"
            }
            (Locale::EnUs, ExpectedValue::AgentAfterRevision) => {
                "a decimal integer from 0 to 9007199254740991"
            }
            (Locale::ZhCn, ExpectedValue::AgentAfterRevision) => "0–9007199254740991 的十进制整数",
            (Locale::EnUs, ExpectedValue::ProviderId) => "one of the canonical provider ids",
            (Locale::ZhCn, ExpectedValue::ProviderId) => "规范 provider ID 之一",
            (Locale::EnUs, ExpectedValue::AssetRootPath) => {
                "a nonempty absolute path; at most 4 may be given"
            }
            (Locale::ZhCn, ExpectedValue::AssetRootPath) => "非空绝对路径；至多可给出 4 个",
            (Locale::EnUs, ExpectedValue::ProfileName) => {
                "a nonempty launch profile name of at most 512 UTF-8 bytes"
            }
            (Locale::ZhCn, ExpectedValue::ProfileName) => "不超过 512 UTF-8 字节的非空启动配置名称",
            (Locale::EnUs, ExpectedValue::ProfileSelector) => {
                "an exact profile ID or singly-matching name of at most 512 UTF-8 bytes"
            }
            (Locale::ZhCn, ExpectedValue::ProfileSelector) => {
                "精确 profile ID 或唯一匹配的名称（不超过 512 UTF-8 字节）"
            }
            (Locale::EnUs, ExpectedValue::Provider) => {
                "one of the supported provider ids ('orchbee agent integration list' names them)"
            }
            (Locale::ZhCn, ExpectedValue::Provider) => {
                "受支持的 provider ID 之一（'orchbee agent integration list' 列出全部）"
            }
            (Locale::EnUs, ExpectedValue::AdapterSource) => {
                "a literal absolute adapter-source directory path"
            }
            (Locale::ZhCn, ExpectedValue::AdapterSource) => "一个字面绝对 adapter-source 目录路径",
            (Locale::EnUs, ExpectedValue::CommandText) => {
                "one non-empty UTF-8 command string within the 256 KiB paste and 64 KiB argv limits"
            }
            (Locale::ZhCn, ExpectedValue::CommandText) => {
                "一个非空 UTF-8 命令字符串，受 256 KiB paste 和 64 KiB argv 限制"
            }
            (Locale::EnUs, ExpectedValue::PromptText) => {
                "non-empty UTF-8 prompt within the 256 KiB paste and 64 KiB argv limits"
            }
            (Locale::ZhCn, ExpectedValue::PromptText) => {
                "非空 UTF-8 提示，受 256 KiB paste 和 64 KiB argv 限制"
            }
            (Locale::EnUs, ExpectedValue::SendKeySequence) => {
                "a key name, f1..f25, space or one character, optionally ctrl/alt/shift/super; at most 64"
            }
            (Locale::ZhCn, ExpectedValue::SendKeySequence) => {
                "键名、f1..f25、space 或单个字符，可组合 ctrl/alt/shift/super；至多 64 个"
            }
        };
        let (problem, hint) = match reason {
            UsageReason::Invalid => return self.locale.usage_failure().to_owned(),
            UsageReason::ArgumentEnvelope => (
                match self.locale {
                    Locale::EnUs => "Invalid invocation: the argument list must be Unicode and its total size may not exceed 64 KiB.".to_owned(),
                    Locale::ZhCn => "调用方式无效：参数必须为 Unicode，且总大小不得超过 64 KiB。".to_owned(),
                },
                hint(UsageContext::Root),
            ),
            UsageReason::RelayRequiresPipes => (
                match self.locale {
                    Locale::EnUs => "'orchbee server connect' carries protocol bytes for another program, so its standard input and output must be pipes, not a terminal.".to_owned(),
                    Locale::ZhCn => "'orchbee server connect' 为其他程序传输协议字节，因此标准输入和输出必须是管道，不能是终端。".to_owned(),
                },
                hint(UsageContext::Path("server connect")),
            ),
            UsageReason::UnknownCommand {
                context: ctx,
                word,
            } => (
                match (self.locale, ctx) {
                    (Locale::EnUs, UsageContext::Root) => {
                        format!("Unknown command {}.", echo(word))
                    }
                    (Locale::EnUs, _) => {
                        format!("Unknown command {} for '{}'.", echo(word), context(*ctx))
                    }
                    (Locale::ZhCn, UsageContext::Root) => format!("未知命令 {}。", echo(word)),
                    (Locale::ZhCn, _) => {
                        format!("'{}' 中未知命令 {}。", context(*ctx), echo(word))
                    }
                },
                hint(*ctx),
            ),
            UsageReason::UnknownOption { context, option } => (
                match self.locale {
                    Locale::EnUs if (option == "--machine" || option.starts_with("--machine=")) => "The --machine routing option must be first: orchbee --machine <target> <command>.".to_owned(),
                    Locale::ZhCn if (option == "--machine" || option.starts_with("--machine=")) => "--machine 路由选项必须放在第一个位置：orchbee --machine <target> <command>。".to_owned(),
                    Locale::EnUs => format!("Unknown option {}.", echo(option)),
                    Locale::ZhCn => format!("未知选项 {}。", echo(option)),
                },
                hint(*context),
            ),
            UsageReason::UnexpectedOption {
                context: ctx,
                option,
            } => (
                match self.locale {
                    Locale::EnUs => {
                        format!("'{option}' is not a valid option for '{}'.", context(*ctx))
                    }
                    Locale::ZhCn => {
                        format!("'{option}' 不是 '{}' 的有效选项。", context(*ctx))
                    }
                },
                hint(*ctx),
            ),
            UsageReason::DuplicateOption { option } => (
                match self.locale {
                    Locale::EnUs => format!("Option '{option}' was given more than once."),
                    Locale::ZhCn => format!("选项 '{option}' 被重复提供。"),
                },
                hint(UsageContext::Root),
            ),
            UsageReason::ConflictingOptions {
                context: ctx,
                first,
                second,
            } => (
                match self.locale {
                    Locale::EnUs => {
                        format!("Options '{first}' and '{second}' cannot be combined.")
                    }
                    Locale::ZhCn => format!("选项 '{first}' 和 '{second}' 不能组合使用。"),
                },
                hint(*ctx),
            ),
            UsageReason::MissingValue { option } => (
                match self.locale {
                    Locale::EnUs => format!("Option '{option}' requires a value."),
                    Locale::ZhCn => format!("选项 '{option}' 需要一个值。"),
                },
                hint(UsageContext::Root),
            ),
            UsageReason::MissingRequired {
                context: ctx,
                requirement,
            } => (
                match self.locale {
                    Locale::EnUs => format!("'{}' requires {requirement}.", context(*ctx)),
                    Locale::ZhCn => format!("'{}' 需要 {requirement}。", context(*ctx)),
                },
                hint(*ctx),
            ),
            UsageReason::CallerContextUnavailable => (
                match self.locale {
                    Locale::EnUs => "Calling terminal context is missing or invalid. Run inside a newly launched Orchbee terminal, or use 'panel current --panel <panel>'.".to_owned(),
                    Locale::ZhCn => "调用终端的上下文缺失或无效。请在新启动的 Orchbee 终端内运行，或使用 'panel current --panel <panel>'。".to_owned(),
                },
                hint(UsageContext::Path("panel current")),
            ),
            UsageReason::UnexpectedArgument {
                context: ctx,
                value,
            } => (
                match self.locale {
                    Locale::EnUs => format!(
                        "'{}' does not take the argument {}.",
                        context(*ctx),
                        echo(value)
                    ),
                    Locale::ZhCn => {
                        format!("'{}' 不接受参数 {}。", context(*ctx), echo(value))
                    }
                },
                hint(*ctx),
            ),
            UsageReason::MissingCommand { context: ctx } => (
                match (self.locale, ctx) {
                    (Locale::EnUs, UsageContext::Root) => "Missing command.".to_owned(),
                    (Locale::EnUs, _) => format!("Missing command after '{}'.", context(*ctx)),
                    (Locale::ZhCn, UsageContext::Root) => "缺少命令。".to_owned(),
                    (Locale::ZhCn, _) => format!("'{}' 后缺少命令。", context(*ctx)),
                },
                hint(*ctx),
            ),
            UsageReason::UnknownHelpTopic { path } => (
                match self.locale {
                    Locale::EnUs => format!("No help topic matches {}.", echo(path)),
                    Locale::ZhCn => format!("没有与 {} 匹配的帮助主题。", echo(path)),
                },
                hint(UsageContext::Root),
            ),
            UsageReason::MisplacedFlag {
                context: ctx,
                flag,
            } => (
                match self.locale {
                    Locale::EnUs => format!("'{flag}' is not valid in this position."),
                    Locale::ZhCn => format!("'{flag}' 在此位置无效。"),
                },
                hint(*ctx),
            ),
            UsageReason::InvalidValue {
                context: ctx, option, expected: ExpectedValue::LaunchEnvironment, ..
            } => (
                match self.locale {
                    Locale::EnUs => format!("Invalid '{option}' (expected {}).", expected(ExpectedValue::LaunchEnvironment)),
                    Locale::ZhCn => format!("'{option}' 无效（应为 {}）。", expected(ExpectedValue::LaunchEnvironment)),
                },
                hint(*ctx),
            ),
            UsageReason::InvalidValue {
                context: ctx,
                option,
                value,
                expected: shape,
            } => (
                match self.locale {
                    Locale::EnUs => format!(
                        "Invalid value {} for '{option}' (expected {}).",
                        echo(value),
                        expected(*shape)
                    ),
                    Locale::ZhCn => format!(
                        "'{option}' 的值 {} 无效（应为 {}）。",
                        echo(value),
                        expected(*shape)
                    ),
                },
                hint(*ctx),
            ),
            UsageReason::SendSource {
                option,
                value,
                cause,
            } => {
                let named = value
                    .as_deref()
                    .map(|path| format!(" {}", echo(path)))
                    .unwrap_or_default();
                (
                    match (self.locale, cause) {
                        (Locale::EnUs, SendSourceCause::NotRegularFile) => {
                            format!("The '{option}' source{named} is not a regular file.")
                        }
                        (Locale::EnUs, SendSourceCause::InteractiveTerminal) => {
                            format!(
                                "The '{option}' source is a terminal; pipe or redirect input instead."
                            )
                        }
                        (Locale::EnUs, SendSourceCause::UnsupportedStream) => {
                            format!("The '{option}' source is not a readable file or pipe.")
                        }
                        (Locale::EnUs, SendSourceCause::Empty) => {
                            format!("The '{option}' source{named} is empty.")
                        }
                        (Locale::EnUs, SendSourceCause::Oversize { limit }) => {
                            format!(
                                "The '{option}' source{named} exceeds the {limit}-byte input bound."
                            )
                        }
                        (Locale::EnUs, SendSourceCause::Unreadable) => {
                            format!("The '{option}' source{named} could not be read.")
                        }
                        (Locale::ZhCn, SendSourceCause::NotRegularFile) => {
                            format!("'{option}' 源{named}不是常规文件。")
                        }
                        (Locale::ZhCn, SendSourceCause::InteractiveTerminal) => {
                            format!("'{option}' 源是终端；请改用管道或重定向输入。")
                        }
                        (Locale::ZhCn, SendSourceCause::UnsupportedStream) => {
                            format!("'{option}' 源不是可读的文件或管道。")
                        }
                        (Locale::ZhCn, SendSourceCause::Empty) => {
                            format!("'{option}' 源{named}为空。")
                        }
                        (Locale::ZhCn, SendSourceCause::Oversize { limit }) => {
                            format!("'{option}' 源{named}超过 {limit} 字节输入上限。")
                        }
                        (Locale::ZhCn, SendSourceCause::Unreadable) => {
                            format!("无法读取 '{option}' 源{named}。")
                        }
                    },
                    hint(UsageContext::Path("terminal send")),
                )
            }
            UsageReason::InvalidGeometry {
                context: ctx,
                cols,
                rows,
                cell_width,
                cell_height,
            } => (
                match self.locale {
                    Locale::EnUs => format!(
                        "Geometry cols={cols} rows={rows} cell-width={cell_width} cell-height={cell_height} does not satisfy the protocol bounds (columns/rows 1-4096, cell pixel dimensions 0-4096 both zero or both nonzero, cells ≤262144, pixel extents ≤65535)."
                    ),
                    Locale::ZhCn => format!(
                        "几何参数 cols={cols} rows={rows} cell-width={cell_width} cell-height={cell_height} 不满足协议约束（columns/rows 1-4096、像素维 0-4096 且须同零或同非零、格子总数 ≤262144、像素边长 ≤65535）。"
                    ),
                },
                hint(*ctx),
            ),
        };
        let suggestions = advice::suggestions(reason);
        let advice = if suggestions.is_empty() {
            String::new()
        } else {
            let candidates = suggestions
                .iter()
                .map(|candidate| format!("\"{candidate}\""))
                .collect::<Vec<_>>()
                .join(", ");
            match self.locale {
                Locale::EnUs => format!(" Did you mean {candidates}?"),
                Locale::ZhCn => format!("是否要输入 {candidates}？"),
            }
        };
        let mut diagnostic = match self.locale {
            Locale::EnUs => format!("{problem}{advice} Run '{hint}'.\n"),
            Locale::ZhCn => format!("{problem}{advice}请运行 ‘{hint}’。\n"),
        };
        if let UsageReason::MissingCommand { context } = reason {
            let children = advice::children(*context);
            if !children.is_empty() {
                diagnostic.push_str(self.cli_help_text(crate::cli_text::HelpText::Commands));
                diagnostic.push('\n');
                for (name, topic) in children {
                    diagnostic.push_str(&format!("  {name}  {}\n", self.cli_help_summary(topic)));
                }
            }
        }
        diagnostic
    }
}
