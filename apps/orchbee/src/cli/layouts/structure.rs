//! Shared topology operations resolve only canonical identities, never a virtual viewport.
use crate::cli_text::CliText;
mod ratio;
use crate::{
    cli::{
        Failure, FailureKind, Output,
        console_interrupt::{ArmedSignals, Finish, SignalWatch},
        failure_output, output, query, results,
    },
    invocation::{ClientLayoutTarget, ClientNavigationTarget, LayoutQuery, OutputFormat, Resource},
    localization::Locale,
};
use orchbee_workbench_protocol::{
    ClientSplitIdentity, ClientSplitPath, PanelNode, Snapshot, Tab, TerminalContext,
    WorkbenchPanelId, WorkbenchRequestId, WorkbenchSplitRatio,
};
pub(in crate::cli) use ratio::submit as submit_ratio;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

pub(in crate::cli) struct Resolved<'a> {
    pub(in crate::cli) context: &'a TerminalContext,
    pub(in crate::cli) tab: &'a Tab,
    pub(in crate::cli) panel: WorkbenchPanelId,
}

pub(in crate::cli) fn resolve<'a>(
    snapshot: &'a Snapshot,
    target: &ClientLayoutTarget,
) -> Result<Resolved<'a>, Failure> {
    match target {
        ClientLayoutTarget::Tab(target) => {
            let (context, tab) = results::resolve_or_fail(
                snapshot.terminal_contexts.iter().flat_map(|context| {
                    context.tabs.iter().enumerate().map(move |(index, tab)| {
                        (
                            tab.tab_id.as_str(),
                            tab.display_label(index),
                            (context, tab),
                        )
                    })
                }),
                target,
                Resource::Tab,
            )?;
            let panel = results::panel_ids_in(&tab.root)
                .first()
                .cloned()
                .cloned()
                .ok_or_else(|| Failure::new(FailureKind::NotFound))?;
            Ok(Resolved {
                context,
                tab,
                panel,
            })
        }
        ClientLayoutTarget::Panel(Some(target)) => {
            let (panel, context, tab) = results::resolve_or_fail(
                results::panel_entries(snapshot)
                    .into_iter()
                    .map(|(panel, context, tab)| (panel.as_str(), "", (panel, context, tab))),
                target,
                Resource::Panel,
            )?;
            Ok(Resolved {
                context,
                tab,
                panel: panel.clone(),
            })
        }
        ClientLayoutTarget::Panel(None) => Err(Failure::new(FailureKind::NotFound)),
    }
}

pub(in crate::cli) fn split(
    root: &PanelNode,
    path: &ClientSplitPath,
) -> Result<(ClientSplitIdentity, WorkbenchSplitRatio), Failure> {
    let mut node = root;
    for index in path.indices() {
        let PanelNode::Split { children, .. } = node else {
            return Err(super::failure(FailureKind::NotFound, "layout-unavailable"));
        };
        node = &children[usize::from(*index)].node;
    }
    let PanelNode::Split {
        split_id,
        children,
        ratio,
        ..
    } = node
    else {
        return Err(super::failure(FailureKind::NotFound, "layout-unavailable"));
    };
    Ok((
        ClientSplitIdentity {
            split_id: split_id.clone(),
            first_slot_id: children[0].slot_id.clone(),
            second_slot_id: children[1].slot_id.clone(),
        },
        *ratio,
    ))
}

pub(in crate::cli) fn explicit(target: &ClientNavigationTarget) -> bool {
    matches!(
        target,
        ClientNavigationTarget::Layout {
            target: Some(_),
            query: LayoutQuery::Layout
        } | ClientNavigationTarget::SetSplitRatio {
            target: ClientLayoutTarget::Tab(_) | ClientLayoutTarget::Panel(Some(_)),
            ..
        }
    )
}

pub(in crate::cli) fn run(
    target: &ClientNavigationTarget,
    operation_id: Option<&WorkbenchRequestId>,
    timeout_ms: u32,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if let Err(failure) =
        crate::cli::execution::admit(crate::cli::execution::Capability::ClientControl)
    {
        return crate::cli::failure_output(failure, Some(format), locale);
    }
    let deadline = Instant::now() + Duration::from_millis(u64::from(timeout_ms));
    let mut signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    let mut result = execute(target, operation_id, deadline, &mut signals);
    let mut finalization = None;
    match signals.finish() {
        Ok(Finish::Clear) => {}
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            if let Ok((document, status)) = &mut result {
                document["result"]["observerFailure"] = json!("cancelled");
                if *status != 8 {
                    *status = 130;
                }
            } else {
                result = Err(Failure::new(FailureKind::Cancelled));
            }
        }
        Err(_) if result.is_err() => {}
        Err(_) => {
            if let Ok((document, status)) = &mut result {
                document["result"]["observerFailure"] = json!("internal-failure");
                if *status == 0 {
                    *status = 1;
                }
            }
        }
    }
    let mut answer = match result {
        Err(error) => failure_output(error, Some(format), locale),
        Ok((document, status)) => {
            let encoded = if format == OutputFormat::Json {
                output::encode_document(&document)
            } else {
                output::encode_text(&CliText::new(locale).cli_shared_layout(&document))
            };
            match encoded {
                Ok(stdout) => Output {
                    status,
                    stdout,
                    stderr: String::new(),
                    finalization: None,
                },
                Err(_) => failure_output(
                    Failure::new(FailureKind::OutputTooLarge),
                    Some(format),
                    locale,
                ),
            }
        }
    };
    answer.finalization = finalization;
    answer
}

fn execute(
    target: &ClientNavigationTarget,
    operation_id: Option<&WorkbenchRequestId>,
    deadline: Instant,
    signals: &mut ArmedSignals,
) -> Result<(Value, u8), Failure> {
    signals.check_before(deadline)?;
    let (mut client, snapshot) = query::stream_snapshot(deadline, signals.poll_fd())?;
    signals.check_before(deadline)?;
    match target {
        ClientNavigationTarget::Layout {
            target: Some(target),
            query: LayoutQuery::Layout,
        } => {
            let resolved = resolve(&snapshot, &ClientLayoutTarget::Panel(Some(target.clone())))?;
            Ok((
                json!({
                    "schema":"orchbee.panel.structure", "version":1,
                    "source":results::snapshot_source(&snapshot),
                    "result":{"scope":"shared", "terminalContextId":resolved.context.terminal_context_id,
                        "tabId":resolved.tab.tab_id,"sourcePanelId":resolved.panel,
                        "root":resolved.tab.root,"observerFailure":null}
                }),
                0,
            ))
        }
        ClientNavigationTarget::SetSplitRatio {
            target,
            path,
            ratio,
        } => {
            let resolved = resolve(&snapshot, target)?;
            let (identity, previous) = split(&resolved.tab.root, path)?;
            let (effect, outcome) = submit_ratio(
                &mut client,
                &snapshot,
                &resolved.context.terminal_context_id,
                &resolved.tab.tab_id,
                identity,
                previous,
                *ratio,
                operation_id.cloned(),
                deadline,
                signals.poll_fd(),
            )?;
            let status = outcome
                .as_ref()
                .err()
                .map_or(0, |failure| failure.kind.exit_code());
            Ok((
                json!({"schema":"orchbee.layout.ratio","version":1,
                "source":results::snapshot_source(&snapshot),
                "result":{"scope":"shared","effect":effect,"observerFailure":null}}),
                status,
            ))
        }
        _ => Err(Failure::new(FailureKind::InternalFailure)),
    }
}

#[cfg(test)]
mod tests;
