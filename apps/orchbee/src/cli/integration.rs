//! `agent integration` — provider onboarding through the local Session. Every command
//! declares activation: it converges the selected local Session toward readiness, then runs one
//! Server-authorized job on the Session's Worker, which resolves provider executables from its own
//! `PATH` and home directory. Mutations are guarded and report the post-effect state they
//! actually observed.

use crate::cli_text::CliText;
use serde_json::{Map, Value, json};

use super::{Failure, FailureKind, Output, failure_output, output, stage_reason_details};
#[cfg(any(unix, windows))]
use crate::cli_text::escape::{display_width, escape};
use crate::invocation::{AgentIntegrationCommand, OutputFormat};
use crate::localization::Locale;

#[cfg(any(unix, windows))]
use crate::agent_integrations::{
    AttemptedStep, CapsuleStatus, FailureCode, InspectionDetail, Issue, MutationReport,
    PlannedStep, Refusal, SnapshotItem, State, StepKind, Verdict,
};

#[cfg(any(unix, windows))]
const INTEGRATIONS_SCHEMA: &str = "orchbee.agent.integrations";
#[cfg(any(unix, windows))]
const PLAN_SCHEMA: &str = "orchbee.agent.integration.plan";
#[cfg(any(unix, windows))]
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

#[cfg(not(any(unix, windows)))]
fn run_local(_command: &AgentIntegrationCommand) -> Result<Prepared, Failure> {
    Err(Failure::new(FailureKind::ServiceUnavailable)
        .details(stage_reason_details("platform", "unsupported-platform")))
}

/// Session activation shares the `server start` budget; the job then runs under the contract's
/// 40-second ceiling plus settlement of its final poll and cancellation.
#[cfg(any(unix, windows))]
const ACTIVATION_BUDGET: std::time::Duration = std::time::Duration::from_secs(20);
#[cfg(any(unix, windows))]
const JOB_BUDGET: std::time::Duration = std::time::Duration::from_secs(50);

#[cfg(any(unix, windows))]
fn run_local(command: &AgentIntegrationCommand) -> Result<Prepared, Failure> {
    if matches!(
        command,
        AgentIntegrationCommand::List {
            outdated_only: true,
            ..
        }
    ) {
        super::execution::admit(super::execution::Capability::IntegrationOutdated)?;
    }
    // No handler is armed during activation, so a termination signal ends it by its default
    // disposition, exactly like `server start`.
    let started = std::time::Instant::now();
    super::server::activate(started + ACTIVATION_BUDGET)?;
    run_job(command, started + ACTIVATION_BUDGET + JOB_BUDGET)
}

/// Runs `command` as one job on the activated Session's Worker. The shared termination source is
/// armed before the job exists: SIGINT then registers cooperative cancellation, and a cancelled
/// mutation settles as `cancelled` (exit 130) through the job's own report. An install failure
/// refuses before any Session exchange.
#[cfg(any(unix, windows))]
pub(super) fn run_job(
    command: &AgentIntegrationCommand,
    deadline: std::time::Instant,
) -> Result<Prepared, Failure> {
    use crate::agent_integrations::IntegrationOperation as Operation;
    use crate::termination_signal::TerminationSource;
    let signal = crate::termination_signal::TerminationSignal::install()
        .map_err(|_| Failure::new(FailureKind::InternalFailure))?;
    let (operation, adapter_source) = match command {
        AgentIntegrationCommand::List {
            provider,
            adapter_source,
            ..
        } => (
            Operation::List {
                provider: *provider,
            },
            adapter_source,
        ),
        AgentIntegrationCommand::Explain {
            provider,
            adapter_source,
        } => (
            Operation::Explain {
                provider: *provider,
            },
            adapter_source,
        ),
        AgentIntegrationCommand::Enable {
            provider,
            adapter_source,
        } => (
            Operation::Enable {
                provider: *provider,
            },
            adapter_source,
        ),
        AgentIntegrationCommand::Disable {
            provider,
            adapter_source,
        } => (
            Operation::Disable {
                provider: *provider,
            },
            adapter_source,
        ),
    };
    let mutating = matches!(
        operation,
        Operation::Enable { .. } | Operation::Disable { .. }
    );
    let (mut client, snapshot) = super::query::stream_snapshot(deadline, None)?;
    if TerminationSource::requested(&signal) {
        return Err(Failure::new(FailureKind::Cancelled));
    }
    let request = crate::agent_integrations::IntegrationRequest {
        operation,
        adapter_source: adapter_source.clone(),
        budget_milliseconds: None,
    };
    let end = client.agent_integration_job(
        &snapshot.session_id,
        request,
        deadline,
        || super::query::request_id().ok(),
        |pause| {
            std::thread::sleep(pause);
            TerminationSource::requested(&signal)
        },
    );
    prepare(settle(end, mutating)?, command)
}

/// Projects how the Worker job ended onto the CLI failure vocabulary. A mutation that may have
/// started is never reported as a definite refusal.
#[cfg(any(unix, windows))]
fn settle(
    end: crate::local_client::AgentIntegrationJobEnd,
    mutating: bool,
) -> Result<crate::agent_integrations::IntegrationReport, Failure> {
    use crate::local_client::AgentIntegrationJobEnd as End;
    use orchbee_workbench_protocol::AgentIntegrationFailure as Reason;
    let failure = |kind, reason| {
        Failure::new(kind).details(stage_reason_details("agent-integration", reason))
    };
    match end {
        End::Completed { report, .. } => Ok(*report),
        End::Failed { reason, started } => {
            let name = match reason {
                Reason::Denied => "denied",
                Reason::SessionMismatch => "session-mismatch",
                Reason::InvalidRequest => "invalid-request",
                Reason::WorkerUnavailable => "worker-unavailable",
                Reason::WorkerChanged => "worker-changed",
                Reason::Busy => "busy",
                Reason::JobUnknown => "job-unknown",
                Reason::Abandoned => "abandoned",
                Reason::Malformed => "malformed",
            };
            Err(match reason {
                _ if mutating && (started || reason == Reason::Abandoned) => {
                    failure(FailureKind::OutcomeUnknown, name)
                }
                Reason::Denied => failure(FailureKind::Denied, name),
                Reason::Busy if !started => failure(FailureKind::Rejected, name),
                Reason::InvalidRequest | Reason::Abandoned | Reason::Malformed => {
                    failure(FailureKind::InternalFailure, name)
                }
                Reason::Busy
                | Reason::SessionMismatch
                | Reason::WorkerUnavailable
                | Reason::WorkerChanged
                | Reason::JobUnknown => failure(FailureKind::ServiceUnavailable, name),
            })
        }
        End::Exchange { submitted, started } if mutating && (submitted || started) => {
            Err(failure(FailureKind::OutcomeUnknown, "transport-loss"))
        }
        End::Exchange { .. } => Err(failure(FailureKind::TransportLoss, "transport-loss")),
        End::Deadline if mutating => Err(failure(FailureKind::OutcomeUnknown, "deadline")),
        End::Deadline => Err(failure(FailureKind::Timeout, "deadline")),
    }
}

#[cfg(any(unix, windows))]
fn prepare(
    report: crate::agent_integrations::IntegrationReport,
    command: &AgentIntegrationCommand,
) -> Result<Prepared, Failure> {
    use crate::agent_integrations::IntegrationReport as Report;
    match (report, command) {
        (Report::HomeUnavailable {}, _) => Err(Failure::new(FailureKind::ServiceUnavailable)
            .details(stage_reason_details("environment", "home-unavailable"))),
        (Report::List { items, capsule }, AgentIntegrationCommand::List { outdated_only, .. }) => {
            Ok(Prepared::List {
                items,
                capsule,
                outdated_only: *outdated_only,
            })
        }
        (
            Report::Explain {
                item,
                detail,
                capsule,
                plan,
            },
            AgentIntegrationCommand::Explain { .. },
        ) => Ok(Prepared::Explain {
            item,
            detail,
            capsule,
            plan,
        }),
        (
            Report::Mutation { report },
            AgentIntegrationCommand::Enable { .. } | AgentIntegrationCommand::Disable { .. },
        ) => Ok(Prepared::Mutation { report }),
        _ => Err(Failure::new(FailureKind::InternalFailure)),
    }
}

/// One prepared integration answer — the typed result before the format
/// split, so text and JSON carry identical facts. Other platforms have no variant:
/// `run_local` always answers the unsupported-platform failure there.
#[cfg(any(unix, windows))]
pub(super) enum Prepared {
    List {
        outdated_only: bool,
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

#[cfg(not(any(unix, windows)))]
enum Prepared {}

impl Prepared {
    pub(super) fn render(self, format: OutputFormat, locale: Locale) -> Output {
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
    #[cfg(any(unix, windows))]
    fn split(self, locale: Locale) -> (u8, Value, String) {
        match self {
            Self::List {
                mut items,
                capsule,
                outdated_only,
            } => {
                let complete = items.iter().all(|item| item.state != State::Unknown);
                let inspected = items.len();
                let unresolved: Vec<_> = items
                    .iter()
                    .filter(|item| item.state == State::Unknown)
                    .map(|item| item.integration_id.provider())
                    .collect();
                let blocked: Vec<_> = items
                    .iter()
                    .filter(|item| item.state == State::Conflict)
                    .map(|item| item.integration_id.provider())
                    .collect();
                if outdated_only {
                    items.retain(|item| item.state == State::UpdateRequired);
                }
                let document = json!({
                    "schema": INTEGRATIONS_SCHEMA,
                    "version": 1,
                    "source": { "kind": "local-inspection" },
                    "result": {
                        "complete": complete,
                        "outdatedOnly": outdated_only,
                        "inspectedCount": inspected,
                        "unresolvedProviderIds": unresolved,
                        "blockedProviderIds": blocked,
                        "capsule": capsule_json(&capsule),
                        "integrations": items.iter().map(item_json).collect::<Vec<_>>(),
                    },
                });
                let mut text = list_text(&items, &capsule, locale);
                if !outdated_only {
                    text.push_str(&list_summary(&items, &capsule, locale));
                }
                if outdated_only {
                    text.push_str(&CliText::new(locale).cli_integration_maintenance(
                        inspected,
                        items.len(),
                        &unresolved,
                        &blocked,
                    ));
                }
                (0, document, text)
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
                        "provider": item.integration_id.provider(),
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

    #[cfg(not(any(unix, windows)))]
    fn split(self, _locale: Locale) -> (u8, Value, String) {
        match self {}
    }
}

// -- projection ---------------------------------------------------------------

#[cfg(any(unix, windows))]
fn item_json(item: &SnapshotItem) -> Value {
    json!({
        "integrationId": item.integration_id.provider(),
        "availability": item.availability.as_str(),
        "managementMode": item.management_mode.as_str(),
        "state": item.state.as_str(),
        "issue": item.issue.map(Issue::as_str),
    })
}

#[cfg(any(unix, windows))]
fn capsule_json(capsule: &CapsuleStatus) -> Value {
    let state = if capsule.binding_failure {
        "binding-failed"
    } else if capsule.ready {
        "ready"
    } else if capsule.source_kind == "absent" {
        "source-absent"
    } else {
        "not-ready"
    };
    let (root_key, root_value) = crate::native_path::detail(&capsule.root);
    let root_key = if root_key == "path" {
        "rootPath"
    } else {
        "rootPathBytesHex"
    };
    let mut object = Map::from_iter([
        ("state".to_owned(), json!(state)),
        ("source".to_owned(), json!(capsule.source_kind)),
        ("payloadSha256".to_owned(), json!(capsule.payload_sha256)),
        (root_key.to_string(), root_value),
    ]);
    if capsule.binding_failure {
        object.insert("bindingFailure".to_owned(), json!(true));
    }
    Value::Object(object)
}

#[cfg(any(unix, windows))]
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
            ..
        } => json!({
            "kind": "guarded-filesystem",
            "targetState": target_state.as_str(),
            "sourceReady": source_ready,
        }),
        InspectionDetail::Plain {} => json!({ "kind": "none" }),
    }
}

#[cfg(any(unix, windows))]
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

#[cfg(any(unix, windows))]
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

#[cfg(any(unix, windows))]
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
        (
            "provider".to_owned(),
            json!(report.integration_id.provider()),
        ),
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
#[cfg(any(unix, windows))]
fn invocation_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("cli-{}-{nanos}", std::process::id())
}

/// The frozen exit map: pre-effect refusals keep their class codes; a
/// post-effect verdict is determinate-failed (9), observed-conflict (3), or
/// undetermined (8). The frontier is `attempted`: once the capsule publish
/// or any vector step ran, a refusal named after a pre-effect class is a
/// determinate post-effect failure (9) — only the deadline family stays
/// uncertain (8).
#[cfg(any(unix, windows))]
pub(crate) fn mutation_status(report: &MutationReport) -> u8 {
    match report.verdict {
        Verdict::Completed => 0,
        Verdict::Refused(refusal) => {
            if report.effect_begun {
                // The frontier is already crossed — a refusal named after
                // a pre-effect class is a determinate post-effect failure
                // (9), while the observation-incomplete family (deadline,
                // unclassifiable read) stays undetermined (8).
                return match refusal {
                    Refusal::Deadline | Refusal::InspectionFailed => 8,
                    _ => 9,
                };
            }
            match refusal {
                Refusal::Conflict => 3,
                Refusal::ManualActionRequired
                | Refusal::AgentUnavailable
                | Refusal::CapsuleUnavailable => 4,
                Refusal::Deadline => 7,
                // An unclassifiable pre-effect state is a determinate
                // failure — the run refuses without ever falling into an
                // else-vector.
                Refusal::InspectionFailed => 9,
            }
        }
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

#[cfg(any(unix, windows))]
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

#[cfg(any(unix, windows))]
fn item_row(item: &SnapshotItem) -> Vec<String> {
    vec![
        item.integration_id.provider().to_owned(),
        item.availability.as_str().to_owned(),
        item.management_mode.as_str().to_owned(),
        item.state.as_str().to_owned(),
        item.issue.map(Issue::as_str).unwrap_or("-").to_owned(),
    ]
}

#[cfg(any(unix, windows))]
fn list_text(items: &[SnapshotItem], capsule: &CapsuleStatus, locale: Locale) -> String {
    let labels = CliText::new(locale).cli_labels();
    let mut out = String::new();
    out.push_str(&CliText::new(locale).cli_integration_capsule_line(&capsule_summary(capsule)));
    let rows: Vec<Vec<String>> = items.iter().map(item_row).collect();
    out.push_str(&bounded_table(&labels.integration_headers, &rows));
    out
}

/// Classify each row once — ready, needs `enable`, needs `explain`, or not on
/// PATH — and name the first provider an `enable` or `explain` would move on.
/// Enabling also publishes an unready capsule.
#[cfg(any(unix, windows))]
fn list_summary(items: &[SnapshotItem], capsule: &CapsuleStatus, locale: Locale) -> String {
    use crate::agent_integrations::model::Availability;
    let mut counts = [0; 4];
    let mut enable = None;
    let mut explain = None;
    for item in items {
        let id = item.integration_id.provider();
        let slot = match (item.availability, item.state) {
            (Availability::NotFound, _) => 3,
            (_, State::Enabled) if capsule.ready => 0,
            (_, State::Enabled | State::NotInstalled | State::Disabled | State::UpdateRequired) => {
                enable.get_or_insert(id);
                1
            }
            (_, State::Conflict | State::Unknown) => {
                explain.get_or_insert(id);
                2
            }
        };
        counts[slot] += 1;
    }
    let next = enable
        .map(|id| ("enable", id))
        .or_else(|| explain.map(|id| ("explain", id)));
    CliText::new(locale).cli_integration_summary(counts, next)
}

#[cfg(any(unix, windows))]
fn capsule_summary(capsule: &CapsuleStatus) -> String {
    let state = if capsule.binding_failure {
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

#[cfg(any(unix, windows))]
fn explain_text(
    item: &SnapshotItem,
    detail: &InspectionDetail,
    capsule: &CapsuleStatus,
    plan: &[PlannedStep],
    locale: Locale,
) -> String {
    let mut out = String::new();
    out.push_str(&CliText::new(locale).cli_integration_explain_header(
        item.integration_id.provider(),
        item.state.as_str(),
        item.availability.as_str(),
        item.issue.map(Issue::as_str).unwrap_or("-"),
        capsule_summary(capsule).as_str(),
    ));
    if let Some(line) = detail_line(detail) {
        out.push_str(&line);
    }
    if plan.is_empty() {
        out.push_str(CliText::new(locale).cli_integration_plan_empty());
    } else {
        out.push_str(CliText::new(locale).cli_integration_plan_header());
        for (index, step) in plan.iter().enumerate() {
            out.push_str(&format!("  {}. {}\n", index + 1, planned_step_text(step)));
        }
    }
    out
}

#[cfg(any(unix, windows))]
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
            ..
        } => Some(format!(
            "target-state: {}  source-ready: {}\n",
            target_state.as_str(),
            if *source_ready { "yes" } else { "no" },
        )),
        InspectionDetail::Plain {} => None,
    }
}

#[cfg(any(unix, windows))]
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

#[cfg(any(unix, windows))]
fn mutation_text(report: &MutationReport, locale: Locale) -> String {
    let provider = report.integration_id.provider();
    let direction = report.direction.as_str();
    let mut out = match report.verdict {
        Verdict::Completed => CliText::new(locale).cli_integration_completed(direction, provider),
        Verdict::Refused(refusal) => {
            CliText::new(locale).cli_integration_refused(direction, provider, refusal.as_str())
        }
        Verdict::Failed(code) => CliText::new(locale).cli_integration_failed(
            direction,
            provider,
            code.as_str(),
            report.observed.map(|item| item.state.as_str()),
        ),
        Verdict::Unknown => CliText::new(locale).cli_integration_unknown(direction, provider),
        Verdict::Cancelled => CliText::new(locale).cli_integration_cancelled(direction, provider),
    };
    for step in &report.attempted {
        out.push_str(&format!(
            "  {} {} → {}\n",
            CliText::new(locale).cli_integration_attempted_word(),
            step_text(&step.step),
            step.outcome.as_str(),
        ));
    }
    if !report.residuals.is_empty() {
        out.push_str(CliText::new(locale).cli_integration_residue_header());
        for residual in &report.residuals {
            out.push_str(&format!("  {}\n", residual.display()));
        }
    }
    out
}

#[cfg(any(unix, windows))]
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
