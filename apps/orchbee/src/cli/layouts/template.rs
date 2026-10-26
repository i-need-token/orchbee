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
                    WorkbenchPanelSplitDirection::Above | WorkbenchPanelSplitDirection::Left => {
                        unreachable!("admitted portable direction")
                    }
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum InputError {
    InvalidLayout,
    TooLarge,
    UnsupportedVersion(u32),
}
impl InputError {
    pub(super) const fn reason(self) -> &'static str {
        match self {
            Self::InvalidLayout => "invalid-layout",
            Self::TooLarge => "layout-too-large",
            Self::UnsupportedVersion(_) => "layout-version-1-required",
        }
    }
}
impl From<&'static str> for InputError {
    fn from(reason: &'static str) -> Self {
        match reason {
            "layout-too-large" => Self::TooLarge,
            _ => Self::InvalidLayout,
        }
    }
}

pub(super) fn parse(bytes: &[u8]) -> Result<Node, InputError> {
    if bytes.len() > MAX_BYTES {
        return Err(InputError::TooLarge);
    }
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    let mut fields: Object = serde_json::from_slice(bytes).map_err(|_| "invalid-layout")?;
    let current_export = fields.contains("schema");
    let root = if current_export {
        let version = fields.decode::<u32>("version")?;
        if fields.string("schema")? != "orchbee.layout.export" {
            return Err(InputError::InvalidLayout);
        }
        if version != 1 {
            return Err(InputError::UnsupportedVersion(version));
        }
        let mut source = Object::parse(&fields.take("source")?)?;
        if source.string("kind")? != "layout-observation" {
            return Err(InputError::InvalidLayout);
        }
        source.decode::<orchbee_workbench_protocol::SessionId>("sessionId")?;
        source.decode::<orchbee_workbench_protocol::WorkbenchRevision>("revision")?;
        let client =
            source.decode::<Option<orchbee_workbench_protocol::WorkbenchClientId>>("clientId")?;
        let view = source
            .decode::<Option<orchbee_workbench_protocol::WorkbenchRevision>>("viewRevision")?;
        let shared = match source.string("scope")?.as_str() {
            "shared" if client.is_none() && view.is_none() => true,
            "client" if client.is_some() && view.is_some() => false,
            _ => return Err(InputError::InvalidLayout),
        };
        source.finish()?;
        let mut result = Object::parse(&fields.take("result")?)?;
        let layout = result.take("layout")?;
        result.finish()?;
        fields.finish()?;
        export_description(Object::parse(&layout)?, shared)?
    } else if fields.contains("root") {
        description(fields)?
    } else {
        return node(fields, 1, &mut 0, false).map_err(Into::into);
    };
    node(Object::parse(&root)?, 1, &mut 0, current_export).map_err(Into::into)
}

fn export_description(mut fields: Object, shared: bool) -> Result<Box<RawValue>, &'static str> {
    use orchbee_workbench_protocol::{
        WorkbenchPanelId, WorkbenchTabId, WorkbenchTerminalContextId, WorkbenchWorkspaceId,
    };
    fields.decode::<WorkbenchWorkspaceId>("workspaceId")?;
    fields.decode::<WorkbenchTerminalContextId>("terminalContextId")?;
    fields.decode::<WorkbenchTabId>("tabId")?;
    let focus = fields.decode::<Option<WorkbenchPanelId>>("focusedPanelId")?;
    let zoom = fields.decode::<Option<bool>>("zoomed")?;
    if (shared && (focus.is_some() || zoom.is_some())) || (!shared && zoom.is_none()) {
        return Err("invalid-layout");
    }
    let root = fields.take("root")?;
    fields.finish()?;
    Ok(root)
}

fn description(mut fields: Object) -> Result<Box<RawValue>, &'static str> {
    let root = fields.take("root")?;
    for name in [
        "workspaceId",
        "terminalContextId",
        "tabId",
        "focusedPanelId",
    ] {
        fields.optional_string(name)?;
    }
    if fields.contains("zoomed") {
        fields.decode::<Option<bool>>("zoomed")?;
    }
    fields.finish()?;
    Ok(root)
}

fn node(
    mut fields: Object,
    depth: usize,
    count: &mut usize,
    current_export: bool,
) -> Result<Node, &'static str> {
    if depth > super::model::MAX_DEPTH {
        return Err("layout-too-large");
    }
    let kind = fields.string("type")?;
    match kind.as_str() {
        "panel" => {
            if current_export {
                fields.require(&["panelId", "rightClick", "primary", "children"])?;
            }
            *count += 1;
            if *count > super::model::MAX_PANELS {
                return Err("layout-too-large");
            }
            if current_export {
                fields.decode::<orchbee_workbench_protocol::WorkbenchPanelId>("panelId")?;
            } else {
                fields.optional_string("panelId")?;
            }
            let right_click = if fields.contains("rightClick") {
                fields.decode("rightClick")?
            } else {
                Default::default()
            };
            if fields.contains("primary") {
                let primary = member::parse(
                    Object::parse(&fields.take("primary")?)?,
                    true,
                    current_export,
                )?;
                let children = if fields.contains("children") {
                    let children: Vec<Box<RawValue>> = fields.decode("children")?;
                    if children.len() > orchbee_workbench_protocol::MAXIMUM_LAYOUT_CHILDREN {
                        return Err("layout-too-large");
                    }
                    children
                        .iter()
                        .map(|raw| member::parse(Object::parse(raw)?, true, current_export))
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
                let primary = member::parse(fields, false, false)?;
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
            if !ratio.is_finite() || (current_export && !(0.1..=0.9).contains(&ratio)) {
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
                first: Box::new(node(
                    Object::parse(&first)?,
                    depth + 1,
                    count,
                    current_export,
                )?),
                second: Box::new(node(
                    Object::parse(&second)?,
                    depth + 1,
                    count,
                    current_export,
                )?),
            })
        }
        _ => Err("invalid-layout"),
    }
}

#[cfg(test)]
mod tests;
