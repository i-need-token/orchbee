//! `agent integration` — the CLI-local provider onboarding commands. Every
//! command runs on this machine only: the ambient PATH resolves provider
//! executables, provider commands run under the bounded local runner, and no
//! Site is started or contacted. Mutations are guarded and report the
//! post-effect state they actually observed.

use serde_json::{Map, Value, json};

use super::escape::{display_width, escape};
use super::{Failure, FailureKind, Output, failure_output, output, stage_reason_details};
use crate::invocation::{AgentIntegrationCommand, OutputFormat};
use crate::localization::Locale;

#[cfg(unix)]
use crate::agent_integrations::{
    AttemptedStep, CapsuleStatus, Direction, FailureCode, InspectionDetail, Issue, MutationReport,
    PlannedStep, Refusal, SnapshotItem, State, StepKind, Verdict,
};

#[cfg(unix)]
const INTEGRATIONS_SCHEMA: &str = "orchbee.agent.integrations";
#[cfg(unix)]
const PLAN_SCHEMA: &str = "orchbee.agent.integration.plan";
#[cfg(unix)]
const MUTATION_SCHEMA: &str = "orchbee.agent.integration.mutation";

pub(crate) fn run(
    command: &AgentIntegrationCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    match run_local(command) {
        Err(failure) => failure_output(failure, Some(format), locale),
        Ok(prepared) => prepared.render(format, locale),
    }
}

#[cfg(not(unix))]
fn run_local(_command: &AgentIntegrationCommand) -> Result<Prepared, Failure> {
    Err(Failure::new(FailureKind::ServiceUnavailable)
        .details(stage_reason_details("platform", "unsupported-platform")))
}

#[cfg(unix)]
pub(super) fn run_local(command: &AgentIntegrationCommand) -> Result<Prepared, Failure> {
    let adapter_source = match command {
        AgentIntegrationCommand::List { adapter_source, .. }
        | AgentIntegrationCommand::Explain { adapter_source, .. }
        | AgentIntegrationCommand::Enable { adapter_source, .. }
        | AgentIntegrationCommand::Disable { adapter_source, .. } => adapter_source.as_deref(),
    };
    // Arm the shared termination source — SIGINT at any frontier boundary
    // settles as `cancelled` (exit 130). An install failure refuses the
    // invocation outright: no context, inspection, or effect may run
    // uncancellable.
    let signal = crate::termination_signal::TerminationSignal::install()
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let cancel: Option<std::sync::Arc<dyn crate::termination_signal::TerminationSource>> =
        Some(std::sync::Arc::new(signal)
            as std::sync::Arc<
                dyn crate::termination_signal::TerminationSource,
            >);
    let context = crate::agent_integrations::context(adapter_source, cancel).map_err(|_| {
        Failure::new(FailureKind::ServiceUnavailable)
            .details(stage_reason_details("environment", "home-unavailable"))
    })?;
    Ok(match command {
        AgentIntegrationCommand::List { provider, .. } => {
            let (items, capsule) = crate::agent_integrations::list(&context);
            let items = match provider {
                Some(id) => items
                    .into_iter()
                    .filter(|item| item.integration_id == *id)
                    .collect(),
                None => items,
            };
            Prepared::List { items, capsule }
        }
        AgentIntegrationCommand::Explain { provider, .. } => {
            let (item, detail, capsule, plan) =
                crate::agent_integrations::explain(&context, *provider);
            Prepared::Explain {
                item,
                detail,
                capsule,
                plan,
            }
        }
        AgentIntegrationCommand::Enable { provider, .. } => {
            let report = crate::agent_integrations::mutate(&context, *provider, Direction::Enable);
            Prepared::Mutation { report }
        }
        AgentIntegrationCommand::Disable { provider, .. } => {
            let report = crate::agent_integrations::mutate(&context, *provider, Direction::Disable);
            Prepared::Mutation { report }
        }
    })
}

/// One prepared integration answer — the typed result before the format
/// split, so text and JSON carry identical facts. Non-unix has no variant:
/// `run_local` always answers the unsupported-platform failure there.
#[cfg(unix)]
pub(super) enum Prepared {
    List {
        items: Vec<SnapshotItem>,
        capsule: CapsuleStatus,
    },
    Explain {
        item: SnapshotItem,
        detail: InspectionDetail,
        capsule: CapsuleStatus,
        plan: Vec<PlannedStep>,
    },
    Mutation {
        report: MutationReport,
    },
}

#[cfg(not(unix))]
enum Prepared {}

impl Prepared {
    fn render(self, format: OutputFormat, locale: Locale) -> Output {
        let too_large = || {
            failure_output(
                Failure::new(FailureKind::OutputTooLarge).details(Map::from_iter([(
                    "limitBytes".to_owned(),
                    json!(output::LIMIT),
                )])),
                Some(format),
                locale,
            )
        };
        let (status, document, text) = self.split(locale);
        match format {
            OutputFormat::Json => match output::encode_document(&document) {
                Ok(stdout) => Output {
                    status,
                    stdout,
                    stderr: String::new(),
                    finalization: None,
                },
                Err(output::TooLarge) => too_large(),
            },
            OutputFormat::Text => match output::encode_text(&text) {
                Ok(stdout) => Output {
                    status,
                    stdout,
                    stderr: String::new(),
                    finalization: None,
                },
                Err(output::TooLarge) => too_large(),
            },
        }
    }

    /// Project the typed result onto (status, machine document, localized
    /// text) — one projection, two renderings.
    #[cfg(unix)]
    fn split(self, locale: Locale) -> (u8, Value, String) {
        match self {
            Self::List { items, capsule } => {
                let complete = items.iter().all(|item| item.state != State::Unknown);
                let document = json!({
                    "schema": INTEGRATIONS_SCHEMA,
                    "version": 1,
                    "source": { "kind": "local-inspection" },
                    "result": {
                        "complete": complete,
                        "capsule": capsule_json(&capsule),
                        "integrations": items.iter().map(item_json).collect::<Vec<_>>(),
                    },
                });
                (0, document, list_text(&items, &capsule, locale))
            }
            Self::Explain {
                item,
                detail,
                capsule,
                plan,
            } => {
                let document = json!({
                    "schema": PLAN_SCHEMA,
                    "version": 1,
                    "source": { "kind": "local-inspection" },
                    "result": {
                        "provider": item.integration_id.as_str(),
                        "availability": item.availability.as_str(),
                        "managementMode": item.management_mode.as_str(),
                        "state": item.state.as_str(),
                        "issue": item.issue.map(Issue::as_str),
                        "capsule": capsule_json(&capsule),
                        "detail": detail_json(&detail),
                        "plan": plan.iter().map(planned_step_json).collect::<Vec<_>>(),
                    },
                });
                (
                    0,
                    document,
                    explain_text(&item, &detail, &capsule, &plan, locale),
                )
            }
            Self::Mutation { report } => {
                let status = mutation_status(&report);
                let document = mutation_document(&report);
                (status, document, mutation_text(&report, locale))
            }
        }
    }

    #[cfg(not(unix))]
    fn split(self, _locale: Locale) -> (u8, Value, String) {
        match self {}
    }
}

// -- projection ---------------------------------------------------------------

#[cfg(unix)]
fn item_json(item: &SnapshotItem) -> Value {
    json!({
        "integrationId": item.integration_id.as_str(),
        "availability": item.availability.as_str(),
        "managementMode": item.management_mode.as_str(),
        "state": item.state.as_str(),
        "issue": item.issue.map(Issue::as_str),
    })
}

#[cfg(unix)]
fn capsule_json(capsule: &CapsuleStatus) -> Value {
    let state = if capsule.unsupported_version.is_some() {
        "unsupported-version"
    } else if capsule.binding_failure {
        "binding-failed"
    } else if capsule.ready {
        "ready"
    } else if capsule.source_kind == "absent" {
        "source-absent"
    } else {
        "not-ready"
    };
    let (root_key, root_value) = super::config::path_detail(&capsule.root);
    let mut object = Map::from_iter([
        ("state".to_owned(), json!(state)),
        ("source".to_owned(), json!(capsule.source_kind)),
        ("supportedPayloadVersion".to_owned(), {
            json!(crate::agent_integrations::PAYLOAD_VERSION)
        }),
        (root_key.to_string(), root_value),
    ]);
    if let Some(declared) = &capsule.declared_payload_version {
        object.insert("declaredPayloadVersion".to_owned(), json!(declared));
    }
    if let Some(declared) = &capsule.unsupported_version {
        object.insert("unsupportedPayloadVersion".to_owned(), json!(declared));
    }
    if capsule.binding_failure {
        object.insert("bindingFailure".to_owned(), json!(true));
    }
    Value::Object(object)
}

#[cfg(unix)]
fn detail_json(detail: &InspectionDetail) -> Value {
    match detail {
        InspectionDetail::Cli {
            marketplace,
            plugin_present,
        } => json!({
            "kind": "provider-cli",
            "marketplace": marketplace.map(|marketplace| marketplace.as_str()),
            "pluginPresent": plugin_present,
        }),
        InspectionDetail::Codex {
            marketplace,
            plugin_present,
        } => json!({
            "kind": "provider-config",
            "marketplace": marketplace.as_str(),
            "pluginPresent": plugin_present,
        }),
        InspectionDetail::Guarded {
            target_state,
            source_ready,
        } => json!({
            "kind": "guarded-filesystem",
            "targetState": target_state.as_str(),
            "sourceReady": source_ready,
        }),
        InspectionDetail::Plain => json!({ "kind": "none" }),
    }
}

#[cfg(unix)]
fn planned_step_json(step: &PlannedStep) -> Value {
    match step {
        PlannedStep::CapsulePublish { target } => json!({
            "kind": "capsule-publish",
            "target": target.to_string_lossy(),
        }),
        PlannedStep::Command { executable, argv } => json!({
            "kind": "provider-command",
            "executable": executable,
            "argv": argv,
        }),
        PlannedStep::GuardedWrite { target } => json!({
            "kind": "guarded-write",
            "target": target.to_string_lossy(),
        }),
        PlannedStep::GuardedRemove { target } => json!({
            "kind": "guarded-remove",
            "target": target.to_string_lossy(),
        }),
    }
}

#[cfg(unix)]
fn attempted_step_json(step: &AttemptedStep) -> Value {
    let outcome = step.outcome.as_str();
    match &step.step {
        StepKind::Command { executable, argv } => json!({
            "kind": "provider-command",
            "executable": executable,
            "argv": argv,
            "outcome": outcome,
        }),
        StepKind::Filesystem { operation, path } => json!({
            "kind": "filesystem",
            "operation": operation,
            "target": path.to_string_lossy(),
            "outcome": outcome,
        }),
    }
}

#[cfg(unix)]
fn mutation_document(report: &MutationReport) -> Value {
    let (outcome, reason) = match report.verdict {
        Verdict::Completed => ("completed", None),
        Verdict::Refused(refusal) => ("refused", Some(refusal.as_str())),
        Verdict::Failed(code) => ("failed", Some(code.as_str())),
        Verdict::Unknown => ("unknown", None),
        Verdict::Cancelled => ("cancelled", None),
    };
    let mut result = Map::from_iter([
        ("invocationId".to_owned(), json!(invocation_id())),
        ("provider".to_owned(), json!(report.integration_id.as_str())),
        ("direction".to_owned(), json!(report.direction.as_str())),
        ("outcome".to_owned(), json!(outcome)),
        (
            "attempted".to_owned(),
            Value::Array(report.attempted.iter().map(attempted_step_json).collect()),
        ),
        (
            "observed".to_owned(),
            json!({
                "item": report.observed.as_ref().map(item_json),
                "provenance": report.provenance.as_str(),
            }),
        ),
    ]);
    if let Some(reason) = reason {
        result.insert("failure".to_owned(), json!({ "code": reason }));
    }
    if !report.residuals.is_empty() {
        result.insert(
            "residue".to_owned(),
            Value::Array(
                report
                    .residuals
                    .iter()
                    .map(|path| json!(path.to_string_lossy()))
                    .collect(),
            ),
        );
    }
    json!({
        "schema": MUTATION_SCHEMA,
        "version": 1,
        "source": { "kind": "local-inspection" },
        "result": Value::Object(result),
    })
}

/// A diagnostic label — process-unique, never an idempotence key.
#[cfg(unix)]
fn invocation_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("cli-{}-{nanos}", std::process::id())
}

/// The frozen exit map: pre-effect refusals keep their class codes; a
/// post-effect verdict is determinate-failed (9), observed-conflict (3), or
/// undetermined (8).
#[cfg(unix)]
pub(crate) fn mutation_status(report: &MutationReport) -> u8 {
    match report.verdict {
        Verdict::Completed => 0,
        Verdict::Refused(refusal) => match refusal {
            Refusal::Conflict | Refusal::StaleSide => 3,
            Refusal::ManualActionRequired
            | Refusal::AgentUnavailable
            | Refusal::CapsuleUnavailable
            | Refusal::CapsuleUnsupportedVersion => 4,
            Refusal::Deadline => 7,
            // An unclassifiable pre-effect state is a determinate failure —
            // the run refuses without ever falling into an else-vector.
            Refusal::InspectionFailed => 9,
        },
        // A post-effect conflict is a determinate post-effect failure — the
        // pre-effect family 3 is reserved for refusals before any effect.
        Verdict::Failed(code) => match code {
            FailureCode::Conflict | FailureCode::OperationFailed => 9,
            FailureCode::ManualActionRequired => 4,
            FailureCode::VerificationFailed
            | FailureCode::TimedOut
            | FailureCode::OutputTooLarge
            | FailureCode::Leaked => 8,
        },
        Verdict::Unknown => 8,
        Verdict::Cancelled => 130,
    }
}

// -- text rendering -----------------------------------------------------------

#[cfg(unix)]
fn bounded_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|header| display_width(header)).collect();
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(display_width(cell));
        }
    }
    let mut out = String::new();
    let render_row = |cells: &[String], out: &mut String| {
        for (index, cell) in cells.iter().enumerate() {
            if index > 0 {
                out.push_str("  ");
            }
            out.push_str(cell);
            if index + 1 < cells.len() {
                for _ in display_width(cell)..widths[index] {
                    out.push(' ');
                }
            }
        }
        out.push('\n');
    };
    render_row(
        &headers
            .iter()
            .map(|header| (*header).to_owned())
            .collect::<Vec<_>>(),
        &mut out,
    );
    for row in rows {
        render_row(row, &mut out);
    }
    out
}

#[cfg(unix)]
fn item_row(item: &SnapshotItem) -> Vec<String> {
    vec![
        item.integration_id.as_str().to_owned(),
        item.availability.as_str().to_owned(),
        item.management_mode.as_str().to_owned(),
        item.state.as_str().to_owned(),
        item.issue.map(Issue::as_str).unwrap_or("-").to_owned(),
    ]
}

#[cfg(unix)]
fn list_text(items: &[SnapshotItem], capsule: &CapsuleStatus, locale: Locale) -> String {
    let labels = locale.cli_labels();
    let mut out = String::new();
    out.push_str(&locale.cli_integration_capsule_line(&capsule_summary(capsule)));
    let rows: Vec<Vec<String>> = items.iter().map(item_row).collect();
    out.push_str(&bounded_table(&labels.integration_headers, &rows));
    out
}

#[cfg(unix)]
fn capsule_summary(capsule: &CapsuleStatus) -> String {
    let state = if capsule.unsupported_version.is_some() {
        "unsupported-version"
    } else if capsule.binding_failure {
        "binding-failed"
    } else if capsule.ready {
        "ready"
    } else if capsule.source_kind == "absent" {
        "source-absent"
    } else {
        "not-ready"
    };
    format!("{state} ({})", capsule.source_kind)
}

#[cfg(unix)]
fn explain_text(
    item: &SnapshotItem,
    detail: &InspectionDetail,
    capsule: &CapsuleStatus,
    plan: &[PlannedStep],
    locale: Locale,
) -> String {
    let mut out = String::new();
    out.push_str(&locale.cli_integration_explain_header(
        item.integration_id.as_str(),
        item.state.as_str(),
        item.availability.as_str(),
        item.issue.map(Issue::as_str).unwrap_or("-"),
        capsule_summary(capsule).as_str(),
    ));
    if let Some(line) = detail_line(detail) {
        out.push_str(&line);
    }
    if plan.is_empty() {
        out.push_str(locale.cli_integration_plan_empty());
    } else {
        out.push_str(locale.cli_integration_plan_header());
        for (index, step) in plan.iter().enumerate() {
            out.push_str(&format!("  {}. {}\n", index + 1, planned_step_text(step)));
        }
    }
    out
}

#[cfg(unix)]
fn detail_line(detail: &InspectionDetail) -> Option<String> {
    match detail {
        InspectionDetail::Cli {
            marketplace,
            plugin_present,
        } => Some(format!(
            "marketplace: {}  plugin-present: {}\n",
            marketplace
                .map(|marketplace| marketplace.as_str())
                .unwrap_or("unknown"),
            plugin_present
                .map(|present| if present { "yes" } else { "no" })
                .unwrap_or("unknown"),
        )),
        InspectionDetail::Codex {
            marketplace,
            plugin_present,
        } => Some(format!(
            "marketplace: {}  plugin-present: {}\n",
            marketplace.as_str(),
            if *plugin_present { "yes" } else { "no" },
        )),
        InspectionDetail::Guarded {
            target_state,
            source_ready,
        } => Some(format!(
            "target-state: {}  source-ready: {}\n",
            target_state.as_str(),
            if *source_ready { "yes" } else { "no" },
        )),
        InspectionDetail::Plain => None,
    }
}

#[cfg(unix)]
fn planned_step_text(step: &PlannedStep) -> String {
    match step {
        PlannedStep::CapsulePublish { target } => {
            format!("publish adapter capsule → {}", target.display())
        }
        PlannedStep::Command { executable, argv } => {
            let mut line = escape(executable);
            for arg in argv {
                line.push(' ');
                line.push_str(&escape(arg));
            }
            line
        }
        PlannedStep::GuardedWrite { target } => {
            format!("guarded write → {}", target.display())
        }
        PlannedStep::GuardedRemove { target } => {
            format!("guarded remove → {}", target.display())
        }
    }
}

#[cfg(unix)]
fn mutation_text(report: &MutationReport, locale: Locale) -> String {
    let provider = report.integration_id.as_str();
    let direction = report.direction.as_str();
    let mut out = match report.verdict {
        Verdict::Completed => locale.cli_integration_completed(direction, provider),
        Verdict::Refused(refusal) => {
            locale.cli_integration_refused(direction, provider, refusal.as_str())
        }
        Verdict::Failed(code) => locale.cli_integration_failed(
            direction,
            provider,
            code.as_str(),
            report.observed.map(|item| item.state.as_str()),
        ),
        Verdict::Unknown => locale.cli_integration_unknown(direction, provider),
        Verdict::Cancelled => locale.cli_integration_cancelled(direction, provider),
    };
    for step in &report.attempted {
        out.push_str(&format!(
            "  {} {} → {}\n",
            locale.cli_integration_attempted_word(),
            step_text(&step.step),
            step.outcome.as_str(),
        ));
    }
    if !report.residuals.is_empty() {
        out.push_str(locale.cli_integration_residue_header());
        for residual in &report.residuals {
            out.push_str(&format!("  {}\n", residual.display()));
        }
    }
    out
}

#[cfg(unix)]
fn step_text(step: &StepKind) -> String {
    match step {
        StepKind::Command { executable, argv } => {
            let mut line = escape(executable);
            for arg in argv {
                line.push(' ');
                line.push_str(&escape(arg));
            }
            line
        }
        StepKind::Filesystem { operation, path } => {
            format!("{operation} {}", path.display())
        }
    }
}
