//! Public terminal copy labels and bounded-command help in both locales.
use crate::{
    localization::Locale,
    terminal_copy::{CopySearchResult, TerminalTextPoint},
};
impl Locale {
    pub(crate) fn cli_terminal_copy_motion(
        self,
        terminal: &str,
        revision: u64,
        motion: &str,
        origin: TerminalTextPoint,
        cursor: TerminalTextPoint,
    ) -> String {
        match self {
            Self::EnUs => format!(
                "Terminal {terminal}, content revision {revision}\nMotion: {motion}, {}:{} → {}:{}\n",
                origin.row, origin.col, cursor.row, cursor.col
            ),
            Self::ZhCn => format!(
                "终端 {terminal}，内容修订 {revision}\n移动：{motion}，{}:{} → {}:{}\n",
                origin.row, origin.col, cursor.row, cursor.col
            ),
        }
    }
    pub(crate) fn cli_terminal_copy_search(
        self,
        terminal: &str,
        revision: u64,
        query: &str,
        result: &CopySearchResult,
    ) -> String {
        let query = crate::cli::escape::quoted(query);
        let current = result
            .current_global
            .map(|index| (index + 1).to_string())
            .unwrap_or_else(|| "—".into());
        let mut text = match self {
            Self::EnUs => format!(
                "Terminal {terminal}, content revision {revision}\nQuery: {query}\nMatches: {}, current: {current}\n",
                result.total
            ),
            Self::ZhCn => format!(
                "终端 {terminal}，内容修订 {revision}\n查询：{query}\n匹配：{}，当前：{current}\n",
                result.total
            ),
        };
        for (i, range) in result.matches.iter().enumerate() {
            text.push_str(&format!(
                "{} {}:{}–{}:{}\n",
                if Some(i) == result.current { "*" } else { " " },
                range.start.row,
                range.start.col,
                range.end.row,
                range.end.col
            ));
        }
        text
    }
}
pub(super) const HELP_MOTION_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal copy-motion <terminal> --cursor ROW:COL --motion MOTION [--content-revision N] [--generation N] [--format text|json]",
        "Windows: compute a screen-buffer cursor without input or Client selection changes.\nMotions: line-end, first-non-blank, next-word-start, previous-word-start,\nnext-word-end, next-big-word-start, previous-big-word-start, next-big-word-end,\nprevious-paragraph, next-paragraph. Words use tmux punctuation; big words split\nonly on whitespace. Whole glyphs and soft wraps are preserved. Paragraphs find\nthe nearest blank row within 999 rows; no target keeps the original cursor.\nCoordinates: zero-based u32 row/u16 column, within captured buffer geometry.\nOptional content revision is full-u64; generation is positive full-u64.\nOne existing-Site read-only viewer, cancellable five-second total. A complete\nscan admits at most 65536 rows, 8 MiB encoded/logical text and 1048576 stored\nglyphs; stale/evicted/oversized data fails whole. JSON: orchbee.terminal.copy-motion v1.",
    );
pub(super) const HELP_MOTION_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal copy-motion <terminal> --cursor ROW:COL --motion MOTION [--content-revision N] [--generation N] [--format text|json]",
        "Windows：计算屏幕缓冲区光标，不发送输入，也不改 Client 选区。移动：line-end、first-non-blank、next-word-start、previous-word-start、\nnext-word-end、next-big-word-start、previous-big-word-start、next-big-word-end、\nprevious-paragraph、next-paragraph。普通词按 tmux 标点分隔，大词仅按空白分隔；保留完整字形／软换行。段落找 999 行内最近空白行，没有目标保留原光标。零起点 u32 行／u16 列必须在捕获缓冲区几何内。内容修订为可选完整 u64，\ngeneration 为正 u64。一次既有 Site 只读 viewer，总期限五秒，可取消；完整扫描最多 65536 行、8 MiB 编码／逻辑文本及 1048576 保留字形；过期／回收／超限整体失败。JSON：orchbee.terminal.copy-motion v1。",
    );
pub(super) const HELP_SEARCH_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal copy-search <terminal> --cursor ROW:COL --query TEXT --direction forward|backward --content-revision N [--previous ROW:COL-ROW:COL] [--generation N] [--format text|json]",
        "Windows: literal search of complete retained logical text with Unicode smart case.\nAny uppercase query character makes it case-sensitive; query <=4096 UTF-8 bytes,\nempty means no matches. Whole glyphs and verified soft wraps are preserved.\nContent revision is required full-u64; generation is optional positive full-u64.\nPrevious is an inclusive ordered range; forward uses its end, backward its start.\nOtherwise cursor is the origin. Next match is strictly after/before the origin,\nwrapping to first/last when needed. Returns the exact total/global current index\nand at most 1024 matches centered around the current match. Zero has null indexes.\nOne existing-Site read-only viewer, cancellable five-second total; no selection,\nclipboard or input effects. Complete scans admit 65536 rows, 8 MiB encoded/logical\ntext and 1048576 stored glyphs. No truncated results/retry. JSON:\norchbee.terminal.copy-search v1; text escapes controls and marks current with *.",
    );
pub(super) const HELP_SEARCH_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal copy-search <terminal> --cursor ROW:COL --query TEXT --direction forward|backward --content-revision N [--previous ROW:COL-ROW:COL] [--generation N] [--format text|json]",
        "Windows：按字面量与 Unicode 智能大小写搜索完整保留逻辑文本；任意大写字符使其区分大小写。查询最多 4096 UTF-8 字节，空查询返回零匹配；保留完整字形、已验证软换行。必须指定完整 u64 内容修订，可另指定正 u64 generation。\nPrevious 是包含端点的已排序范围，向前用其末端，向后用始端；否则起点为光标。下一项必须严格位于起点之后／之前，没有则环绕到首项／末项。返回精确总数、全局当前位置及以当前项居中的最多 1024 项；零匹配当前索引为 null。一次既有 Site 只读 viewer，总期限五秒，可取消，不改选区／剪贴板，也不发送输入。完整扫描最多 65536 行、8 MiB 编码／逻辑文本、1048576 保留字形；不截断／重试。\nJSON：orchbee.terminal.copy-search v1；文本转义控制字符，以 * 标出当前项。",
    );
