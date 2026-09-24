//! Localized layout observations; machine geometry and identifiers remain locale independent.
use super::Locale;
use crate::invocation::LayoutQuery;
use orchbee_workbench_protocol::{ClientCellRect, ClientPanelDirection, ClientPanelLayout};

impl Locale {
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

pub(super) const HELP_LAYOUT_EN: &str = "Usage: orchbee panel layout [panel] [--panel <panel> | --current] [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]\n\nRead actual cell layout from one Windows TUI, including an inactive or unvisited Tab. This never selects a Tab or resizes a terminal. Hidden rectangles are null; inactive geometry is a projection into the sampled viewport.\n";
pub(super) const HELP_LAYOUT_ZH: &str = "用法: orchbee panel layout [panel] [--panel <panel> | --current] [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]\n\n读取一个 Windows TUI 的实际单元格布局，支持非活动或未访问的标签页。不会选择标签页或调整终端尺寸。隐藏矩形为 null；非活动几何表示在采样视口中的投影。\n";
pub(super) const HELP_NEIGHBOR_EN: &str = "Usage: orchbee panel neighbor [panel] [--panel <panel> | --current] --direction left|right|up|down [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]\n\nRead the adjacent Panel using the TUI's unzoomed outer rectangles. A visible source without a neighbor succeeds at an edge; a hidden source returns exit 3. JSON retains the full geometry and source availability. No focus changes occur.\n";
pub(super) const HELP_NEIGHBOR_ZH: &str = "用法: orchbee panel neighbor [panel] [--panel <panel> | --current] --direction left|right|up|down [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]\n\n按 TUI 的未缩放外框矩形查询相邻面板。源可见且无邻居时成功表示边缘；源隐藏时退出码为 3。JSON 保留完整几何与源可用性。不会改变焦点。\n";
pub(super) const HELP_EDGES_EN: &str = "Usage: orchbee panel edges [panel] [--panel <panel> | --current] [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]\n\nRead all four directional edges without changing focus. Zoom preserves unzoomed neighbors. A hidden source has no edge claim and returns exit 3. JSON retains the full layout observation.\n";
pub(super) const HELP_EDGES_ZH: &str = "用法: orchbee panel edges [panel] [--panel <panel> | --current] [--client <client-id>] [--request-id <operation-id>] [--detach] [--timeout <ms>] [--format text|json]\n\n读取四方向边缘，不改变焦点。缩放保留未缩放邻居。源隐藏时不作边缘判断，退出码为 3。JSON 保留完整布局观测。\n";
