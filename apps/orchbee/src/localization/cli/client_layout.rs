//! Localized layout observations; machine geometry and identifiers remain locale independent.
use super::Locale;
use crate::invocation::LayoutQuery;
use orchbee_workbench_protocol::{ClientCellRect, ClientPanelDirection, ClientPanelLayout};

impl Locale {
    #[cfg(windows)]
    pub(crate) fn cli_shared_layout(self, document: &serde_json::Value) -> String {
        let heading = match self {
            Self::EnUs => "Shared layout (session scope)",
            Self::ZhCn => "共享布局（会话范围）",
        };
        format!(
            "{heading}\n{}\n",
            serde_json::to_string_pretty(&document["result"]).expect("JSON values serialize")
        )
    }
    pub(crate) fn cli_panel_layout(self, layout: &ClientPanelLayout, query: LayoutQuery) -> String {
        if !matches!(query, LayoutQuery::Layout) && !layout.source_available {
            return match self {
                Self::EnUs => "Source Panel is hidden at this viewport size; neighbors and edges are unavailable.\n",
                Self::ZhCn => "源面板在当前视口尺寸下隐藏；无法判断邻居和边缘。\n",
            }.into();
        }
        match query {
            LayoutQuery::Neighbor(direction) => layout.neighbors.get(direction).map_or_else(
                || match self {
                    Self::EnUs => "No adjacent Panel.\n".into(),
                    Self::ZhCn => "没有相邻面板。\n".into(),
                },
                |id| format!("{id}\n"),
            ),
            LayoutQuery::Edges => {
                let edge = match self {
                    Self::EnUs => "edge",
                    Self::ZhCn => "边缘",
                };
                let interior = match self {
                    Self::EnUs => "interior",
                    Self::ZhCn => "内部",
                };
                let mut text = String::new();
                for direction in [
                    ClientPanelDirection::Left,
                    ClientPanelDirection::Right,
                    ClientPanelDirection::Up,
                    ClientPanelDirection::Down,
                ] {
                    let label = self.layout_direction(direction);
                    let value = if layout.at_edge(direction) == Some(true) {
                        edge
                    } else {
                        interior
                    };
                    text.push_str(&format!("{label}: {value}\n"));
                }
                text
            }
            LayoutQuery::Layout => {
                let words = match self {
                    Self::EnUs => [
                        "Context",
                        "Tab",
                        "Selected",
                        "Zoom",
                        "Area",
                        "Panel",
                        "outer",
                        "terminal",
                        "displayed",
                        "Split",
                        "ratio",
                        "divider",
                    ],
                    Self::ZhCn => [
                        "上下文",
                        "标签页",
                        "选中",
                        "缩放",
                        "区域",
                        "面板",
                        "外框",
                        "终端",
                        "显示",
                        "分割",
                        "比例",
                        "分隔线",
                    ],
                };
                let yes = match self {
                    Self::EnUs => "yes",
                    Self::ZhCn => "是",
                };
                let no = match self {
                    Self::EnUs => "no",
                    Self::ZhCn => "否",
                };
                let mut text = format!(
                    "{}: {}\n{}: {}\n{}: {}\n{}: {}\n{}: {}\n",
                    words[0],
                    layout.terminal_context_id,
                    words[1],
                    layout.tab_id,
                    words[2],
                    if layout.selected { yes } else { no },
                    words[3],
                    if layout.zoomed { yes } else { no },
                    words[4],
                    self.layout_rect(Some(layout.area))
                );
                // The admitted4096-node/1MiB observation bounds this text below the8MiB stdout cap.
                for panel in &layout.panels {
                    text.push_str(&format!(
                        "{} {} | {}={} | {}={} | {}={}\n",
                        words[5],
                        panel.panel_id,
                        words[6],
                        self.layout_rect(panel.unzoomed),
                        words[7],
                        self.layout_rect(panel.terminal),
                        words[8],
                        self.layout_rect(panel.displayed)
                    ));
                }
                for split in &layout.splits {
                    text.push_str(&format!(
                        "{} {} | {}={} | {}={}\n",
                        words[9],
                        split.split_id,
                        words[10],
                        split.ratio.value(),
                        words[11],
                        self.layout_rect(split.divider)
                    ));
                }
                text
            }
        }
    }
    fn layout_rect(self, rect: Option<ClientCellRect>) -> String {
        rect.map_or_else(
            || match self {
                Self::EnUs => "hidden".into(),
                Self::ZhCn => "隐藏".into(),
            },
            |rect| format!("{},{} {}x{}", rect.x, rect.y, rect.width, rect.height),
        )
    }
    fn layout_direction(self, direction: ClientPanelDirection) -> &'static str {
        match (self, direction) {
            (Self::EnUs, ClientPanelDirection::Left) => "left",
            (Self::EnUs, ClientPanelDirection::Right) => "right",
            (Self::EnUs, ClientPanelDirection::Up) => "up",
            (Self::EnUs, ClientPanelDirection::Down) => "down",
            (Self::ZhCn, ClientPanelDirection::Left) => "左",
            (Self::ZhCn, ClientPanelDirection::Right) => "右",
            (Self::ZhCn, ClientPanelDirection::Up) => "上",
            (Self::ZhCn, ClientPanelDirection::Down) => "下",
        }
    }
}

pub(super) const HELP_LAYOUT_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel layout [<panel>] [--current] [--client <client-id>] [--timeout <ms>] [--format text|json]",
        "Read shared topology and nominal ratios headlessly when an explicit Panel is supplied without --client. JSON uses orchbee.panel.structure v1 with scope shared and no viewport geometry. Add --client to read that Windows TUI's cell geometry, focus and zoom, including inactive Tabs. An omitted target uses the caller/current Client. Reads never change selection, terminal size or control. --request-id and --detach are not read options.",
    );
pub(super) const HELP_LAYOUT_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel layout [<panel>] [--current] [--client <client-id>] [--timeout <ms>] [--format text|json]",
        "显式指定 Panel 且省略 --client 时，无头读取共享拓扑和标称比例。JSON 使用 orchbee.panel.structure v1，scope 为 shared，不包含视口几何。添加 --client 可读取该 Windows TUI 的单元格几何、焦点及缩放，支持非活动 Tab。省略目标使用调用者／当前 Client。读取不改变选择、终端尺寸或控制权。只读命令不接受 --request-id 和 --detach。",
    );
pub(super) const HELP_NEIGHBOR_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel neighbor [<panel>] [--current] --direction left|right|up|down [--client <client-id>] [--timeout <ms>] [--format text|json]",
        "Read the adjacent Panel using the TUI's unzoomed outer rectangles. A visible source without a neighbor succeeds at an edge; a hidden source returns exit 3. JSON retains the full geometry and source availability. No focus changes occur.",
    );
pub(super) const HELP_NEIGHBOR_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel neighbor [<panel>] [--current] --direction left|right|up|down [--client <client-id>] [--timeout <ms>] [--format text|json]",
        "按 TUI 的未缩放外框矩形查询相邻面板。源可见且无邻居时成功表示边缘；源隐藏时退出码为 3。JSON 保留完整几何与源可用性。不会改变焦点。",
    );
pub(super) const HELP_EDGES_EN: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel edges [<panel>] [--current] [--client <client-id>] [--timeout <ms>] [--format text|json]",
        "Read all four directional edges without changing focus. Zoom preserves unzoomed neighbors. A hidden source has no edge claim and returns exit 3. JSON retains the full layout observation.",
    );
pub(super) const HELP_EDGES_ZH: crate::localization::HelpDetails =
    crate::localization::HelpDetails::new(
        "orchbee panel edges [<panel>] [--current] [--client <client-id>] [--timeout <ms>] [--format text|json]",
        "读取四方向边缘，不改变焦点。缩放保留未缩放邻居。源隐藏时不作边缘判断，退出码为 3。JSON 保留完整布局观测。",
    );
