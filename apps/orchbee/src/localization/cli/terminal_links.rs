//! Localized safe link receipts and public immutable viewport help.
use crate::{
    localization::Locale, terminal_buffer_text::TerminalTextPoint, terminal_links::ResolvedLink,
};
impl Locale {
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
pub(super) const HELP_EN: &str = r#"Usage: orchbee terminal link-resolve <terminal> --cursor ROW:COL [--offset-from-bottom N] [--content-revision N] [--generation N] [--format text|json]

Windows: resolve one whole cell in an explicit read-only native viewport.
Zero-based row/column are u16; offset is u64, defaults0 and cannot exceed captured
scrollback. The viewport starts at scrollback-offset, independent of TUI focus
or scroll. Optional revision is full-u64; generation is positive full-u64.
Reads one render row for an OSC8 URI (empty regions), then mapped text as needed
for a lowercase http(s) whitespace token with punctuation/balanced closer rules.
Wide/combining glyphs and verified soft wraps are preserved, including invisible
token ends. Shared8192-cell inspection includes spacers/tails and boundary probes;
exhaustion yields no link. Visible inclusive regions
are clipped to the requested viewport. URI/URL is returned without opening it
or invoking a plugin. No controller/input/attention/selection/clipboard effects.
One existing-Site viewer, five-second cancellable total, strict route fences;
65536 scanned rows/aggregate8MiB encoded bytes/1048576 stored glyphs. Stale,
evicted, malformed or over-limit data fails whole and releases; no retry.
JSON: orchbee.terminal.link-resolve v1. Text escapes controls.
"#;
pub(super) const HELP_ZH: &str = r#"用法：orchbee terminal link-resolve <terminal> --cursor ROW:COL [--offset-from-bottom N] [--content-revision N] [--generation N] [--format text|json]

Windows：解析显式只读原生视口中的完整单元格。零起点行／列为 u16；offset
为 u64，默认零，不得超出捕获的 scrollback。视口始于 scrollback-offset，
独立于 TUI 焦点／滚动。可选完整 u64 修订，generation 为正完整 u64。
先读一行 render 检查 OSC8 URI（空 regions），再按需读取映射文本，识别小写
http(s) 空白分隔 token，并处理尾部标点／平衡闭括号。保留宽字／组合字和已验证
软换行，包括不可见 token 端点。共享 8192 格探查计算宽字占位／尾格及边界探查，
预算耗尽返回无链接。
包含端点可见区域裁剪到请求视口。只返回 URI／URL，不打开，也不调用插件；
不取得 controller，不改输入／attention／选区／剪贴板。一次既有 Site viewer，
五秒可取消总期限，严格路由围栏；65536 扫描行、合计 8 MiB 编码字节、1048576
存储字形。过期／回收／格式错误／超限整体失败并释放，不重试。
JSON：orchbee.terminal.link-resolve v1；文本转义控制字符。
"#;
