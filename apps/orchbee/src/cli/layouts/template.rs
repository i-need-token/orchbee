//! Complete portable input admission and deterministic creation order, before capabilities.
mod object;
use object::Object;
use orchbee_client_server_protocol::{
    MAXIMUM_LAUNCH_SELECTION_ARGUMENTS, MAXIMUM_LAUNCH_SELECTION_BYTES,
};
use orchbee_workbench_protocol::{
    ClientSplitPath, ClientSplitRatio, LaunchEnvironment, PanelLabelUpdate,
    WorkbenchPanelSplitDirection,
};
use serde_json::value::RawValue;

pub(super) const MAX_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Leaf {
    pub(super) label: PanelLabelUpdate,
    pub(super) cwd: Option<String>,
    pub(super) command: Option<Vec<String>>,
    pub(super) environment: LaunchEnvironment,
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
    pub(super) leaves: Vec<Leaf>,
    pub(super) splits: Vec<Split>,
}

impl Node {
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
        let mut plan = Plan {
            leaves: vec![self.first_leaf().clone()],
            splits: Vec::new(),
        };
        walk(self, 0, &mut vec![], &mut plan);
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
            || fields.decode::<u32>("version")? != 1
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
            let label = PanelLabelUpdate::parse(fields.optional_string("label")?.as_deref())
                .map_err(|_| "invalid-layout")?;
            let cwd = fields.optional_string("cwd")?;
            if cwd
                .as_ref()
                .is_some_and(|path| path.is_empty() || path.len() > 4096)
            {
                return Err("invalid-layout");
            }
            let command = if let Some(raw) = fields.remove("command") {
                serde_json::from_str::<Option<Vec<String>>>(raw.get())
                    .map_err(|_| "invalid-layout")?
            } else {
                None
            };
            if let Some(argv) = &command
                && (argv.first().is_none_or(String::is_empty)
                    || argv.len() > MAXIMUM_LAUNCH_SELECTION_ARGUMENTS + 1
                    || argv.iter().any(|arg| arg.contains('\0'))
                    || argv.iter().map(String::len).sum::<usize>() > MAXIMUM_LAUNCH_SELECTION_BYTES
                    || argv
                        .iter()
                        .flat_map(|arg| arg.bytes())
                        .map(|b| match b {
                            0..=31 => 6,
                            b'"' | b'\\' => 2,
                            _ => 1,
                        })
                        .sum::<usize>()
                        > MAXIMUM_LAUNCH_SELECTION_BYTES)
            {
                return Err("invalid-layout");
            }
            let environment = if let Some(raw) = fields.remove("env") {
                let mut env = Object::parse(&raw)?;
                let names = env.names();
                let entries = names
                    .into_iter()
                    .map(|name| {
                        let value = env.string(&name)?;
                        Ok((name, value))
                    })
                    .collect::<Result<Vec<_>, &'static str>>()?;
                LaunchEnvironment::new(entries).map_err(|_| "invalid-layout")?
            } else {
                LaunchEnvironment::empty()
            };
            fields.finish()?;
            Ok(Node::Panel(Leaf {
                label,
                cwd,
                command,
                environment,
            }))
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
