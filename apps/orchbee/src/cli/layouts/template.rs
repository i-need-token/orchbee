//! Complete portable input admission and deterministic creation order, before capabilities.
mod member;
mod object;
pub(super) use member::Member;
use object::Object;
use orchbee_workbench_protocol::{
    ClientSplitPath, ClientSplitRatio, PanelOrientation, PanelRightClickTarget,
    WorkbenchLayoutNode, WorkbenchLayoutTemplate, WorkbenchPanelSplitDirection,
};
use serde_json::value::RawValue;

pub(super) const MAX_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Leaf {
    pub(super) primary: Member,
    pub(super) children: Vec<Member>,
    pub(super) right_click: PanelRightClickTarget,
}
impl Leaf {
    pub(super) fn members(&self) -> impl Iterator<Item = &Member> {
        std::iter::once(&self.primary).chain(&self.children)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum Node {
    Panel(Leaf),
    Split {
        direction: WorkbenchPanelSplitDirection,
        ratio: ClientSplitRatio,
        first: Box<Self>,
        second: Box<Self>,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Split {
    pub(super) source: usize,
    pub(super) created: usize,
    pub(super) direction: WorkbenchPanelSplitDirection,
    pub(super) ratio: ClientSplitRatio,
    pub(super) path: ClientSplitPath,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct Plan {
    pub(super) layout: WorkbenchLayoutTemplate,
    pub(super) leaves: Vec<Leaf>,
    pub(super) splits: Vec<Split>,
}

impl Node {
    fn structural(&self) -> WorkbenchLayoutNode {
        match self {
            Self::Panel(leaf) => WorkbenchLayoutNode::Panel {
                primary: leaf.primary.structural(),
                children: leaf.children.iter().map(Member::structural).collect(),
                right_click: leaf.right_click,
            },
            Self::Split {
                direction,
                first,
                second,
                ..
            } => WorkbenchLayoutNode::Split {
                orientation: match direction {
                    WorkbenchPanelSplitDirection::Right => PanelOrientation::Horizontal,
                    WorkbenchPanelSplitDirection::Below => PanelOrientation::Vertical,
                    _ => unreachable!("admitted portable direction"),
                },
                children: Box::new([first.structural(), second.structural()]),
            },
        }
    }

    fn first_leaf(&self) -> &Leaf {
        match self {
            Self::Panel(leaf) => leaf,
            Self::Split { first, .. } => first.first_leaf(),
        }
    }

    pub(super) fn plan(&self) -> Plan {
        fn walk(node: &Node, source: usize, path: &mut Vec<u8>, plan: &mut Plan) {
            if let Node::Split {
                direction,
                ratio,
                first,
                second,
            } = node
            {
                let created = plan.leaves.len();
                plan.leaves.push(second.first_leaf().clone());
                plan.splits.push(Split {
                    source,
                    created,
                    direction: *direction,
                    ratio: *ratio,
                    path: ClientSplitPath::new(path.clone()).expect("admitted layout depth"),
                });
                path.push(0);
                walk(first, source, path, plan);
                path.pop();
                path.push(1);
                walk(second, created, path, plan);
                path.pop();
            }
        }
        fn number(node: &mut Node, next: &mut usize) {
            match node {
                Node::Panel(leaf) => {
                    *next += 1;
                    if leaf.primary.automatic_name {
                        leaf.primary.name =
                            orchbee_workbench_protocol::WorkbenchLabel::parse(&next.to_string())
                                .expect("bounded numeric label");
                    }
                }
                Node::Split { first, second, .. } => {
                    number(first, next);
                    number(second, next);
                }
            }
        }
        let mut normalized = self.clone();
        number(&mut normalized, &mut 0);
        let mut plan = Plan {
            layout: WorkbenchLayoutTemplate::new(normalized.structural())
                .expect("admitted portable bounds"),
            leaves: vec![normalized.first_leaf().clone()],
            splits: Vec::new(),
        };
        walk(&normalized, 0, &mut vec![], &mut plan);
        plan
    }
}

pub(super) fn parse(bytes: &[u8]) -> Result<Node, &'static str> {
    if bytes.len() > MAX_BYTES {
        return Err("layout-too-large");
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let mut fields: Object = serde_json::from_slice(bytes).map_err(|_| "invalid-layout")?;
    let root = if fields.contains("schema") {
        if fields.string("schema")? != "orchbee.layout.export"
            || !matches!(fields.decode::<u32>("version")?, 1 | 2)
        {
            return Err("invalid-layout");
        }
        let mut source = Object::parse(&fields.take("source")?)?;
        if source.string("kind")? != "layout-observation" {
            return Err("invalid-layout");
        }
        for name in ["siteId", "revision", "clientId", "viewRevision"] {
            source.string(name)?;
        }
        source.finish()?;
        let mut result = Object::parse(&fields.take("result")?)?;
        let layout = result.take("layout")?;
        result.finish()?;
        fields.finish()?;
        description(Object::parse(&layout)?)?
    } else if fields.contains("root") {
        description(fields)?
    } else {
        return node(fields, 1, &mut 0);
    };
    node(Object::parse(&root)?, 1, &mut 0)
}

fn description(mut fields: Object) -> Result<Box<RawValue>, &'static str> {
    let root = fields.take("root")?;
    for name in [
        "workspaceId",
        "terminalContextId",
        "tabId",
        "focusedPanelId",
        "workspace_id",
        "tab_id",
        "focused_pane_id",
    ] {
        fields.optional_string(name)?;
    }
    if fields.contains("zoomed") {
        fields.decode::<bool>("zoomed")?;
    }
    fields.finish()?;
    Ok(root)
}

fn node(mut fields: Object, depth: usize, count: &mut usize) -> Result<Node, &'static str> {
    if depth > super::model::MAX_DEPTH {
        return Err("layout-too-large");
    }
    let kind = fields.string("type")?;
    match kind.as_str() {
        "panel" | "pane" => {
            *count += 1;
            if *count > super::model::MAX_PANELS {
                return Err("layout-too-large");
            }
            if fields.contains("panelId") && fields.contains("pane_id") {
                return Err("invalid-layout");
            }
            fields.optional_string("panelId")?;
            fields.optional_string("pane_id")?;
            let right_click = if fields.contains("rightClick") {
                fields.decode("rightClick")?
            } else {
                Default::default()
            };
            if fields.contains("primary") {
                let primary = member::parse(Object::parse(&fields.take("primary")?)?, true)?;
                let children = if fields.contains("children") {
                    let children: Vec<Box<RawValue>> = fields.decode("children")?;
                    if children.len() > orchbee_workbench_protocol::MAXIMUM_LAYOUT_CHILDREN {
                        return Err("layout-too-large");
                    }
                    children
                        .iter()
                        .map(|raw| member::parse(Object::parse(raw)?, true))
                        .collect::<Result<Vec<_>, _>>()?
                } else {
                    Vec::new()
                };
                fields.finish()?;
                Ok(Node::Panel(Leaf {
                    primary,
                    children,
                    right_click,
                }))
            } else {
                let primary = member::parse(fields, false)?;
                Ok(Node::Panel(Leaf {
                    primary,
                    children: Vec::new(),
                    right_click,
                }))
            }
        }
        "split" => {
            let direction = match fields.string("direction")?.as_str() {
                "right" => WorkbenchPanelSplitDirection::Right,
                "down" => WorkbenchPanelSplitDirection::Below,
                _ => return Err("invalid-layout"),
            };
            let ratio = fields.decode::<f64>("ratio")?;
            if !ratio.is_finite() {
                return Err("invalid-layout");
            }
            let ratio =
                ClientSplitRatio::new(ratio.clamp(0.1, 0.9)).map_err(|_| "invalid-layout")?;
            let first = fields.take("first")?;
            let second = fields.take("second")?;
            fields.finish()?;
            Ok(Node::Split {
                direction,
                ratio,
                first: Box::new(node(Object::parse(&first)?, depth + 1, count)?),
                second: Box::new(node(Object::parse(&second)?, depth + 1, count)?),
            })
        }
        _ => Err("invalid-layout"),
    }
}

#[cfg(test)]
mod tests;
