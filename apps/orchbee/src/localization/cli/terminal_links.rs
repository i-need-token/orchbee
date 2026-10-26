//! Localized safe link receipts and public immutable viewport help.
use crate::localization::Locale;
#[cfg(windows)]
use crate::{terminal_buffer_text::TerminalTextPoint, terminal_links::ResolvedLink};
impl Locale {
    #[cfg(windows)]
    pub(crate) fn cli_terminal_link_activation(
        self,
        state: &str,
        operation: Option<&str>,
    ) -> String {
        let label = match (self, state) {
            (Self::EnUs, "no-link") => "No link to activate",
            (Self::EnUs, "no-handler") => "No matching plugin handler",
            (Self::EnUs, "started") => "Plugin Job admitted",
            (Self::EnUs, "failed") => "Plugin invocation refused",
            (Self::EnUs, _) => "Plugin outcome unknown; do not retry",
            (Self::ZhCn, "no-link") => "没有可激活的链接",
            (Self::ZhCn, "no-handler") => "没有匹配的插件处理器",
            (Self::ZhCn, "started") => "插件 Job 已准入",
            (Self::ZhCn, "failed") => "插件调用被拒绝",
            (Self::ZhCn, _) => "插件结果未知；不要重试",
        };
        let mut text = format!("{label}\n");
        if let Some(operation) = operation {
            text.push_str(&format!(
                "{}: {}\n",
                self.plugin_runtime_state("operation-id"),
                crate::cli::escape::escape(operation)
            ));
        }
        text
    }
    #[cfg(windows)]
    pub(crate) fn cli_terminal_link(
        self,
        terminal: &str,
        revision: u64,
        cursor: TerminalTextPoint,
        offset: u64,
        kind: &str,
        resolved: &ResolvedLink,
    ) -> String {
        let url = resolved.url.as_deref().map(crate::cli::escape::quoted);
        let label = match (self, kind) {
            (Self::ZhCn, "none") => "无",
            (Self::ZhCn, "plain-text") => "纯文本",
            (Self::ZhCn, "explicit") => "显式超链接",
            _ => kind,
        };
        let mut text = match self {
            Self::EnUs => format!(
                "Terminal {terminal}, content revision {revision}\nViewport: {}:{}, offset {offset}\nLink ({label}): {}\n",
                cursor.row,
                cursor.col,
                url.as_deref().unwrap_or("none")
            ),
            Self::ZhCn => format!(
                "终端 {terminal}，内容修订 {revision}\n视口：{}:{}，偏移 {offset}\n链接（{label}）：{}\n",
                cursor.row,
                cursor.col,
                url.as_deref().unwrap_or("无")
            ),
        };
        for region in &resolved.regions {
            text.push_str(&format!(
                "  {}:{}-{}\n",
                region.row, region.start_col, region.end_col
            ));
        }
        text
    }
}
pub(super) const HELP_EN: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee terminal link-resolve <terminal> --cursor ROW:COL [--offset-from-bottom N] [--content-revision N] [--generation N] [--format text|json]",
    "Windows: resolve one whole cell in an explicit read-only native viewport.\nZero-based row/column are u16; offset is u64, defaults0 and cannot exceed captured\nscrollback. The viewport starts at scrollback-offset, independent of TUI focus\nor scroll. Optional revision is full-u64; generation is positive full-u64.\nReads one render row for an OSC8 URI (empty regions), then mapped text as needed\nfor a lowercase http(s) whitespace token with punctuation/balanced closer rules.\nWide/combining glyphs and verified soft wraps are preserved, including invisible\ntoken ends. Shared8192-cell inspection includes spacers/tails and boundary probes;\nexhaustion yields no link. Visible inclusive regions\nare clipped to the requested viewport. URI/URL is returned without opening it\nor invoking a plugin. No controller/input/attention/selection/clipboard effects.\nOne existing-session viewer, five-second cancellable total, strict route fences;\n65536 scanned rows/aggregate8MiB encoded bytes/1048576 stored glyphs. Stale,\nevicted, malformed or over-limit data fails whole and releases; no retry.\nJSON: orchbee.terminal.link-resolve v1. Text escapes controls.",
);
pub(super) const HELP_ZH: crate::localization::HelpDetails = crate::localization::HelpDetails::new(
    "orchbee terminal link-resolve <terminal> --cursor ROW:COL [--offset-from-bottom N] [--content-revision N] [--generation N] [--format text|json]",
    "Windows：解析显式只读原生视口中的完整单元格。零起点行／列为 u16；offset\n为 u64，默认零，不得超出捕获的 scrollback。视口始于 scrollback-offset，独立于 TUI 焦点／滚动。可选完整 u64 修订，generation 为正完整 u64。先读一行 render 检查 OSC8 URI（空 regions），再按需读取映射文本，识别小写\nhttp(s) 空白分隔 token，并处理尾部标点／平衡闭括号。保留宽字／组合字和已验证软换行，包括不可见 token 端点。共享 8192 格探查计算宽字占位／尾格及边界探查，预算耗尽返回无链接。包含端点可见区域裁剪到请求视口。只返回 URI／URL，不打开，也不调用插件；不取得 controller，不改输入／attention／选区／剪贴板。一次既有会话 viewer，五秒可取消总期限，严格路由围栏；65536 扫描行、合计 8 MiB 编码字节、1048576\n存储字形。过期／回收／格式错误／超限整体失败并释放，不重试。\nJSON：orchbee.terminal.link-resolve v1；文本转义控制字符。",
);

pub(super) const ACTIVATE_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal link-activate <terminal> --cursor ROW:COL [--offset-from-bottom N] [--content-revision N] [--generation N] [--format text|json]",
        "Windows: resolve the exact whole cell, then invoke a matching enabled plugin handler.\nSame explicit native viewport/u16 cursor/u64 offset(default0)/revision/generation\nand OSC8/plain URL rules as link-resolve. Ordered by plugin ID then manifest handler\norder; handler and action must support Windows. No match is handled=false.\nMatched URL must fit4096 UTF-8 bytes and existing bounded plugin display context;\nexact clicked primary/child Panel context, never an unrelated Client focus.\nOne existing5s viewer; strict final content and catalog preflight before one invoke.\nServer resolves argv/plugin-root/protected environment; Worker owns Job/logs.\nNo browser/input/navigation/clipboard/session start. Always Detach/EOF after reads.\nJSON: orchbee.terminal.link-activate v1. handled=true means known Job admission,\nnot command success. Refusal exits3; lost/unknown outcome exits8 with operation ID;\nnever retry unknown delivery. Ctrl+C exits130 without cancelling an admitted Job.",
    );
pub(super) const ACTIVATE_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal link-activate <terminal> --cursor ROW:COL [--offset-from-bottom N] [--content-revision N] [--generation N] [--format text|json]",
        "Windows：解析精确完整单元格，再调用匹配的已启用插件处理器。与 link-resolve 相同的显式原生 viewport／u16 光标／u64 offset（默认零）／修订／generation 和 OSC8／普通 URL 规则。按插件 ID 和 manifest 处理器顺序，处理器与动作都须支持 Windows；无匹配 handled=false。匹配 URL 至多4096 UTF-8\n字节，遵循既有插件显示上下文限制；上下文来自精确点击主／子终端面板，不是无关客户端焦点。一个既有五秒 viewer，单次 invoke 前最终内容／清单预检。\nServer 解析 argv／插件根／保护环境，Worker 拥有 Job／日志；不打开浏览器、不输入／导航／改剪贴板／启动会话，读取后总是 Detach／EOF 释放。\nJSON：orchbee.terminal.link-activate v1。handled=true 表示已知 Job 准入，不代表命令成功；拒绝退出3，丢失／未知结果退出8并保留操作ID，不重试未知投递。\nCtrl+C 退出130，不取消已准入的 Job。",
    );
