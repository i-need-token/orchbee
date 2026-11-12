//! Formats layout command guidance and application receipts, including retained recovery identities; it does not change layouts.
use super::Locale;
pub(super) const EXPORT_EN: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee layout export [--tab <target>|--panel <target>|--current] [--client <id>] [--timeout <ms>] [--format text|json]",
    "Export a reusable layout as JSON, including observed split ratios, manual labels,\ndirectories and immutable launch argv. Explicit --tab/--panel without --client exports shared structure without a TUI.\nAdd --client for its focus and maximized-state observations; omitted targets require a Ready Client. An omitted target uses the validated\ncaller or selected Client Panel. Does not change focus or terminal control.\nDefault total timeout: 30000 ms. At most 24 Panels and depth 16. Environment\nvalues are never exported. Primary/child order and empty terminal slots are\npreserved, with at most 16 children per Panel. Both formats produce JSON on success.",
);
pub(super) const EXPORT_ZH: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee layout export [--tab <target>|--panel <target>|--current] [--client <id>] [--timeout <ms>] [--format text|json]",
    "导出可复用 JSON 布局，包含观测比例、手工标签、目录和不可变启动参数。显式 --tab／--panel 且省略 --client 时无头导出共享结构。添加 --client 可包含其焦点及最大化状态观察；省略目标需要 Ready 客户端。省略目标使用已验证调用者或所选客户端面板，不改变焦点或终端控制权。总超时默认 30000 ms，最多 24 个面板、深度 16。不导出环境变量值。保留主／子终端顺序和空终端位，每个面板最多 16 个子终端。两种格式成功时均输出 JSON。",
);

pub(super) const APPLY_EN: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee layout apply (--file <path>|--stdin) [--tab <target> --confirm|--workspace <target>] [--label <label>] [--client <id>] [--focus|--no-focus] [--timeout <ms>] [--format text|json]",
    "Create a fresh Tab from a JSON layout, or replace a confirmed Tab after every new\nterminal has started and its layout is ready. Explicit destinations without\n--client or --focus run headlessly; other cases require one Ready Windows TUI.\nAccepts native exports, layout descriptions and Panel nodes. Primary/child\norder and empty slots are preserved; only requested members start. Exported IDs\nnever select or reuse terminals. Default focus: false, except active replacement.\nInput: UTF-8 file or noninteractive stdin, optional BOM, at most 1 MiB, 24 leaves\nand 16 levels including root; at most 16 children per Panel. Default total timeout:\n30000 ms, including recovery.\nFailure receipts retain created IDs and acknowledged effects; uncertain or\nconcurrently edited resources are preserved. Use --format json for full receipts.",
);
pub(super) const APPLY_ZH: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee layout apply (--file <path>|--stdin) [--tab <target> --confirm|--workspace <target>] [--label <label>] [--client <id>] [--focus|--no-focus] [--timeout <ms>] [--format text|json]",
    "从 JSON 布局新建标签页；替换需要 --confirm，并等待所有新终端启动及布局就绪。显式目的地且省略 --client 和 --focus 时可无头运行；其他情况需要一个 Ready Windows TUI。接受原生导出、布局描述和面板节点。导出标识不会选择目标或复用终端。默认不切换焦点，替换活动标签页除外。输入为 UTF-8 文件或非交互 stdin，允许 BOM，最多 1 MiB、24 个叶子、含根 16 层。总超时默认 30000 ms，包含恢复时间。失败回执保留已创建标识和已确认效果；结果不确定或被并发编辑的资源会保留。--format json 可查看完整回执。保留主／子终端顺序及空终端位，仅启动指定成员，每个面板最多 16 个子终端。",
);

impl super::CliText {
    /// Render the application outcome and retained recovery identities in the selected locale.
    pub(crate) fn cli_layout_apply(self, document: &serde_json::Value) -> String {
        let result = &document["result"];
        let complete = result["outcome"] == "completed";
        let (title, new_label, old_label, stage_label, recovery_label, receipt_label) =
            match self.locale {
                Locale::EnUs => (
                    if complete {
                        "Layout applied"
                    } else {
                        "Layout application incomplete"
                    },
                    "New Tab",
                    "Replacement target",
                    "Stage",
                    "Recovery",
                    "Request",
                ),
                Locale::ZhCn => (
                    if complete {
                        "布局应用完成"
                    } else {
                        "布局应用未完成"
                    },
                    "新标签页",
                    "替换目标",
                    "阶段",
                    "恢复",
                    "请求",
                ),
            };
        let mut text = format!("{title}\n");
        if !complete {
            let stage = stage(self.locale, result["stage"].as_str().unwrap_or_default());
            let recovery = match (
                self.locale,
                result["recovery"]["outcome"].as_str().unwrap_or_default(),
            ) {
                (Locale::EnUs, "not-needed") => "not needed",
                (Locale::ZhCn, "not-needed") => "无需恢复",
                (Locale::EnUs, "closed") => "new Tab closed",
                (Locale::ZhCn, "closed") => "新标签页已关闭",
                (Locale::EnUs, "closing") => "close accepted",
                (Locale::ZhCn, "closing") => "已接受关闭请求",
                (Locale::EnUs, "incomplete") => "incomplete; inspect retained receipts",
                (Locale::ZhCn, "incomplete") => "未完成，请检查保留的回执",
                (Locale::EnUs, _) => "new Tab preserved",
                (Locale::ZhCn, _) => "新标签页已保留",
            };
            text.push_str(&format!(
                "{stage_label}: {stage}\n{recovery_label}: {recovery}\n"
            ));
        }
        for (key, label) in [("newTabId", new_label), ("oldTabId", old_label)] {
            if let Some(id) = result[key].as_str() {
                text.push_str(&format!(
                    "{label}: {}\n",
                    crate::cli_text::escape::escape(&crate::cli::handles::tab(id))
                ));
            }
        }
        if let Some(effects) = result["effects"].as_array() {
            for effect in effects {
                let receipt = &effect["document"];
                if let Some(id) = receipt["source"]["requestId"]
                    .as_str()
                    .or_else(|| receipt["requestId"].as_str())
                    .or_else(|| receipt["result"]["operationId"].as_str())
                {
                    text.push_str(&format!(
                        "{receipt_label}: {} {}\n",
                        stage(self.locale, effect["phase"].as_str().unwrap_or_default()),
                        serde_json::json!(id)
                    ));
                }
            }
        }
        text
    }
}

fn stage(locale: crate::localization::Locale, value: &str) -> &str {
    use crate::localization::Locale;
    match (locale, value) {
        (Locale::ZhCn, "input") => "读取输入",
        (Locale::ZhCn, "discovery") => "连接运行环境",
        (Locale::ZhCn, "directory") => "确认工作目录",
        (Locale::ZhCn, "create") => "新建标签页",
        (Locale::ZhCn, "split") => "创建面板",
        (Locale::ZhCn, "launch") => "等待终端启动",
        (Locale::ZhCn, "label") => "设置标签",
        (Locale::ZhCn, "ratio") => "设置分屏比例",
        (Locale::ZhCn, "ready") => "检查新布局",
        (Locale::ZhCn, "focus") => "切换焦点",
        (Locale::ZhCn, "replace") => "关闭原标签页",
        (Locale::ZhCn, "replace-status") => "确认关闭状态",
        (Locale::ZhCn, "observation") => "读取最终布局",
        (Locale::ZhCn, "rollback") => "回收新标签页",
        (Locale::ZhCn, "complete") => "完成",
        _ => value,
    }
}

pub(super) const HISTORY_EN: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee layout history [--format text|json]",
    "List the earlier layouts this Session keeps, newest first. The Server keeps one when the\nlayout changed and at least 15 minutes passed since the last one, up to 48. Each entry shows its\nsave time (the value layout reopen takes), when it was saved and how many of its Workspaces and\nTabs are not open now. Read-only.",
);
pub(super) const HISTORY_ZH: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee layout history [--format text|json]",
    "列出本会话保留的早先布局，最新的在前。布局有变化且距上一份至少 15 分钟时，服务器保留一份，最多 48 份。每项显示保存时间（即 layout reopen 接受的值）、保存于何时，以及其中多少工作区和标签页当前未打开。只读。",
);
pub(super) const REOPEN_EN: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee layout reopen <saved-at> [--format text|json]",
    "Reopen the Workspaces and Tabs of one earlier layout that are not open now. <saved-at> is\nthe save time layout history shows. Nothing open is closed, moved or renamed; reopened items\nget new identities and keep their split layout and labels. Shells start in their saved\ndirectories; a Panel whose directory is gone waits with Retry and Start in home folder. No\nprocess or Agent comes back, and focus does not move. A reopened item counts as open, so\nreopening the same layout again reopens nothing. An unknown outcome exits 8; check the open\nTabs before repeating.",
);
pub(super) const REOPEN_ZH: crate::cli_text::HelpDetails = crate::cli_text::HelpDetails::new(
    "orchbee layout reopen <saved-at> [--format text|json]",
    "重新打开某个早先布局中当前未打开的工作区和标签页。<saved-at> 是 layout history 显示的保存时间。不会关闭、移动或重命名已打开的内容；重新打开的项获得新标识，保留分屏布局和标签。Shell 在保存的目录中启动；目录已不存在的面板会等待，可选择重试或在主文件夹启动。不会恢复进程或 Agent，也不移动焦点。重新打开的项算作已打开，因此再次重新打开同一布局不会重复打开。结果未知时退出码为 8，重复前请先查看已打开的标签页。",
);

impl super::CliText {
    /// A listing or a reopen outcome, in the selected locale.
    pub(crate) fn cli_layout_history(
        self,
        outcome: &orchbee_workbench_protocol::LayoutHistoryOutcome,
        now: std::time::SystemTime,
    ) -> String {
        let orchbee_workbench_protocol::LayoutHistoryOutcome::Listed { entries } = outcome else {
            return format!(
                "{}\n",
                self.locale
                    .earlier_layout_outcome(outcome)
                    .unwrap_or_default()
            );
        };
        if entries.is_empty() {
            return format!("{}\n", self.locale.earlier_layouts_empty());
        }
        let mut text = format!("{}\n", self.locale.earlier_layouts_title());
        for entry in entries {
            text.push_str(&format!(
                "{}  {}  {}\n",
                entry.saved_at,
                self.locale.earlier_layout_when(entry.saved_at, now),
                self.locale.earlier_layout_summary(entry)
            ));
        }
        text
    }

    pub(crate) fn cli_layout_reopen_unknown(self, reason: &str) -> String {
        format!("{} ({reason})\n", self.locale.earlier_layout_unknown())
    }
}
