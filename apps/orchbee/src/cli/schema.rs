//! Embedded JSON Schema dictionary. Every document ships inside the binary so
//! dictionary reads never touch the filesystem, the network, or a Site.
//! Explicit file publication belongs to the separate schema exporter.

use serde_json::{Map, Value, json};

use super::{Failure, FailureKind, Output, failure_output, output, target_details};
use crate::invocation::{OutputFormat, Resource};
use crate::localization::Locale;

pub(crate) const FAILURE_ID: &str = "orchbee.cli.failure";
pub(crate) const SOURCE_FAILURE_ID: &str = "orchbee.cli.failure.source";
/// The dedicated `worktree list` failure envelope: the command's own detail
/// shapes (directory resource, worktree-list stage, shared Git bound origin)
/// without touching the generic failure schema.
pub(crate) const WORKTREE_FAILURE_ID: &str = "orchbee.cli.failure.worktree";
/// The dedicated Agent read failure envelope: post-admission exchange and
/// semantic failures plus the signal-cancelled kind, kept off the generic
/// failure schema.
pub(crate) const AGENT_FAILURE_ID: &str = "orchbee.cli.failure.agent";
/// The dedicated launch-profile failure envelope: the catalog read's typed
/// states and exchange faults plus the mutations' profile-resolution
/// settlements — the generic schema's resource and candidate grammars are
/// closed without it.
pub(crate) const LAUNCH_PROFILE_FAILURE_ID: &str = "orchbee.cli.failure.launch-profile";
/// The dedicated Workspace Close lifecycle envelope: `close-status` and
/// `close-resume` stages, the resume rejection vocabulary, and the
/// outcome-unknown request echo — kept off the generic failure schema.
pub(crate) const CLOSE_FAILURE_ID: &str = "orchbee.cli.failure.close";
pub(crate) const MUTATION_RESULT_ID: &str = "orchbee.mutation.result";
pub(crate) const PANEL_RENAME_RESULT_ID: &str = "orchbee.panel.rename";
pub(crate) const WORKSPACE_MOVE_RESULT_ID: &str = "orchbee.workspace.move";
pub(crate) const WORKSPACE_MOVE_FAILURE_ID: &str = "orchbee.cli.failure.workspace-move";
pub(crate) const TAB_MOVE_RESULT_ID: &str = "orchbee.tab.move";
pub(crate) const TAB_MOVE_FAILURE_ID: &str = "orchbee.cli.failure.tab-move";
pub(crate) const PANEL_INPUT_RESULT_ID: &str = "orchbee.panel.input";
pub(crate) const PROJECT_RENAME_RESULT_ID: &str = "orchbee.project.rename";

/// `schema` id → embedded document, in dictionary order.
const DOCUMENTS: &[(&str, &str)] = &[
    (
        "orchbee.popup.result",
        include_str!("schemas/orchbee.popup.result.json"),
    ),
    (
        "orchbee.agent.view",
        include_str!("schemas/orchbee.agent.view.json"),
    ),
    (
        "orchbee.machine",
        include_str!("schemas/orchbee.machine.json"),
    ),
    (
        "orchbee.update",
        include_str!("schemas/orchbee.update.json"),
    ),
    (
        "orchbee.plugin.logs",
        include_str!("schemas/orchbee.plugin.logs.json"),
    ),
    (
        "orchbee.plugin.provision",
        include_str!("schemas/orchbee.plugin.provision.json"),
    ),
    (
        "orchbee.channel.show",
        include_str!("schemas/orchbee.channel.show.json"),
    ),
    (
        "orchbee.plugin.runtime",
        include_str!("schemas/orchbee.plugin.runtime.json"),
    ),
    (
        "orchbee.plugin.registry",
        include_str!("schemas/orchbee.plugin.registry.json"),
    ),
    (
        "orchbee.terminal.title",
        include_str!("schemas/orchbee.terminal.title.json"),
    ),
    (
        "orchbee.notification.show",
        include_str!("schemas/orchbee.notification.show.json"),
    ),
    (
        "orchbee.site.configuration",
        include_str!("schemas/orchbee.site.configuration.json"),
    ),
    (
        "orchbee.config.reset-keys",
        include_str!("schemas/orchbee.config.reset-keys.json"),
    ),
    (
        "orchbee.panel.terminal-create",
        include_str!("schemas/orchbee.panel.terminal-create.json"),
    ),
    (
        "orchbee.site.agent-manifests",
        include_str!("schemas/orchbee.site.agent-manifests.json"),
    ),
    (
        "orchbee.panel.agent-report",
        include_str!("schemas/orchbee.panel.agent-report.json"),
    ),
    (
        "orchbee.panel.metadata",
        include_str!("schemas/orchbee.panel.metadata.json"),
    ),
    (
        "orchbee.workspace.metadata",
        include_str!("schemas/orchbee.workspace.metadata.json"),
    ),
    (
        "orchbee.cli.failure.workbench-query",
        include_str!("schemas/orchbee.cli.failure.workbench-query.json"),
    ),
    (
        "orchbee.cli.failure.client-query",
        include_str!("schemas/orchbee.cli.failure.client-query.json"),
    ),
    (
        "orchbee.layout.apply",
        include_str!("schemas/orchbee.layout.apply.json"),
    ),
    (
        "orchbee.cli.failure.layout-apply",
        include_str!("schemas/orchbee.cli.failure.layout-apply.json"),
    ),
    (
        "orchbee.layout.export",
        include_str!("schemas/orchbee.layout.export.json"),
    ),
    (
        "orchbee.cli.failure.layout",
        include_str!("schemas/orchbee.cli.failure.layout.json"),
    ),
    (
        "orchbee.panel.move",
        include_str!("schemas/orchbee.panel.move.json"),
    ),
    (
        WORKSPACE_MOVE_RESULT_ID,
        include_str!("schemas/orchbee.workspace.move.json"),
    ),
    (
        WORKSPACE_MOVE_FAILURE_ID,
        include_str!("schemas/orchbee.cli.failure.workspace-move.json"),
    ),
    (
        "orchbee.session.list",
        include_str!("schemas/orchbee.session.list.json"),
    ),
    (
        "orchbee.session.delete",
        include_str!("schemas/orchbee.session.delete.json"),
    ),
    (
        "orchbee.terminal.control.input",
        include_str!("schemas/orchbee.terminal.control.input.json"),
    ),
    (
        "orchbee.terminal.stream",
        include_str!("schemas/orchbee.terminal.stream.json"),
    ),
    (
        "orchbee.agent.changes",
        include_str!("schemas/orchbee.agent.changes.json"),
    ),
    (
        "orchbee.status.client",
        include_str!("schemas/orchbee.status.client.json"),
    ),
    (
        "orchbee.status.server",
        include_str!("schemas/orchbee.status.server.json"),
    ),
    (
        "orchbee.terminal.output",
        include_str!("schemas/orchbee.terminal.output.json"),
    ),
    (
        TAB_MOVE_RESULT_ID,
        include_str!("schemas/orchbee.tab.move.json"),
    ),
    (
        TAB_MOVE_FAILURE_ID,
        include_str!("schemas/orchbee.cli.failure.tab-move.json"),
    ),
    (
        PANEL_INPUT_RESULT_ID,
        include_str!("schemas/orchbee.panel.input.json"),
    ),
    (
        "orchbee.cli.failure.machine-route",
        include_str!("schemas/orchbee.cli.failure.machine-route.json"),
    ),
    (
        "orchbee.cli.failure.configuration",
        include_str!("schemas/orchbee.cli.failure.configuration.json"),
    ),
    (
        "orchbee.workspace.creation",
        include_str!("schemas/orchbee.workspace.creation.json"),
    ),
    (
        "orchbee.creation.result",
        include_str!("schemas/orchbee.creation.result.json"),
    ),
    (
        "orchbee.panel.swap",
        include_str!("schemas/orchbee.panel.swap.json"),
    ),
    (
        PANEL_RENAME_RESULT_ID,
        include_str!("schemas/orchbee.panel.rename.json"),
    ),
    (
        "orchbee.panel.layout",
        include_str!("schemas/orchbee.panel.layout.json"),
    ),
    (
        "orchbee.cli.failure.schema-export",
        include_str!("schemas/orchbee.cli.failure.schema-export.json"),
    ),
    (
        "orchbee.client.view",
        include_str!("schemas/orchbee.client.view.json"),
    ),
    (
        "orchbee.client.control",
        include_str!("schemas/orchbee.client.control.json"),
    ),
    (
        "orchbee.cli.failure.client-control",
        include_str!("schemas/orchbee.cli.failure.client-control.json"),
    ),
    (
        "orchbee.workspace.group-close",
        include_str!("schemas/orchbee.workspace.group-close.json"),
    ),
    (
        "orchbee.worktree.operation",
        include_str!("schemas/orchbee.worktree.operation.json"),
    ),
    (
        "orchbee.site.stop",
        include_str!("schemas/orchbee.site.stop.json"),
    ),
    (
        "orchbee.agent.start",
        include_str!("schemas/orchbee.agent.start.json"),
    ),
    (
        "orchbee.cli.failure.agent-start",
        include_str!("schemas/orchbee.cli.failure.agent-start.json"),
    ),
    (
        "orchbee.cli.failure.agent-runtime",
        include_str!("schemas/orchbee.cli.failure.agent-runtime.json"),
    ),
    (
        "orchbee.panel.process-info",
        include_str!("schemas/orchbee.panel.process-info.json"),
    ),
    (
        "orchbee.cli.failure.process-info",
        include_str!("schemas/orchbee.cli.failure.process-info.json"),
    ),
    (
        "orchbee.agent.doctor",
        include_str!("schemas/orchbee.agent.doctor.json"),
    ),
    (
        "orchbee.agent.explain",
        include_str!("schemas/orchbee.agent.explain.json"),
    ),
    (
        "orchbee.agent.get",
        include_str!("schemas/orchbee.agent.get.json"),
    ),
    (
        "orchbee.agent.integration.mutation",
        include_str!("schemas/orchbee.agent.integration.mutation.json"),
    ),
    (
        "orchbee.agent.integration.plan",
        include_str!("schemas/orchbee.agent.integration.plan.json"),
    ),
    (
        "orchbee.agent.integrations",
        include_str!("schemas/orchbee.agent.integrations.json"),
    ),
    (
        "orchbee.agent.list",
        include_str!("schemas/orchbee.agent.list.json"),
    ),
    (
        "orchbee.agent.prompt",
        include_str!("schemas/orchbee.agent.prompt.json"),
    ),
    (
        "orchbee.agent.resolve",
        include_str!("schemas/orchbee.agent.resolve.json"),
    ),
    (
        "orchbee.agent.names",
        include_str!("schemas/orchbee.agent.names.json"),
    ),
    (
        "orchbee.agent.rename",
        include_str!("schemas/orchbee.agent.rename.json"),
    ),
    (
        "orchbee.cli.failure.agent-prompt",
        include_str!("schemas/orchbee.cli.failure.agent-prompt.json"),
    ),
    (
        "orchbee.cli.failure.agent-names",
        include_str!("schemas/orchbee.cli.failure.agent-names.json"),
    ),
    (
        "orchbee.agent.wait",
        include_str!("schemas/orchbee.agent.wait.json"),
    ),
    (
        "orchbee.api.schema",
        include_str!("schemas/orchbee.api.schema.json"),
    ),
    (
        "orchbee.api.snapshot",
        include_str!("schemas/orchbee.api.snapshot.json"),
    ),
    (FAILURE_ID, include_str!("schemas/orchbee.cli.failure.json")),
    (
        AGENT_FAILURE_ID,
        include_str!("schemas/orchbee.cli.failure.agent.json"),
    ),
    (
        CLOSE_FAILURE_ID,
        include_str!("schemas/orchbee.cli.failure.close.json"),
    ),
    (
        LAUNCH_PROFILE_FAILURE_ID,
        include_str!("schemas/orchbee.cli.failure.launch-profile.json"),
    ),
    (
        SOURCE_FAILURE_ID,
        include_str!("schemas/orchbee.cli.failure.source.json"),
    ),
    (
        WORKTREE_FAILURE_ID,
        include_str!("schemas/orchbee.cli.failure.worktree.json"),
    ),
    (
        "orchbee.config.check",
        include_str!("schemas/orchbee.config.check.json"),
    ),
    (
        "orchbee.launch-profile.get",
        include_str!("schemas/orchbee.launch-profile.get.json"),
    ),
    (
        "orchbee.launch-profile.list",
        include_str!("schemas/orchbee.launch-profile.list.json"),
    ),
    (
        MUTATION_RESULT_ID,
        include_str!("schemas/orchbee.mutation.result.json"),
    ),
    (
        "orchbee.panel.current",
        include_str!("schemas/orchbee.panel.current.json"),
    ),
    (
        "orchbee.panel.get",
        include_str!("schemas/orchbee.panel.get.json"),
    ),
    (
        "orchbee.panel.list",
        include_str!("schemas/orchbee.panel.list.json"),
    ),
    (
        "orchbee.project.get",
        include_str!("schemas/orchbee.project.get.json"),
    ),
    (
        "orchbee.project.list",
        include_str!("schemas/orchbee.project.list.json"),
    ),
    (
        PROJECT_RENAME_RESULT_ID,
        include_str!("schemas/orchbee.project.rename.json"),
    ),
    (
        "orchbee.site.diagnose",
        include_str!("schemas/orchbee.site.diagnose.json"),
    ),
    (
        "orchbee.site.diagnose.windows",
        include_str!("schemas/orchbee.site.diagnose.windows.json"),
    ),
    (
        "orchbee.site.start",
        include_str!("schemas/orchbee.site.start.json"),
    ),
    (
        "orchbee.status",
        include_str!("schemas/orchbee.status.json"),
    ),
    (
        "orchbee.tab.get",
        include_str!("schemas/orchbee.tab.get.json"),
    ),
    (
        "orchbee.tab.list",
        include_str!("schemas/orchbee.tab.list.json"),
    ),
    (
        "orchbee.terminal.get",
        include_str!("schemas/orchbee.terminal.get.json"),
    ),
    (
        "orchbee.terminal.history",
        include_str!("schemas/orchbee.terminal.history.json"),
    ),
    (
        "orchbee.terminal.list",
        include_str!("schemas/orchbee.terminal.list.json"),
    ),
    (
        "orchbee.terminal.read",
        include_str!("schemas/orchbee.terminal.read.json"),
    ),
    (
        "orchbee.terminal.selection",
        include_str!("schemas/orchbee.terminal.selection.json"),
    ),
    (
        "orchbee.terminal.link-resolve",
        include_str!("schemas/orchbee.terminal.link-resolve.json"),
    ),
    (
        "orchbee.terminal.clear",
        include_str!("schemas/orchbee.terminal.clear.json"),
    ),
    (
        "orchbee.terminal.link-activate",
        include_str!("schemas/orchbee.terminal.link-activate.json"),
    ),
    (
        "orchbee.terminal.copy-motion",
        include_str!("schemas/orchbee.terminal.copy-motion.json"),
    ),
    (
        "orchbee.terminal.copy-search",
        include_str!("schemas/orchbee.terminal.copy-search.json"),
    ),
    (
        "orchbee.terminal.send",
        include_str!("schemas/orchbee.terminal.send.json"),
    ),
    (
        "orchbee.terminal.wait-output",
        include_str!("schemas/orchbee.terminal.wait-output.json"),
    ),
    (
        "orchbee.workspace.close-status",
        include_str!("schemas/orchbee.workspace.close-status.json"),
    ),
    (
        "orchbee.workspace.get",
        include_str!("schemas/orchbee.workspace.get.json"),
    ),
    (
        "orchbee.workspace.list",
        include_str!("schemas/orchbee.workspace.list.json"),
    ),
    (
        "orchbee.worktree.query",
        include_str!("schemas/orchbee.worktree.query.json"),
    ),
    (
        "orchbee.worktree.list",
        include_str!("schemas/orchbee.worktree.list.json"),
    ),
];

pub(crate) fn embedded_source() -> Value {
    json!({ "kind": "embedded" })
}

fn document(schema_id: &str) -> Option<&'static str> {
    DOCUMENTS
        .iter()
        .find(|(id, _document)| *id == schema_id)
        .map(|(_id, document)| *document)
}

/// `api schema` always answers a machine document on success, in either
/// format. Its failures keep the caller's selected format like every other
/// command: text/default keeps stdout empty, json emits the failure envelope.
pub(crate) fn answer(schema_id: Option<&str>, format: OutputFormat, locale: Locale) -> Output {
    let caller = Some(format);
    let result = match schema_id {
        Some(schema_id) => {
            let Some(document) = document(schema_id) else {
                return failure_output(
                    Failure::new(FailureKind::NotFound)
                        .details(target_details(Resource::Schema, schema_id))
                        .source(embedded_source()),
                    caller,
                    locale,
                );
            };
            let document = match serde_json::from_str::<Value>(document) {
                Ok(document) => document,
                Err(_) => {
                    return failure_output(
                        Failure::new(FailureKind::InternalFailure),
                        caller,
                        locale,
                    );
                }
            };
            json!({
                "schema": {
                    "schemaId": schema_id,
                    "document": document,
                }
            })
        }
        None => {
            let mut schemas = Map::new();
            for (schema_id, document) in DOCUMENTS {
                match serde_json::from_str::<Value>(document) {
                    Ok(document) => {
                        schemas.insert((*schema_id).to_owned(), document);
                    }
                    Err(_) => {
                        return failure_output(
                            Failure::new(FailureKind::InternalFailure),
                            caller,
                            locale,
                        );
                    }
                }
            }
            json!({ "schemas": Value::Object(schemas) })
        }
    };
    let document = json!({
        "schema": "orchbee.api.schema",
        // Includes prompt-preserving terminal clear receipts.
        "version": 130,
        "source": embedded_source(),
        "result": result,
    });
    match output::encode_document(&document) {
        Ok(stdout) => Output {
            status: 0,
            stdout,
            stderr: String::new(),
            finalization: None,
        },
        Err(output::TooLarge) => failure_output(
            Failure::new(FailureKind::OutputTooLarge)
                .details(Map::from_iter([(
                    "limitBytes".to_owned(),
                    json!(output::LIMIT),
                )]))
                .source(embedded_source()),
            caller,
            locale,
        ),
    }
}

#[cfg(test)]
mod tests;
