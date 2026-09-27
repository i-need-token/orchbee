//! Panel runtime observations keep frame, process and Client view authority separate.
use super::{
    Failure, FailureKind,
    output::{Bounded, TooLarge},
    query,
    results::Projection,
    source::{ArmedSignals, SignalWatch},
};
use crate::{invocation::Target, local_client::LocalClient, localization::Locale};
use orchbee_workbench_protocol::Snapshot;
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Instant};

#[derive(Debug)]
enum Observation {
    Unbound,
    Unavailable,
    Observed(Box<query::terminal_metadata::Read>),
}
#[derive(Debug)]
pub(super) struct Document(BTreeMap<String, Observation>);

pub(super) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    projection: &Projection,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<Option<Document>, Failure> {
    let panels: Vec<_> = match projection {
        Projection::PanelList(panels) => panels.iter().collect(),
        Projection::PanelGet(panel) => vec![&panel.row],
        _ => return Ok(None),
    };
    let mut rows = BTreeMap::new();
    for panel in panels {
        signals.check_before(deadline)?;
        let observation = match &panel.primary_terminal_id {
            None => Observation::Unbound,
            Some(id) => {
                let outcome = query::terminal_metadata::read(
                    client,
                    snapshot,
                    &Target::Id(id.clone()),
                    deadline,
                    signals.poll_fd(),
                );
                // Local I/O can report a stopped read as a transport error. Preserve
                // the query's timeout/cancellation authority on failure paths too.
                signals.check_before(deadline)?;
                match outcome {
                    Ok(read) => Observation::Observed(Box::new(read)),
                    Err(error) if error.kind == FailureKind::ServiceUnavailable => {
                        Observation::Unavailable
                    }
                    Err(error) => return Err(error),
                }
            }
        };
        signals.check_before(deadline)?;
        rows.insert(panel.panel_id.clone(), observation);
    }
    Ok(Some(Document(rows)))
}

fn stripped(title: &str) -> Option<&str> {
    let title = title
        .strip_prefix("Administrator: ")
        .unwrap_or(title)
        .trim();
    let first = title.chars().next()?;
    let rest = &title[first.len_utf8()..];
    let activity = matches!(first, '\u{2800}'..='\u{28ff}') || "·✢✳✶✻✽◐◓◑◒".contains(first);
    let title = if activity && rest.chars().next().is_none_or(char::is_whitespace) {
        rest.trim()
    } else {
        title
    };
    (!title.is_empty()).then_some(title)
}

fn reported_directory(value: &str) -> Option<String> {
    let path = if let Some(uri) = value.strip_prefix("file://") {
        if uri.contains(['?', '#', '\\']) {
            return None;
        }
        let (host, path) = uri.split_once('/')?;
        if matches!(host, "." | "..")
            || host.contains(['@', ':', '%'])
            || host.chars().any(char::is_whitespace)
        {
            return None;
        }
        let mut bytes = Vec::with_capacity(path.len());
        let mut input = path.as_bytes().iter().copied();
        while let Some(byte) = input.next() {
            bytes.push(if byte == b'%' {
                let high = char::from(input.next()?).to_digit(16)?;
                let low = char::from(input.next()?).to_digit(16)?;
                (high * 16 + low) as u8
            } else {
                byte
            });
        }
        let path = String::from_utf8(bytes).ok()?.replace('/', "\\");
        if path.as_bytes().get(1) == Some(&b':') {
            path
        } else if !host.is_empty() && !host.eq_ignore_ascii_case("localhost") {
            format!("\\\\{host}\\{path}")
        } else {
            return None;
        }
    } else {
        value.to_owned()
    };
    let parsed = std::path::Path::new(&path);
    if path.len() > 16 * 1024 || path.chars().any(char::is_control) || !parsed.is_absolute() {
        return None;
    }
    // Observed UNC directories are data, not permission to launch a Workspace there.
    // Keep their validation independent of the narrower Workbench launch-path contract.
    let Some(std::path::Component::Prefix(prefix)) = parsed.components().next() else {
        return None;
    };
    if !matches!(
        prefix.kind(),
        std::path::Prefix::Disk(_)
            | std::path::Prefix::VerbatimDisk(_)
            | std::path::Prefix::UNC(_, _)
            | std::path::Prefix::VerbatimUNC(_, _)
    ) {
        return None;
    }
    Some(path)
}

impl Observation {
    fn fields(&self) -> Value {
        let mut fields = json!({"cwd":null,"cwdSource":null,"foregroundCwd":null,"terminalTitle":null,"terminalTitleStripped":null});
        let read = match self {
            Self::Unbound => {
                fields["terminalObservation"] = json!({"state":"unbound"});
                return fields;
            }
            Self::Unavailable => {
                fields["terminalObservation"] =
                    json!({"state":"unavailable","reason":"terminal-unavailable"});
                return fields;
            }
            Self::Observed(read) => read,
        };
        let frame = &read.frame;
        let mut observation = json!({"state":"observed","frame":{
            "hostEpoch":frame.route.host_epoch.to_text(),"attachmentId":frame.route.attachment_id.to_text(),
            "sequence":frame.route.sequence.to_string(),"generation":frame.route.generation.to_string(),"revision":frame.route.revision.to_string(),
            "columns":frame.columns,"rows":frame.rows,"alternateScreen":frame.alternate_screen,"scrollbackRows":frame.scrollback_rows.to_string(),
            "title":frame.title.as_deref(),"currentDirectory":frame.current_directory.as_deref()
        },"processes":{"state":"unavailable","reason":"process-unavailable"}});
        fields["terminalTitle"] = json!(frame.title.as_deref());
        fields["terminalTitleStripped"] = json!(frame.title.as_deref().and_then(stripped));
        let reported = frame
            .current_directory
            .as_deref()
            .and_then(reported_directory);
        if let Some(cwd) = &reported {
            fields["cwd"] = json!(cwd);
            fields["cwdSource"] = json!("terminal-frame");
        }
        if let Some((source, processes)) = &read.processes {
            let view = super::processes::interpret(&processes.info);
            if reported.is_none()
                && let Some(cwd) = processes
                    .info
                    .processes
                    .iter()
                    .find(|row| row.pid == processes.info.root_pid)
                    .and_then(|row| row.current_directory.as_deref())
            {
                fields["cwd"] = json!(cwd);
                fields["cwdSource"] = json!("root-process");
            }
            fields["foregroundCwd"] = json!(
                view.foreground
                    .pid
                    .and_then(|pid| processes.info.processes.iter().find(|row| row.pid == pid))
                    .and_then(|row| row.current_directory.as_deref())
            );
            observation["processes"] = json!({"state":"observed","source":source,"result":super::results::panel_process_result(processes)});
        }
        fields["terminalObservation"] = observation;
        fields
    }
}

impl Document {
    pub(super) fn enrich(&self, result: &mut Value) {
        let mut enrich = |row: &mut Value| {
            if let Some(observation) = row["panelId"].as_str().and_then(|id| self.0.get(id)) {
                row.as_object_mut()
                    .expect("typed Panel row")
                    .extend(observation.fields().as_object().unwrap().clone());
            }
        };
        if let Some(panels) = result.get_mut("panels").and_then(Value::as_array_mut) {
            panels.iter_mut().for_each(&mut enrich);
        }
        if let Some(panel) = result.get_mut("panel") {
            enrich(panel);
        }
    }
    pub(super) fn render(&self, out: &mut Bounded, locale: Locale) -> Result<(), TooLarge> {
        out.write(format_args!(
            "{}\n",
            locale.cli_terminal_observations_heading()
        ))?;
        for (id, observation) in &self.0 {
            let fields = observation.fields();
            out.write(format_args!(
                "{}  {}  {}\n",
                id,
                super::escape::escape(fields["terminalTitleStripped"].as_str().unwrap_or("-")),
                super::escape::escape(fields["cwd"].as_str().unwrap_or("-"))
            ))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
