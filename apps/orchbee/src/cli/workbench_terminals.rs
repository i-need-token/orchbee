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
use serde_json::Value;
use std::{collections::BTreeMap, time::Instant};

use super::terminal_observation::Observation;

#[derive(Debug)]
pub(super) struct Document(BTreeMap<String, Observation>);

pub(super) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    projection: &Projection,
    current: Option<&super::current_presentation::Resolved>,
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
        let terminal = if let Some(member) = current {
            if panel.panel_id != member.panel_id.as_str() {
                return Err(Failure::new(FailureKind::InternalFailure));
            }
            member
                .binding
                .as_ref()
                .map(|binding| binding.terminal_id.as_str())
        } else {
            panel.primary_terminal_id.as_deref()
        };
        let observation = match terminal {
            None => Observation::Unbound,
            Some(id) => {
                let outcome = query::terminal_metadata::read(
                    client,
                    snapshot,
                    &Target::Id(id.to_owned()),
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
                super::escape::unquoted(fields["terminalTitleStripped"].as_str().unwrap_or("-")),
                super::escape::unquoted(fields["cwd"].as_str().unwrap_or("-"))
            ))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
