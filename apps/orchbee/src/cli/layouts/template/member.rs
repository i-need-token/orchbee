//! Strict launch intent admission for each primary or child member.
use super::object::Object;
use orchbee_client_server_protocol::{
    MAXIMUM_LAUNCH_SELECTION_ARGUMENTS, MAXIMUM_LAUNCH_SELECTION_BYTES,
};
use orchbee_workbench_protocol::{
    LaunchEnvironment, PanelLabelUpdate, WorkbenchLabel, WorkbenchLayoutPresentation,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::cli::layouts) struct Member {
    pub(in crate::cli::layouts) name: WorkbenchLabel,
    pub(in crate::cli::layouts) automatic_name: bool,
    pub(in crate::cli::layouts) start: bool,
    pub(in crate::cli::layouts) label: PanelLabelUpdate,
    pub(in crate::cli::layouts) cwd: Option<String>,
    pub(in crate::cli::layouts) command: Option<Vec<String>>,
    pub(in crate::cli::layouts) environment: LaunchEnvironment,
}
impl Member {
    pub(in crate::cli::layouts) fn structural(&self) -> WorkbenchLayoutPresentation {
        WorkbenchLayoutPresentation {
            label: self.name.clone(),
            custom_label: self.label.custom_label().cloned(),
        }
    }
}
pub(in crate::cli::layouts) fn parse(
    mut fields: Object,
    structured: bool,
    current_export: bool,
) -> Result<Member, &'static str> {
    if current_export {
        fields.require(&[
            "presentationId",
            "name",
            "start",
            "label",
            "cwd",
            "command",
            "env",
        ])?;
    }
    let (name, start) = if structured {
        if current_export {
            fields
                .decode::<orchbee_workbench_protocol::TerminalPresentationId>("presentationId")?;
        } else {
            fields.optional_string("presentationId")?;
        }
        (
            fields.decode::<WorkbenchLabel>("name")?,
            fields.decode::<bool>("start")?,
        )
    } else {
        (
            WorkbenchLabel::parse("1").map_err(|_| "invalid-layout")?,
            true,
        )
    };
    let label = if fields.contains("label") {
        fields.decode::<Option<String>>("label")?
    } else {
        None
    };
    if current_export && let Some(label) = &label {
        orchbee_workbench_protocol::PanelCustomLabel::parse(label).map_err(|_| "invalid-layout")?;
    }
    let label = PanelLabelUpdate::parse(label.as_deref()).map_err(|_| "invalid-layout")?;
    let cwd = fields.optional_string("cwd")?;
    if cwd
        .as_ref()
        .is_some_and(|path| path.is_empty() || path.len() > 4096)
    {
        return Err("invalid-layout");
    }
    if current_export && let Some(cwd) = &cwd {
        orchbee_workbench_protocol::WorkbenchDirectoryPath::parse(cwd)
            .map_err(|_| "invalid-layout")?;
    }
    let command = if let Some(raw) = fields.remove("command") {
        serde_json::from_str::<Option<Vec<String>>>(raw.get()).map_err(|_| "invalid-layout")?
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
    if current_export && let Some(argv) = &command {
        orchbee_client_server_protocol::LaunchSelection::new(argv[0].clone(), argv[1..].to_vec())
            .map_err(|_| "invalid-layout")?;
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
    if current_export && !environment.entries().is_empty() {
        return Err("invalid-layout");
    }
    if !start && (cwd.is_some() || command.is_some() || !environment.entries().is_empty()) {
        return Err("invalid-layout");
    }
    Ok(Member {
        automatic_name: !structured,
        name,
        start,
        label,
        cwd,
        command,
        environment,
    })
}
