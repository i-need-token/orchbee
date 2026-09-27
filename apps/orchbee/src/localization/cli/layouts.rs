pub(super) const EXPORT_EN: &str = "orchbee layout export [--tab <target>|--panel <target>|--current] [--client <id>] [--timeout <ms>] [--format text|json]\n\nExport a reusable layout as JSON, including observed split ratios, manual labels,\ndirectories and immutable launch argv. Requires one Ready Windows TUI Client;\nselect it with --client when ambiguous. An omitted target uses the validated\ncaller or selected Client Panel. Does not change focus or terminal control.\nDefault total timeout: 30000 ms. At most 24 Panels and depth 16. Environment\nvalues are never exported. Both formats produce JSON on success.\n";
pub(super) const EXPORT_ZH: &str = "orchbee layout export [--tab <target>|--panel <target>|--current] [--client <id>] [--timeout <ms>] [--format text|json]\n\n导出可复用 JSON 布局，包含观测比例、手工标签、目录和不可变启动参数。\n要求一个 Ready Windows TUI Client；有歧义时用 --client 指定。省略目标使用\n已验证调用者或所选 Client Panel，不改变焦点或终端控制权。\n总超时默认 30000 ms，最多 24 个 Panel、深度 16。不导出环境变量值。\n两种格式成功时均输出 JSON。\n";

pub(super) const FAMILY_EN: &str = "orchbee layout apply|export|set-split-ratio\n\nApply or export reusable JSON layouts, or adjust a split in one Windows TUI Client.\nRun orchbee layout <command> --help for targets and options.\n";
pub(super) const FAMILY_ZH: &str = "orchbee layout apply|export|set-split-ratio\n\n应用或导出可复用 JSON 布局，或调整一个 Windows TUI Client 的分屏比例。\n运行 orchbee layout <command> --help 查看目标与选项。\n";
pub(super) const APPLY_EN: &str = "orchbee layout apply (--file <path>|--stdin) [--tab <target> --confirm|--workspace <target>|--standalone] [--label <label>] [--client <id>] [--focus|--no-focus] [--timeout <ms>] [--format text|json]\n\nCreate a fresh Tab from a JSON layout, or replace a confirmed Tab after every new\nterminal has started and its layout is ready. Requires one Ready Windows TUI.\nAccepts native exports, layout descriptions and Herdr pane nodes. Exported IDs\nnever select or reuse terminals. Default focus: false, except active replacement.\nInput: UTF-8 file or noninteractive stdin, optional BOM, at most 1 MiB, 24 leaves\nand 16 levels including root. Default total timeout: 30000 ms, including recovery.\nFailure receipts retain created IDs and acknowledged effects; uncertain or\nconcurrently edited resources are preserved. Use --format json for full receipts.\n";
pub(super) const APPLY_ZH: &str = "orchbee layout apply (--file <path>|--stdin) [--tab <target> --confirm|--workspace <target>|--standalone] [--label <label>] [--client <id>] [--focus|--no-focus] [--timeout <ms>] [--format text|json]\n\n从 JSON 布局新建标签页；替换需要 --confirm，并等待所有新终端启动及布局就绪。\n要求一个 Ready Windows TUI。接受原生导出、布局描述和 Herdr pane 节点。\n导出标识不会选择目标或复用终端。默认不切换焦点，替换活动标签页除外。\n输入为 UTF-8 文件或非交互 stdin，允许 BOM，最多 1 MiB、24 个叶子、含根 16 层。\n总超时默认 30000 ms，包含恢复时间。失败回执保留已创建标识和已确认效果；\n结果不确定或被并发编辑的资源会保留。--format json 可查看完整回执。\n";

impl crate::localization::Locale {
    /// Render the application outcome and retained recovery identities in the selected locale.
    pub(crate) fn cli_layout_apply(self, document: &serde_json::Value) -> String {
        let result = &document["result"];
        let complete = result["outcome"] == "completed";
        let (title, new_label, old_label, stage_label, recovery_label, receipt_label) = match self {
            Self::EnUs => (
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
            Self::ZhCn => (
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
            let stage = stage(self, result["stage"].as_str().unwrap_or_default());
            let recovery = match (
                self,
                result["recovery"]["outcome"].as_str().unwrap_or_default(),
            ) {
                (Self::EnUs, "not-needed") => "not needed",
                (Self::ZhCn, "not-needed") => "无需恢复",
                (Self::EnUs, "closed") => "new Tab closed",
                (Self::ZhCn, "closed") => "新标签页已关闭",
                (Self::EnUs, "closing") => "close accepted",
                (Self::ZhCn, "closing") => "已接受关闭请求",
                (Self::EnUs, "incomplete") => "incomplete; inspect retained receipts",
                (Self::ZhCn, "incomplete") => "未完成，请检查保留的回执",
                (Self::EnUs, _) => "new Tab preserved",
                (Self::ZhCn, _) => "新标签页已保留",
            };
            text.push_str(&format!(
                "{stage_label}: {stage}\n{recovery_label}: {recovery}\n"
            ));
        }
        for (key, label) in [("newTabId", new_label), ("oldTabId", old_label)] {
            if let Some(id) = result[key].as_str() {
                text.push_str(&format!("{label}: {id}\n"));
            }
        }
        if let Some(effects) = result["effects"].as_array() {
            for effect in effects {
                let receipt = &effect["document"];
                if let Some(id) = receipt["source"]["requestId"]
                    .as_str()
                    .or_else(|| receipt["operationId"].as_str())
                {
                    text.push_str(&format!(
                        "{receipt_label}: {} {}\n",
                        stage(self, effect["phase"].as_str().unwrap_or_default()),
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
        (Locale::ZhCn, "split") => "创建窗格",
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
