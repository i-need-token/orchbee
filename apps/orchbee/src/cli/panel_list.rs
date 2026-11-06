//! Human Panel lists join the same Agent rows as agent list; JSON retains its detailed owner.
use super::{
    Failure, FailureKind, agent_projection,
    console_interrupt::SignalWatch,
    output::{Bounded, TooLarge},
    query,
    results::{self, PanelRow, Projection},
};
use crate::cli_text::CliText;
use crate::terminal_title::{display_name, program_name};
use crate::{
    invocation::{QueryCommand, Target},
    local_client::LocalClient,
    localization::Locale,
};
use orchbee_workbench_protocol::Snapshot;
use std::{collections::BTreeMap, time::Instant};
#[derive(Debug)]
struct Row {
    number: usize,
    handle: String,
    title: String,
    agents: String,
    state: Option<&'static str>,
    cwd: Option<String>,
}
#[derive(Debug)]
pub(super) struct Document {
    rows: Vec<Row>,
}

pub(super) fn read(
    client: &mut LocalClient,
    snapshot: &Snapshot,
    panels: &[PanelRow],
    locale: Locale,
    deadline: Instant,
    signals: &mut impl SignalWatch,
) -> Result<Document, Failure> {
    let Projection::PanelList(all) = results::project(
        &QueryCommand::PanelList {
            tab: None,
            workspace: None,
        },
        snapshot,
    )?
    else {
        return Err(Failure::new(FailureKind::InternalFailure));
    };
    let handles = super::handles::stable(
        "panel",
        &all.iter().map(|p| p.panel_id.as_str()).collect::<Vec<_>>(),
    );
    let handles = all
        .iter()
        .zip(handles)
        .map(|(p, h)| (p.panel_id.as_str(), h))
        .collect::<BTreeMap<_, _>>();
    let projection = agent_projection::read(client, snapshot, false, deadline, signals.poll_fd())?;
    let mut rows = Vec::with_capacity(panels.len());
    for (index, panel) in panels.iter().enumerate() {
        signals.check_before(deadline)?;
        let agents = projection
            .rows
            .iter()
            .filter(|r| r.in_panel(&panel.panel_id))
            .collect::<Vec<_>>();
        let mut providers = Vec::new();
        for row in &agents {
            if !providers.contains(&row.provider()) {
                providers.push(row.provider());
            }
        }
        let state = agents.iter().map(|r| r.status()).max_by_key(|s| match *s {
            "blocked" => 6,
            "error" => 5,
            "done" => 4,
            "working" => 3,
            "idle" => 2,
            "ended" => 1,
            _ => 0,
        });
        let mut terminal_title = None;
        let mut cwd = None;
        if index < 64
            && let Some(terminal) = &panel.primary_terminal_id
        {
            let observation = query::terminal_metadata::read(
                client,
                snapshot,
                &Target::Id(terminal.clone()),
                deadline,
                signals.poll_fd(),
            );
            signals.check_before(deadline)?;
            match observation {
                Ok(observation) => {
                    // The shared projection's physical rows and this optional frame
                    // must refer to the same Host incarnation when one is known.
                    if agents
                        .iter()
                        .filter_map(|a| a.terminal())
                        .filter(|a| {
                            a.binding.presentation_id.as_str() == panel.primary_presentation_id
                        })
                        .any(|a| {
                            a.binding.host_epoch.as_str()
                                != observation.frame.route.host_epoch.to_text()
                        })
                    {
                        return Err(agent_projection::failure(FailureKind::Rejected, "changed"));
                    }
                    let fields =
                        super::terminal_observation::Observation::Observed(Box::new(observation))
                            .fields();
                    cwd = fields["cwd"].as_str().map(str::to_owned);
                    terminal_title = fields["terminalTitle"].as_str().map(str::to_owned);
                }
                Err(error) if error.kind == FailureKind::ServiceUnavailable => {}
                Err(error) => return Err(error),
            }
        }
        rows.push(Row {
            number: index + 1,
            handle: handles
                .get(panel.panel_id.as_str())
                .expect("complete Panel membership")
                .clone(),
            title: panel_title(panel, terminal_title.as_deref(), locale),
            agents: if providers.is_empty() {
                "-".into()
            } else {
                providers.join(", ")
            },
            state,
            cwd,
        });
    }
    agent_projection::confirm(
        client,
        snapshot,
        &projection,
        false,
        deadline,
        signals.poll_fd(),
    )?;
    signals.check_before(deadline)?;
    Ok(Document { rows })
}

/// The Panel list TITLE: the Panel's own label, else the primary terminal's title named by the
/// shared terminal title rule, then its launched program, else the Presentation label.
pub(super) fn panel_title(
    panel: &PanelRow,
    terminal_title: Option<&str>,
    locale: Locale,
) -> String {
    panel
        .custom_label
        .clone()
        .or_else(|| terminal_title.and_then(|title| display_name(title, locale)))
        .or_else(|| {
            panel
                .launch_program
                .as_deref()
                .and_then(|program| program_name(program, locale))
        })
        .unwrap_or_else(|| panel.display_label.clone())
}

impl Document {
    pub(super) fn render(&self, out: &mut Bounded, locale: Locale) -> Result<(), TooLarge> {
        let headers = CliText::new(locale).cli_panel_table_headers();
        let handle = self
            .rows
            .iter()
            .map(|r| r.handle.len())
            .max()
            .unwrap_or(0)
            .max(crate::cli_text::escape::display_width(headers[1]));
        let spare = 80usize.saturating_sub(4 + handle + 9 + 10);
        let title = 14.min(spare / 3);
        let agent = 12.min((spare - title) / 2);
        let cwd = spare - title - agent;
        super::present::compact_table(
            out,
            &headers,
            &[4, handle, title, agent, 9, cwd],
            &self.rows,
            |r| {
                vec![
                    r.number.to_string(),
                    r.handle.clone(),
                    super::present::truncate_terminal_cell(&r.title, title),
                    super::present::truncate_terminal_cell(&r.agents, agent),
                    r.state
                        .map_or("-", |s| CliText::new(locale).cli_projection_status(s))
                        .into(),
                    super::present::truncate_terminal_cell(r.cwd.as_deref().unwrap_or("-"), cwd),
                ]
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_panel_table_is_one_localized_safe_eighty_cell_table() {
        let document = Document {
            rows: vec![Row {
                number: 1,
                handle: "a".repeat(32),
                title: "\u{1b}[31m窗口\n\u{202e}name".into(),
                agents: "codex, grok-build, claude-code".into(),
                state: Some("working"),
                cwd: Some("C:\\项目\\very-long-working-directory".into()),
            }],
        };
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let bytes =
                super::super::output::render_text(|out| document.render(out, locale)).unwrap();
            let text = String::from_utf8(bytes).unwrap();
            let lines = text.lines().collect::<Vec<_>>();
            assert_eq!(lines.len(), 2);
            assert!(lines[0].contains(CliText::new(locale).cli_panel_table_headers()[2]));
            assert!(
                lines
                    .iter()
                    .all(|line| crate::cli_text::escape::display_width(line) <= 80),
                "{text}"
            );
            assert!(!text.contains('\u{1b}'));
            assert!(!text.contains('\u{202e}'));
            assert!(text.contains(&"a".repeat(32)));
            assert!(text.contains(CliText::new(locale).cli_projection_status("working")));
        }
    }
}
