//! Shared terminal frame and process projection for Panel and Agent queries.
use super::query;
use serde_json::{Value, json};

#[derive(Debug)]
pub(super) enum Observation {
    Unbound,
    Unavailable,
    Observed(Box<query::terminal_metadata::Read>),
}
use crate::terminal_title::stripped;

use orchbee_local_runtime::windows_reported_directory as reported_directory;

impl Observation {
    pub(super) fn fields(&self) -> Value {
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

pub(super) fn render(
    out: &mut super::output::Bounded,
    fields: &Value,
    locale: crate::localization::Locale,
) -> Result<(), super::output::TooLarge> {
    out.write(format_args!(
        "{}\n{}  {}\n",
        locale.cli_agent_terminal_heading(),
        super::escape::unquoted(fields["terminalTitleStripped"].as_str().unwrap_or("-")),
        super::escape::unquoted(fields["cwd"].as_str().unwrap_or("-")),
    ))?;
    if let Some(metadata) = fields.get("metadata") {
        out.write(format_args!(
            "{}\n{}  {}\n",
            locale.cli_panel_metadata_heading(),
            super::escape::unquoted(metadata["presentation"]["title"].as_str().unwrap_or("-")),
            super::escape::unquoted(
                metadata["presentation"]["displayAgent"]
                    .as_str()
                    .unwrap_or("-")
            )
        ))?;
        for key in ["tokens", "stateLabels"] {
            let values = if key == "tokens" {
                &metadata[key]
            } else {
                &metadata["presentation"][key]
            };
            if let Some(values) = values.as_object() {
                for (key, value) in values {
                    out.write(format_args!(
                        "{}={}\n",
                        super::escape::unquoted(key),
                        super::escape::unquoted(value.as_str().expect("typed metadata value"))
                    ))?;
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
