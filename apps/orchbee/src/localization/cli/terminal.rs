//! Localized terminal query and input results, with command help; does not submit input.

use super::Locale;

pub(super) const HELP_TERMINAL_OBSERVE_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal session observe <terminal> [--cols <n>] [--rows <n>] [--timeout <ms>] [--format text|json]",
        "Windows: observe one existing terminal as NDJSON (orchbee.terminal.stream v1).\nTarget: complete terminal ID or @terminal:0x<hex>. Default viewport: 120x40;\ndimensions: 1..4096, at most 262144 cells. Crops/pads without resizing the source.\nFrames contain full safe ANSI screens encoded as base64, not raw PTY output.\nThe stream does not read stdin or acquire control. Both formats emit NDJSON.\nNo total timeout by default; --timeout: 1..3600000 ms. Ctrl+C exits 130; timeout\nexits 7. Each transport/output stall is bounded to five seconds; cleanup may\ntake two seconds. A disconnected source is a failure, not a successful exit.",
    );

pub(super) const HELP_TERMINAL_OBSERVE_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal session observe <terminal> [--cols <n>] [--rows <n>] [--timeout <ms>] [--format text|json]",
        "Windows：以 NDJSON 持续观察一个已有终端（orchbee.terminal.stream v1）。目标：完整终端 ID 或 @terminal:0x<hex>。默认视口为 120x40；单边尺寸为\n1..4096，总计最多 262144 个单元格。仅裁剪或填充，不改变源终端大小。帧包含以 base64 编码的完整安全 ANSI 屏幕，不是原始 PTY 输出。不读取 stdin，也不获取控制权。两种格式均输出 NDJSON。默认没有总超时；--timeout 范围为 1..3600000 毫秒。Ctrl+C 退出码为 130，超时为 7。每次传输或输出停滞最多五秒；清理可能需要两秒。源连接断开视为失败，不会报告成功退出。",
    );

pub(super) const HELP_TERMINAL_LIST_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal list [--workspace <t> | --tab <t> | --panel <t>] [--format text|json]",
        "List every terminal bound by a Presentation, optionally only those inside\none Workspace, Tab or Panel (exact ID, selector, or label where the resource\nhas one). Unbound terminals are not visible; a binding is structural\nevidence, not proof a process is alive. Text shows each terminal's\nWorkspace and Panel handles and its full working directory.",
    );

pub(super) const HELP_TERMINAL_LIST_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal list [--workspace <t> | --tab <t> | --panel <t>] [--format text|json]",
        "列出所有被 Presentation 绑定的终端，或仅列出某个工作区、标签页或面板内的终端（精确 ID、选择器，或资源支持时的 label）。未绑定终端不可见；绑定是结构证据，不是进程存活的证明。文本显示每个终端所在的工作区与面板句柄及完整工作目录。",
    );

pub(super) const HELP_TERMINAL_GET_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal get <terminal> [--format text|json]",
        "Show one bound terminal by complete typed ID or @terminal:0x<hex>:\nworker id and its single boundTo location (presentation, panel, tab,\ncontext).",
    );

pub(super) const HELP_TERMINAL_GET_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal get <terminal> [--format text|json]",
        "按完整类型化 ID 或 @terminal:0x<hex> 显示一个已绑定终端：worker id 及其唯一 boundTo 位置（presentation、panel、tab、context）。",
    );

pub(super) const HELP_TERMINAL_RUN_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal run (<target>|--panel <panel>) <command> [--wait] [--timeout <ms>] [--format text|json]",
        "Targets: terminal ID / @terminal:, or Panel ID / @panel: / --panel. Panels select the primary terminal; children require a terminal identity. Bare IDs that name different resources refuse; use a typed handle.\nSubmit one quoted command as paste followed by Enter in one request. Target: exact ID or @terminal:0x<hex>. Whitespace and newlines are preserved. Use -- before operands resembling options. Empty/extra command operands are rejected; total argv is at most 64 KiB.\nWithout --wait, the five-second send budget and orchbee.terminal.send receipt report input only.\nWindows --wait requires an idle managed PowerShell integration and an empty input line. It checks readiness before submitting once and waits for this invocation's correlated completion. The total timeout defaults to 30000 ms. JSON orchbee.terminal.run v1 keeps submission, completion, failure and viewer cleanup separately. powershell-success is 0/1 Shell status, not a native process exit code. Use JSON before submitting scripts that need recovery evidence. Cancellation does not stop the command. Never replay accepted, partial or unknown input. No controller, resize or focus is acquired.",
    );

pub(super) const HELP_TERMINAL_RUN_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal run (<target>|--panel <panel>) <command> [--wait] [--timeout <ms>] [--format text|json]",
        "目标：终端 ID／@terminal:，或面板 ID／@panel:／--panel。面板选择主终端，子终端需终端身份。裸 ID 指向不同资源时拒绝，请使用带类型的句柄。\n一次请求发送带引号命令的 paste 和 Enter。目标为完整 ID 或 @terminal:0x<hex>，保留空白和换行。操作数类似选项时使用 --。拒绝空命令和多余命令操作数，参数包最多 64 KiB。\n未加 --wait 时，五秒发送预算和 orchbee.terminal.send 回执只说明输入进度。\nWindows --wait 需要空闲的受管 PowerShell 集成及空输入行。发送前检查就绪，只提交一次并等待本次调用的关联完成。总期限默认 30000 毫秒。JSON orchbee.terminal.run v1 分别保留 submission、completion、failure 和 viewer 清理。powershell-success 是 0/1 Shell 状态，不是原生进程退出码。需要恢复证据的脚本应在提交前选择 JSON。取消不会停止命令。不要重放已接受、部分或未知输入。不取得 controller、resize 或焦点。",
    );

pub(super) const HELP_TERMINAL_SEND_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal send (<target>|--panel <panel>) (--text <utf8> [--enter] | --key <name>... | --file <path> [--paste] [--enter] | --stdin [--paste] [--enter]) [--format text|json]",
        "Targets: terminal ID / @terminal:, or Panel ID / @panel: / --panel. Panels select the primary terminal; children require a terminal identity. Bare IDs that name different resources refuse; use a typed handle.\nSubmit one bounded input sequence to one bound terminal. Choose exactly one\nsource family. --text accepts up to 4096 UTF-8 bytes including an optional\ncarriage return. Repeat --key for up to 64 semantic keys; named keys include\nenter, escape, tab, backspace, delete, up, down, left, right, home, end,\npage-up, page-down, ctrl+c, ctrl+d, insert, space and f1..f25. A single\nprintable Unicode character or punctuation name (plus, semicolon, etc.)\nis also accepted. Combine ctrl/control, alt/option, shift and super/cmd/command\nwith +, for example ctrl+shift+a or super+c. Quote shell punctuation.\nThe Host encodes keys for the terminal's current keyboard mode.\n\n--file reads a regular file. --stdin accepts a redirected regular file or\npipe, including Windows pipes, and refuses interactive terminals/devices.\nFile/stdin preserves raw bytes, including non-UTF-8 data, up to 1 MiB\nincluding an optional --enter carriage return. Empty input is refused.\n--paste is valid only with --file or --stdin, limits content to 256 KiB,\nand sends bracketed paste under the terminal's mode. With --paste, --enter\nadds a separate trailing Enter record. Nothing is sent until the complete\nsource is acquired and validated under the shared five-second deadline.\nWindows pipe cancellation terminates and reaps its owned reader process.\n\nThe temporary input-only attachment never takes the TUI's controller,\nviewer or resize ownership. Inspect inputStatus and cleanupStatus in the\norchbee.terminal.send JSON result independently. Partial acceptance is not\ncomplete success. When the outcome is unknown (exit 8), inspect the terminal\nbefore another action: blindly resending may type twice. No request is\nautomatically replayed.",
    );

pub(super) const HELP_TERMINAL_SEND_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal send (<target>|--panel <panel>) (--text <utf8> [--enter] | --key <name>... | --file <path> [--paste] [--enter] | --stdin [--paste] [--enter]) [--format text|json]",
        "目标：终端 ID／@terminal:，或面板 ID／@panel:／--panel。面板选择主终端，子终端需终端身份。裸 ID 指向不同资源时拒绝，请使用带类型的句柄。\n向一个已绑定终端提交一组有界输入。必须恰好选择一种来源。\n--text 接受不超过 4096 字节的 UTF-8 文字，含可选附加回车。重复 --key 可发送最多 64 个语义按键；具名按键包括 enter、escape、tab、\nbackspace、delete、up、down、left、right、home、end、page-up、\npage-down、ctrl+c、ctrl+d、insert、space 和 f1..f25。也接受单个可打印 Unicode 字符或 plus、semicolon 等标点名称。用 + 组合\nctrl/control、alt/option、shift、super/cmd/command，例如 ctrl+shift+a\n或 super+c；Shell 标点请加引号。由 Host 按终端当前键盘模式编码。\n\n--file 读取普通文件。--stdin 接受重定向普通文件或管道，包括 Windows\n管道，拒绝交互终端和设备。文件/stdin 原样保留字节，含非 UTF-8 数据，连同 --enter 的可选回车上限为 1 MiB。空输入被拒绝。\n--paste 仅用于 --file 或 --stdin，内容上限 256 KiB，按终端模式发送\nbracketed paste。此时 --enter 附加独立的尾随 Enter 记录。来源在共用五秒期限内完整读取并校验后才发送任何内容。\nWindows 管道取消时会终止并回收所属读取进程。\n\n临时仅输入附属通道不获取 TUI 的 controller、viewer 或 resize 所有权。分别检查 orchbee.terminal.send JSON 结果的 inputStatus 与 cleanupStatus。部分接受不等于全部成功。结果未知（退出码 8）时，采取下一步操作前先检查终端：盲目重发可能输入两次。请求不会自动重放。",
    );

pub(super) const HELP_TERMINAL_CONTROL_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal session control <terminal> [--takeover] [--cols N] [--rows N] [--timeout ms] [--format text|json]",
        "Windows: acquire exclusive input/resize control of an existing terminal and\nstream validated ANSI/base64 NDJSON frames. Default geometry 120x40.\n--takeover explicitly revokes the old controller; failed takeover may still\nrevoke it. Read stdin as UTF-8 NDJSON: terminal.input (text XOR base64 bytes),\nterminal.resize (cols,rows), terminal.scroll (direction,lines), terminal.release.\nInput records are bounded to 2 MiB; decoded input to 256 KiB. Invalid commands\nproduce rejection records without effects; input is admitted in ordered 4 KiB\nchunks. Receipts do not mean shell execution. EOF/release detaches; it never\nstops the terminal. Cancellation 130, timeout 7; no reconnect or effect replay.\nOutput stalls/exchanges are bounded to 5s, cleanup 2s. Normal-screen scroll\nuses retained history; application mouse modes and alternate screens use\nthe corresponding input. See schema orchbee.terminal.control.input.",
    );

pub(super) const HELP_TERMINAL_CONTROL_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal session control <terminal> [--takeover] [--cols N] [--rows N] [--timeout ms] [--format text|json]",
        "Windows：获取既有终端的排他输入/尺寸控制权，持续输出经过验证的\nANSI/base64 NDJSON 帧。默认尺寸120x40。--takeover 显式撤销旧控制者；接管失败也可能撤销旧权限。stdin 为 UTF-8 NDJSON：terminal.input\n（text 或 base64 bytes 二选一）、terminal.resize（cols、rows）、\nterminal.scroll（direction、lines）、terminal.release。每条记录最多2MiB，解码输入最多256KiB。无效命令返回无副作用的拒绝记录；输入按顺序分成4KiB 提交，接收回执不代表 shell 已执行。EOF/release 仅解除附着，不停止终端。取消130、超时7；不重连或重放效果。传输/输出停滞最多5秒，清理2秒。普通屏幕滚动使用历史，应用鼠标模式和备用屏幕使用对应输入。详见 schema orchbee.terminal.control.input。",
    );

pub(super) const HELP_TERMINAL_ATTACH_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal attach <terminal> [--takeover] [--timeout ms] [--mouse on|off] [--format text|json]",
        "Windows interactive terminal attachment. Requires terminal stdin/stdout.\nUses an existing terminal ID or @terminal:0x<hex>; never starts a session or focuses\na Workbench Panel. Uses the console size and exclusive input/resize control.\n--takeover revokes the old controller and may revoke it even on failure.\nCtrl+B then q detaches; Ctrl+B twice sends a literal Ctrl+B. Ctrl+C reaches\nthe application. Paste is semantic; plain page keys and wheel use history\nor the application's mouse/cursor modes. Mouse is on by default.\n--timeout is 1..3600000 ms; default unlimited, exchanges bounded to 5s, cleanup\nto 2s. No reconnect or input replay. Modes restore before diagnostics.\nOutput is an interactive screen with either --format; use terminal session\nobserve/control for NDJSON.",
    );

pub(super) const HELP_TERMINAL_ATTACH_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal attach <terminal> [--takeover] [--timeout ms] [--mouse on|off] [--format text|json]",
        "Windows 交互终端附着，需要终端 stdin/stdout。使用既有终端 ID 或\n@terminal:0x<hex>，不启动会话，也不聚焦 Workbench 面板。采用控制台尺寸，取得独占输入/尺寸控制权。--takeover 撤销旧控制者；失败也可能撤销旧权限。\nCtrl+B 后按 q 脱离；连续两次 Ctrl+B 发送字面 Ctrl+B。Ctrl+C 交付应用。粘贴使用语义操作；普通翻页键和滚轮使用历史或应用鼠标/光标模式。默认启用鼠标。--timeout 为 1..3600000 毫秒，默认不限总时长；单次交换最多 5 秒、清理最多 2 秒。不重连或重放输入。输出诊断前恢复终端模式。两种 --format 均显示交互画面；NDJSON 请使用 terminal session observe/control。",
    );

pub(super) const HELP_TERMINAL_READ_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal read (<target>|--panel <panel>) [--source visible|recent|recent-unwrapped|detection] [--lines <n>] [--plain|--ansi] [--format text|json]",
        "Targets: terminal ID / @terminal:, or Panel ID / @panel: / --panel. Panels select the primary terminal; children require a terminal identity. Bare IDs that name different resources refuse; use a typed handle.\nRead a bound terminal through one temporary read-only viewer. Default source: recent, 80 physical rows including history and the live screen. --source visible reads the screen; recent-unwrapped joins verified soft wraps; detection reads a cursor-centered screen-height window. --lines accepts 0..4294967295 and clamps to 1000. No input, focus or resize effect. --plain emits UTF-8 and conflicts with --format json. --ansi emits styled SGR text; JSON always uses orchbee.terminal.output v1 with explicit text/ansi encoding. A nullable output cursor can fence a later --after wait. The cursor is not a command completion marker. Sources never silently widen after a stale sample.",
    );

pub(super) const HELP_TERMINAL_READ_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal read (<target>|--panel <panel>) [--source visible|recent|recent-unwrapped|detection] [--lines <n>] [--plain|--ansi] [--format text|json]",
        "目标：终端 ID／@terminal:，或面板 ID／@panel:／--panel。面板选择主终端，子终端需终端身份。裸 ID 指向不同资源时拒绝，请使用带类型的句柄。\n通过一个临时只读 viewer 读取已绑定终端。默认来源 recent，包含历史及当前屏幕的 80 个物理行。--source visible 读取屏幕；recent-unwrapped 拼接已验证软换行；detection 读取光标附近一个屏幕高度的窗口。--lines 接受 0..4294967295 并钳制到 1000。不输入、不改焦点或尺寸。--plain 输出 UTF-8，与 --format json 互斥。--ansi 输出带 SGR 样式的文本，JSON 均使用 orchbee.terminal.output v1，并明确 text/ansi 编码。可空输出游标可用于后续 --after 等待，但不表示命令完成。样本陈旧时不会悄悄扩大来源范围。",
    );

pub(super) const HELP_TERMINAL_WAIT_OUTPUT_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal wait-output (<target>|--panel <panel>) (--match <text>|--regex <pattern>) [--source visible|recent|recent-unwrapped|detection] [--lines <n>] [--after <cursor|now>] [--timeout <ms>|--forever] [--format text|json]",
        "Targets: terminal ID / @terminal:, or Panel ID / @panel: / --panel. Panels select the primary terminal; children require a terminal identity. Bare IDs that name different resources refuse; use a typed handle.\nWait for selected output with a 30000 ms default deadline. --timeout accepts 1..3600000 ms; --forever explicitly removes the total deadline and conflicts with --timeout. Each I/O stays bounded and Ctrl+C releases the read-only viewer. Default source: recent, 80 physical rows. --lines clamps to 1000; zero never matches. --match is a case-sensitive literal of 1..4096 UTF-8 bytes; --regex is at most 1024 bytes and compiles before connection. Existing content can match unless --after supplies a read cursor or now. Fresh waits exclude the captured boundary and prompt/input rows; reset, eviction or a lost selected window fails explicitly. No input, resize or focus change. Exit 0 on match, 7 on timeout, 130 on cancellation. JSON uses orchbee.terminal.wait-output v1 with a nullable after member.",
    );

pub(super) const HELP_TERMINAL_WAIT_OUTPUT_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal wait-output (<target>|--panel <panel>) (--match <text>|--regex <pattern>) [--source visible|recent|recent-unwrapped|detection] [--lines <n>] [--after <cursor|now>] [--timeout <ms>|--forever] [--format text|json]",
        "目标：终端 ID／@terminal:，或面板 ID／@panel:／--panel。面板选择主终端，子终端需终端身份。裸 ID 指向不同资源时拒绝，请使用带类型的句柄。\n等待所选输出，默认总期限 30000 ms。--timeout 接受 1..3600000 毫秒；--forever 显式取消总期限，与 --timeout 互斥。每次 I/O 仍有界，Ctrl+C 释放只读 viewer。默认来源 recent、80 个物理行。--lines 钳制到 1000，零行永不匹配。--match 是区分大小写的 1..4096 UTF-8 字节字面值；--regex 最多 1024 字节，连接前编译。除非 --after 指定读取游标或 now，否则既有内容也可匹配。新输出等待排除捕获边界及提示符／输入行；重置、逐出或选定窗口丢失会明确失败。不输入、不改尺寸或焦点。匹配退出 0，超时退出 7，取消退出 130。JSON 使用 orchbee.terminal.wait-output v1，after 成员始终存在且可空。",
    );

pub(super) const HELP_TERMINAL_SELECTION_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal selection-read <terminal> --anchor ROW:COL --cursor ROW:COL [--content-revision N] [--generation N] [--format text|json]",
        "Windows: read one inclusive screen-buffer selection, independent of Client viewport.\nUse a complete terminal ID or @terminal:0x<hex>. Coordinates are zero-based;\nrow 0..4294967295, column 0..65535, at most 4096 selected rows and an 8 MiB\nrow window. Reversed endpoints are ordered. Wide/combining glyphs remain whole;\nverified soft wraps join, hard-line padding is trimmed.\nContent revision is optional full-u64; generation is optional positive full-u64.\nBoth pin expected content before reading. Changed/evicted content fails whole.\nOne cancellable five-second existing-session read; no input, clipboard or user\nselection changes. JSON: orchbee.terminal.selection v1; text escapes controls.",
    );

pub(super) const HELP_TERMINAL_SELECTION_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee terminal selection-read <terminal> --anchor ROW:COL --cursor ROW:COL [--content-revision N] [--generation N] [--format text|json]",
        "Windows：读取包含两个端点的屏幕缓冲区选区，与客户端视口无关。使用完整终端 ID 或 @terminal:0x<hex>。坐标以零开始；行 0..4294967295、列 0..65535，最多选 4096 行及一个 8 MiB 行窗口。反向端点先排序；保留完整宽字／组合字形，连接已验证软换行并移除硬行尾填充。可选内容修订为完整 u64，generation 为正 u64；先检查预期内容，变化或已回收内容整体失败。一次可取消的五秒既有会话读取；不发送输入、不改用户选区／剪贴板。JSON 为 orchbee.terminal.selection v1，文本转义控制字符。",
    );

impl Locale {
    pub(crate) fn cli_terminal_run_result(self, terminal: &str, completed: bool) -> String {
        match (self, completed) {
            (Self::EnUs, true) => {
                format!("Command completed successfully in terminal {terminal}.\n")
            }
            (Self::ZhCn, true) => format!("终端 {terminal} 中的命令已成功完成。\n"),
            (Self::EnUs, false) => format!(
                "Command did not complete successfully in terminal {terminal}. Submission and completion evidence requires selecting --format json before the original invocation. Never replay accepted or unknown input. Waiting requires an idle managed PowerShell shell.\n"
            ),
            (Self::ZhCn, false) => format!(
                "终端 {terminal} 中的命令未成功完成。提交和完成证据须在原调用前选择 --format json；不要重放已接受或结果未知的输入。等待功能需要处于空闲状态的受管 PowerShell。\n"
            ),
        }
    }

    pub(crate) fn cli_terminal_selection(
        self,
        terminal: &str,
        revision: u64,
        text: &str,
    ) -> String {
        let text = crate::cli::escape::quoted(text);
        match self {
            Self::EnUs => {
                format!("Terminal {terminal}, content revision {revision}\nSelection: {text}\n")
            }
            Self::ZhCn => format!("终端 {terminal}，内容修订 {revision}\n选区：{text}\n"),
        }
    }

    /// A successful input submission, without claiming command completion.
    pub(crate) fn cli_send_success(self, terminal_id: &str) -> String {
        match self {
            Self::EnUs => format!("Sent to terminal {terminal_id}.\n"),
            Self::ZhCn => format!("已发送至终端 {terminal_id}。\n"),
        }
    }

    /// The stderr line a typed `terminal send` answer carries on a nonzero
    /// exit: what the outcome axis settled, and — whenever resending could
    /// type the same input twice — an explicit do-not-resend warning. The
    /// send envelope on stdout still reports both axes verbatim.
    pub(crate) fn cli_send_stderr(
        self,
        verdict: crate::cli::send::SendVerdict,
        reason: Option<&'static str>,
        cleanup_unconfirmed: bool,
    ) -> String {
        use crate::cli::send::SendVerdict;
        let cleanup_note = match (self, cleanup_unconfirmed) {
            (_, false) => "",
            (Self::EnUs, true) => " Release of the temporary input channel is unconfirmed.",
            (Self::ZhCn, true) => " 临时输入通道的释放状态未确认。",
        };
        match verdict {
            SendVerdict::Quiet => String::new(),
            SendVerdict::AcceptedCleanupUnconfirmed => match self {
                Self::EnUs => {
                    "The terminal accepted the input, but release of the temporary input channel could not be confirmed. Do not resend — the input may already have taken effect.\n".to_owned()
                }
                Self::ZhCn => {
                    "终端已接受该输入，但临时输入通道的释放无法确认。请勿重发——输入可能已生效。\n".to_owned()
                }
            },
            SendVerdict::Rejected => match self {
                Self::EnUs => format!(
                    "The local session rejected the input ({}); nothing was sent.{cleanup_note}\n",
                    reason.unwrap_or("unknown")
                ),
                Self::ZhCn => format!(
                    "本地会话拒绝了该输入（{}）；未发送任何内容。{cleanup_note}\n",
                    reason.unwrap_or("unknown")
                ),
            },
            SendVerdict::PartialAccepted => match self {
                Self::EnUs => format!(
                    "The send stopped partway ({}): the receipt's recordsAccepted prefix was typed; the stopped record and everything after it was not sent. Do not resend the whole sequence — the accepted prefix may already have taken effect.{cleanup_note}\n",
                    reason.unwrap_or("unknown")
                ),
                Self::ZhCn => format!(
                    "发送中途停止（{}）：回执中 recordsAccepted 前缀的输入已键入；被停下的记录及其后的内容未发送。请勿重发整个序列——已接受的前缀可能已生效。{cleanup_note}\n",
                    reason.unwrap_or("unknown")
                ),
            },
            SendVerdict::Unavailable => match self {
                Self::EnUs => format!(
                    "The input could not be submitted ({}); nothing was sent.{cleanup_note}\n",
                    reason.unwrap_or("unknown")
                ),
                Self::ZhCn => format!(
                    "该输入未能提交（{}）；未发送任何内容。{cleanup_note}\n",
                    reason.unwrap_or("unknown")
                ),
            },
            SendVerdict::OutcomeUnknown => match self {
                Self::EnUs => format!(
                    "The send outcome is unknown: the input may or may not have reached the terminal. It was not retried — inspect the terminal before deciding whether to resend.{cleanup_note}\n"
                ),
                Self::ZhCn => format!(
                    "发送结果未知：输入可能已到达终端，也可能未到达。未自动重试——请先检查终端，再决定是否重发。{cleanup_note}\n"
                ),
            },
        }
    }

    /// The text-output marker appended when `--lines` tailed emitted entries
    /// — rows, or logical lines under `--unwrap`. Distinct from the read
    /// budget's own truncation marker.
    #[cfg(test)]
    pub(crate) fn cli_read_tail_omitted(self, omitted: usize, logical: bool) -> String {
        match (self, logical) {
            (Self::EnUs, false) => format!("… {omitted} earlier rows not shown\n"),
            (Self::EnUs, true) => format!("… {omitted} earlier lines not shown\n"),
            (Self::ZhCn, false) => format!("… 未显示前 {omitted} 行\n"),
            (Self::ZhCn, true) => format!("… 未显示前 {omitted} 个逻辑行\n"),
        }
    }

    /// The text-output marker appended when the read budget omitted frame rows.
    pub(crate) fn cli_read_truncated(self, omitted: usize) -> String {
        match self {
            Self::EnUs => format!("… {omitted} more rows truncated at the read limit\n"),
            Self::ZhCn => format!("… 因读取上限省略了 {omitted} 行\n"),
        }
    }

    pub(crate) fn cli_terminal_agent_client_view(
        self,
        client: &str,
        status: Option<&str>,
        seen: Option<bool>,
    ) -> String {
        let status = self.cli_agent_client_status(status);
        let seen = self.cli_agent_seen(seen);
        match self {
            Self::EnUs => format!("Client {client}: status {status}; seen {seen}\n"),
            Self::ZhCn => format!("客户端 {client}：状态 {status}；已看见 {seen}\n"),
        }
    }

    pub(crate) const fn cli_terminal_list_state(
        self,
        state: crate::cli::results::TerminalListState,
    ) -> &'static str {
        use crate::cli::results::TerminalListState;
        use orchbee_workbench_protocol::PanelAgentState;
        match (self, state) {
            (_, TerminalListState::Observed) => match self {
                Self::EnUs => "observed",
                Self::ZhCn => "已观察",
            },
            (Self::EnUs, TerminalListState::Agent(PanelAgentState::Idle)) => "idle",
            (Self::EnUs, TerminalListState::Agent(PanelAgentState::Working)) => "working",
            (Self::EnUs, TerminalListState::Agent(PanelAgentState::Blocked)) => "blocked",
            (Self::EnUs, TerminalListState::Agent(PanelAgentState::Done)) => "done",
            (Self::EnUs, TerminalListState::Agent(PanelAgentState::Error)) => "error",
            (Self::EnUs, TerminalListState::Agent(PanelAgentState::Unknown)) => "unknown",
            (Self::ZhCn, TerminalListState::Agent(PanelAgentState::Idle)) => "空闲",
            (Self::ZhCn, TerminalListState::Agent(PanelAgentState::Working)) => "工作中",
            (Self::ZhCn, TerminalListState::Agent(PanelAgentState::Blocked)) => "受阻",
            (Self::ZhCn, TerminalListState::Agent(PanelAgentState::Done)) => "已完成",
            (Self::ZhCn, TerminalListState::Agent(PanelAgentState::Error)) => "错误",
            (Self::ZhCn, TerminalListState::Agent(PanelAgentState::Unknown)) => "未知",
        }
    }

    pub(crate) const fn cli_terminal_observations_heading(self) -> &'static str {
        match self {
            Self::EnUs => "Terminal observations: Panel / title / directory",
            Self::ZhCn => "终端观察：面板 / 标题 / 目录",
        }
    }

    pub(crate) const fn cli_scroll_heading(self) -> &'static str {
        match self {
            Self::EnUs => "Client scroll: Panel / state / offset / maximum / rows",
            Self::ZhCn => "客户端滚动：面板 / 状态 / 偏移 / 最大偏移 / 行数",
        }
    }

    pub(crate) const fn cli_scroll_states(self) -> [&'static str; 7] {
        match self {
            Self::EnUs => [
                "not attached",
                "frame unavailable",
                "entering history",
                "awaiting window",
                "recovering",
                "live",
                "history",
            ],
            Self::ZhCn => [
                "未连接",
                "帧不可用",
                "进入历史中",
                "等待窗口",
                "恢复中",
                "实时",
                "历史",
            ],
        }
    }
}
