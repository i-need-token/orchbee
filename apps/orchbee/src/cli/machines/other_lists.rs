//! `workspace list` and `agent list` that this computer answers for itself also carry each
//! connected machine's own answer, so a person or an orchestrating Agent here finds every Workspace
//! and Agent in one command. Only the machines the hub serving the data root reports online are
//! asked, in catalog order, one after another under one deadline; a machine that does not answer
//! is named and never asked again. Each answer stays that machine's own document: its IDs and
//! compact handles belong to its Session, and `--machine` reaches them.
use super::{Context, list};
use crate::{
    cli::{FailureKind, Output, console_interrupt::ArmedSignals, output},
    cli_text::{CliText, escape::escape},
    invocation::{OutputFormat, QueryCommand, Target},
    localization::Locale,
    machines::Store,
};
use orchbee_machine_protocol::hub::MachineState;
use serde_json::{Value, json};
use std::time::{Duration, Instant};

/// How long every machine together may take to answer.
const DEADLINE: Duration = Duration::from_secs(20);

/// What the list names, for the line about a machine that did not answer.
#[derive(Clone, Copy)]
pub(crate) enum Listing {
    Workspaces,
    Agents,
}

/// One connected machine and its answer: its own standard output, or `None` when it did not
/// answer.
struct Answer {
    id: String,
    label: String,
    stdout: Option<Vec<u8>>,
}

/// `local` with each connected machine's answer to the same `command`. A failed local list, and a
/// list read through a named Session, a Client, or a page cursor, which name this computer's own
/// state, ask no machine. The JSON document always carries `result.machines`.
pub(crate) fn carry(
    local: Output,
    command: &QueryCommand,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    if local.status != 0 {
        return local;
    }
    let Some((listing, mut arguments)) = arguments(command) else {
        return compose(local, &[], format, None, locale);
    };
    if format == OutputFormat::Json {
        arguments.extend(["--format".to_owned(), "json".to_owned()]);
    }
    let mut local = local;
    let answers = match ask_all(&arguments, format, locale, &mut local) {
        Ok(answers) => answers,
        Err(interrupted) => return *interrupted,
    };
    compose(local, &answers, format, Some(listing), locale)
}

/// The command line each machine runs, and what it lists; `None` when no machine is asked. A
/// list another machine routed here answers for this computer alone, so no two machines that
/// list each other ask in a circle.
fn arguments(command: &QueryCommand) -> Option<(Listing, Vec<String>)> {
    if crate::cli::server::routed() || crate::local_activation::selected_session_name().is_some() {
        return None;
    }
    match command {
        QueryCommand::WorkspaceList { project, worktrees } => {
            let mut arguments = vec!["workspace".to_owned(), "list".to_owned()];
            if let Some(project) = project {
                // A compact handle is this Session's; the Project's full ID is shared.
                let project = match project {
                    Target::Selector(_) => project.selector_identity()?,
                    _ => project.text().to_owned(),
                };
                arguments.extend(["--project".to_owned(), project]);
            }
            if *worktrees {
                arguments.push("--worktrees".to_owned());
            }
            Some((Listing::Workspaces, arguments))
        }
        QueryCommand::AgentList {
            include_ended,
            limit,
            cursor: None,
            client: None,
        } => {
            let mut arguments = vec!["agent".to_owned(), "list".to_owned()];
            if *include_ended {
                arguments.push("--include-ended".to_owned());
            }
            arguments.extend(["--limit".to_owned(), limit.to_string()]);
            Some((Listing::Agents, arguments))
        }
        _ => None,
    }
}

/// Asks every connected machine in catalog order. Only an interrupt is an error: it ends the
/// command with the interrupted machine's failure and no document.
fn ask_all(
    arguments: &[String],
    format: OutputFormat,
    locale: Locale,
    local: &mut Output,
) -> Result<Vec<Answer>, Box<Output>> {
    if crate::tui_capabilities::Capability::SavedMachines
        .require()
        .is_err()
    {
        return Ok(Vec::new());
    }
    let Ok(catalog) = Store::configured().and_then(|store| store.read()) else {
        return Ok(Vec::new());
    };
    if catalog.profiles.is_empty() {
        return Ok(Vec::new());
    }
    let Ok(signals) = ArmedSignals::arm_interrupts_except_linux() else {
        return Ok(Vec::new());
    };
    let mut context = Context {
        signals,
        deadline: Instant::now() + DEADLINE,
    };
    let states = list::read_states(&mut context).unwrap_or_default();
    let mut answers = Vec::new();
    for profile in catalog
        .profiles
        .iter()
        .filter(|profile| matches!(states.get(profile.id.as_str()), Some(MachineState::Online)))
    {
        let output = super::routing::ask(
            profile.id.as_str(),
            arguments.to_vec(),
            format,
            locale,
            &mut context,
        );
        if output.status == FailureKind::Cancelled.exit_code() {
            finish(local, context);
            let mut interrupted = output;
            interrupted.finalization = local.finalization.take();
            return Err(Box::new(interrupted));
        }
        answers.push(Answer {
            id: profile.id.as_str().to_owned(),
            label: profile.draft.label.clone(),
            stdout: (output.status == 0).then_some(output.stdout),
        });
    }
    finish(local, context);
    Ok(answers)
}

/// The local list keeps the first finalization, which restores the signal state the process
/// started with; a later one is released at once and leaves an interrupt pending for it.
fn finish(local: &mut Output, context: Context) {
    let finalization = context.signals.into_finalization();
    if local.finalization.is_none() {
        local.finalization = Some(finalization);
    } else {
        let _ = finalization.release();
    }
}

fn compose(
    mut local: Output,
    answers: &[Answer],
    format: OutputFormat,
    listing: Option<Listing>,
    locale: Locale,
) -> Output {
    let text = CliText::new(locale);
    for answer in answers.iter().filter(|answer| answer.stdout.is_none()) {
        if let Some(listing) = listing {
            local.stderr.push_str(
                &text.cli_machine_unlisted(&answer.label, matches!(listing, Listing::Agents)),
            );
        }
    }
    match format {
        OutputFormat::Text => {
            for answer in answers {
                if let Some(stdout) = &answer.stdout {
                    local.stdout.extend_from_slice(b"\n@");
                    local
                        .stdout
                        .extend_from_slice(escape(&answer.label).as_bytes());
                    local.stdout.push(b'\n');
                    local.stdout.extend_from_slice(stdout);
                }
            }
            local
        }
        OutputFormat::Json => {
            let Ok(mut document) = serde_json::from_slice::<Value>(&local.stdout) else {
                return local;
            };
            let machines: Vec<Value> = answers
                .iter()
                .map(|answer| {
                    let remote = answer
                        .stdout
                        .as_deref()
                        .and_then(|stdout| serde_json::from_slice::<Value>(stdout).ok());
                    let part = |name: &str| {
                        remote
                            .as_ref()
                            .and_then(|remote| remote.get(name))
                            .cloned()
                            .unwrap_or(Value::Null)
                    };
                    json!({
                        "machine": answer.id,
                        "label": answer.label,
                        "source": part("source"),
                        "result": part("result"),
                    })
                })
                .collect();
            if let Some(result) = document.get_mut("result").and_then(Value::as_object_mut) {
                result.insert("machines".to_owned(), Value::Array(machines));
            }
            if let Ok(stdout) = output::encode_document(&document) {
                local.stdout = stdout;
            }
            local
        }
    }
}
