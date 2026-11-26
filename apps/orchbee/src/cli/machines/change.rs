//! `workspace set-machine`: a Workspace moves to another machine by being created there and closed
//! where it ran. The command runs on this computer and asks each machine in turn: the machine that
//! holds the Workspace for what it is, the machine it goes to for its Project and the new
//! Workspace, then the first machine again to close the old one. Files never move: the new
//! Workspace starts in the same folder relative to home when that folder exists there, otherwise in
//! home. Nothing closes unless the new Workspace was created, and a step that fails says what
//! changed. Without `--confirm` nothing moves: the command says what the move would stop, as the
//! machine that holds the Workspace reads it, and refuses.

mod facts;

use super::{Context, Error, list, routing};
use crate::{
    cli::{
        self, Failure, FailureKind, Output,
        console_interrupt::{ArmedSignals, Finish},
        output,
    },
    cli_text::{ChangeLine, CliText, Place},
    invocation::{
        OutputFormat, SetMachine,
        grammar::schemas::{
            WORKSPACE_SET_MACHINE_FAILURE_ID as FAILURE_ID,
            WORKSPACE_SET_MACHINE_RESULT_ID as RESULT_ID,
        },
        user_path::{self, spelled, windows_form},
    },
    localization::Locale,
    machines::{Catalog, Error as CatalogError, Profile, Store, names_this_computer},
};
use facts::Facts;
use orchbee_local_runtime::CommandFailure;
use orchbee_machine_protocol::hub::MachineState;
use orchbee_workbench_protocol::WorkbenchProjectId;
use serde_json::{Map, Value, json};
use std::time::{Duration, Instant};

/// How long the hub's states and capacities may take, as for `machine list`.
const HUB: Duration = Duration::from_secs(5);
/// The bound on one step run on this computer; each step keeps its own shorter default wait.
const LOCAL_STEP: Duration = Duration::from_secs(120);

/// A machine the change involves.
enum End {
    Here,
    Saved(Profile),
}

impl End {
    fn place(&self) -> Place<'_> {
        match self {
            Self::Here => Place::Here,
            Self::Saved(profile) => Place::Saved(&profile.draft.label),
        }
    }
    /// The machine as JSON names it: `local`, or the saved machine's ID.
    fn token(&self) -> &str {
        match self {
            Self::Here => crate::machines::LOCAL,
            Self::Saved(profile) => profile.id.as_str(),
        }
    }
    fn json(&self) -> Value {
        match self {
            Self::Here => json!({"local": true, "id": null, "label": null}),
            Self::Saved(profile) => json!({
                "local": false,
                "id": profile.id.as_str(),
                "label": profile.draft.label,
            }),
        }
    }
}

/// Why the change stopped, in the closed vocabulary of its failure document.
struct Refused {
    stage: &'static str,
    reason: &'static str,
    kind: FailureKind,
    /// The person's sentence, ending in a newline.
    line: String,
    machine: Option<String>,
    state: Option<String>,
    /// The step whose answer stopped the change; its status, kind and own message carry over.
    step: Option<Answer>,
    /// The Workspace the change already created on the other machine.
    created: Option<String>,
    /// What the move would stop, when it refused for want of `--confirm`.
    facts: Option<Facts>,
}

impl Refused {
    fn new(
        stage: &'static str,
        reason: &'static str,
        kind: FailureKind,
        line: String,
    ) -> Box<Self> {
        Box::new(Self {
            stage,
            reason,
            kind,
            line,
            machine: None,
            state: None,
            step: None,
            created: None,
            facts: None,
        })
    }
    /// A step that answered with a failure; its kind is the change's.
    fn step(stage: &'static str, answer: Answer, line: String) -> Box<Self> {
        let kind = answer
            .kind()
            .and_then(FailureKind::parse)
            .unwrap_or(FailureKind::InternalFailure);
        let mut refused = Self::new(stage, "step-failed", kind, line);
        refused.step = Some(answer);
        refused
    }
    fn output(self, format: OutputFormat, locale: Locale) -> Output {
        let step = self.step.as_ref().map(|answer| {
            json!({
                "kind": answer.kind().map_or(Value::Null, |kind| json!(kind)),
                "reason": answer.reason().map_or(Value::Null, |reason| json!(reason)),
            })
        });
        let facts =
            Facts::members(self.facts.as_ref()).map(|(name, value)| (name.to_owned(), value));
        let details = Map::from_iter(
            facts.into_iter().chain([
                ("stage".to_owned(), json!(self.stage)),
                ("reason".to_owned(), json!(self.reason)),
                ("machine".to_owned(), json!(self.machine)),
                ("state".to_owned(), json!(self.state)),
                ("step".to_owned(), step.unwrap_or(Value::Null)),
                (
                    "created".to_owned(),
                    self.created
                        .as_ref()
                        .map_or(Value::Null, |id| json!({"workspaceId": id})),
                ),
            ]),
        );
        let mut result = cli::failure_output(
            Failure::new(self.kind)
                .failure_schema(FAILURE_ID)
                .details(details),
            Some(format),
            locale,
        );
        if !self.line.is_empty() {
            result.stderr = self.line;
        }
        if let Some(answer) = self.step {
            // The step's own exit status and message say what it met; a step that exited 0 with
            // an answer the change cannot use keeps the change's failure status.
            if answer.status != 0 {
                result.status = answer.status;
            }
            if !answer.stderr.is_empty() {
                result.stderr.push_str(&answer.stderr);
                if !answer.stderr.ends_with('\n') {
                    result.stderr.push('\n');
                }
            }
        }
        result
    }
}

/// One step's answer: its exit status, what it wrote, the JSON document that was, if any, and
/// its own message.
struct Answer {
    status: u8,
    stdout: Vec<u8>,
    stderr: String,
    document: Option<Value>,
}

impl Answer {
    fn new(output: Output) -> Self {
        Self {
            document: serde_json::from_slice(&output.stdout).ok(),
            status: output.status,
            stdout: output.stdout,
            stderr: output.stderr,
        }
    }
    fn at(&self, pointer: &str) -> Option<&Value> {
        self.document.as_ref()?.pointer(pointer)
    }
    /// The failure the step reported: a failure document's, or a creation receipt's.
    fn failure(&self) -> Option<&Value> {
        self.at("/failure")
            .filter(|failure| failure.is_object())
            .or_else(|| {
                self.at("/result/failure")
                    .filter(|failure| failure.is_object())
            })
    }
    fn kind(&self) -> Option<&str> {
        self.failure()?.get("kind")?.as_str()
    }
    fn reason(&self) -> Option<&str> {
        self.failure()?.pointer("/details/reason")?.as_str()
    }
}

/// What the change learned about the Workspace where it runs.
struct Source {
    id: String,
    label: String,
    automatic_label: bool,
    project: Option<(String, String)>,
    folder: Option<String>,
}

pub(crate) fn run(change: &SetMachine, locale: Locale) -> Output {
    if crate::tui_capabilities::Capability::SavedMachines
        .require()
        .is_err()
    {
        return cli::unsupported_platform(Some(change.format), locale);
    }
    let Ok(signals) = ArmedSignals::arm_interrupts_latched() else {
        return cli::failure_output(
            Failure::new(FailureKind::InternalFailure),
            Some(change.format),
            locale,
        );
    };
    let mut context = Context {
        signals,
        deadline: Instant::now() + HUB,
    };
    let result = execute(change, locale, &mut context);
    let finalization = match context.signals.finish() {
        Ok(Finish::Cancelled { finalization, .. }) => Some(finalization),
        Ok(Finish::Clear) | Err(_) => None,
    };
    // A finished change stands even if an interrupt arrives after its last step.
    let mut output = match result {
        Ok(output) => output,
        Err(refused) => refused.output(change.format, locale),
    };
    output.finalization = finalization;
    output
}

fn execute(
    change: &SetMachine,
    locale: Locale,
    context: &mut Context,
) -> Result<Output, Box<Refused>> {
    let words = CliText::new(locale);
    let operand = change.workspace.as_str();
    let here = |selector: Option<&str>| selector.is_none_or(names_this_computer);
    let catalog = if here(change.from.as_deref()) && here(Some(&change.to)) {
        None
    } else {
        Some(
            Store::configured()
                .and_then(|store| store.read())
                .map_err(|error| catalog_refusal(error, words))?,
        )
    };
    let from = end(catalog.as_ref(), change.from.as_deref(), words)?;
    let to = end(catalog.as_ref(), Some(&change.to), words)?;
    if from.token() == to.token() {
        let mut refused = Refused::new(
            "machine",
            "same-machine",
            FailureKind::Rejected,
            words.cli_set_machine(&ChangeLine::Same {
                workspace: operand,
                place: to.place(),
            }),
        );
        refused.machine = Some(to.token().to_owned());
        return Err(refused);
    }
    let saved: Vec<&Profile> = [&from, &to]
        .into_iter()
        .filter_map(|end| match end {
            End::Saved(profile) => Some(profile),
            End::Here => None,
        })
        .collect();
    let mut hub = if saved.is_empty() {
        None
    } else {
        Some(read_hub(context)?)
    };
    for profile in saved {
        let state = match &hub {
            Some(list::Hub::Read { states, .. }) => states.get(profile.id.as_str()),
            Some(list::Hub::Absent | list::Hub::Unreadable) | None => None,
        };
        // Only a connected machine, or one waiting for a login that the steps then ask for, is
        // used; the change never connects one.
        if !matches!(
            state,
            Some(MachineState::Online | MachineState::NeedsLogin { .. })
        ) {
            let mut refused = Refused::new(
                "machine",
                "not-connected",
                FailureKind::ServiceUnavailable,
                words.cli_set_machine(&ChangeLine::NotConnected {
                    machine: &profile.draft.label,
                }),
            );
            refused.machine = Some(profile.id.as_str().to_owned());
            refused.state = Some(
                state
                    .map_or("unknown", |state| state.name().as_str())
                    .to_owned(),
            );
            return Err(refused);
        }
    }
    let source = read_source(&from, operand, locale, context)?;
    let workspace = source.label.as_str();
    if let Some((project, _)) = &source.project
        && !WorkbenchProjectId::parse(project).is_ok_and(|project| project.is_random())
    {
        return Err(Refused::new(
            "project",
            "project-not-shareable",
            FailureKind::Rejected,
            words.cli_set_machine(&ChangeLine::ProjectNotShareable { workspace }),
        ));
    }
    let mut home = home_of(&from, hub.as_ref());
    if home.is_none() && matches!(from, End::Saved(_)) {
        // A machine that waited for a login when the hub was first read has logged in since.
        hub = Some(read_hub(context)?);
        home = home_of(&from, hub.as_ref());
    }
    let parts = home
        .as_deref()
        .zip(source.folder.as_deref())
        .and_then(|(home, folder)| below(home, folder))
        .filter(|parts| !parts.is_empty());
    let folder = parts
        .as_ref()
        .map_or_else(|| "~".to_owned(), |parts| format!("~/{}", parts.join("/")));
    let summary = |folder: &str, created: Option<Value>, facts: Option<&Facts>| {
        let mut result = Map::from_iter([
            ("dryRun".to_owned(), json!(change.dry_run)),
            (
                "workspace".to_owned(),
                json!({
                    "workspaceId": source.id,
                    "label": source.label,
                    "projectId": source.project.as_ref().map(|(project, _)| project),
                }),
            ),
            ("from".to_owned(), from.json()),
            ("to".to_owned(), to.json()),
            ("folder".to_owned(), json!(folder)),
            ("created".to_owned(), created.unwrap_or(Value::Null)),
        ]);
        result.extend(
            Facts::members(facts)
                .into_iter()
                .map(|(name, value)| (name.to_owned(), value)),
        );
        json!({"schema": RESULT_ID, "version": 1, "result": result})
    };
    if change.dry_run || !change.confirm {
        // What closing the Workspace would stop, as `workspace close --dry-run` reads it there.
        let preview = step(
            &from,
            &[
                "workspace",
                "close",
                &source.id,
                "--dry-run",
                "--format",
                "json",
            ],
            OutputFormat::Json,
            locale,
            context,
        );
        let line = || words.cli_set_machine(&ChangeLine::SourceFailed { workspace });
        if preview.status != 0 {
            return Err(Refused::step("source", preview, line()));
        }
        let Some(facts) = Facts::of(&preview) else {
            return Err(Refused::step("source", preview, line()));
        };
        let names = facts.names(locale);
        if !change.dry_run {
            let mut refused = Refused::new(
                "confirm",
                "confirmation-required",
                FailureKind::InvalidUsage,
                words.cli_set_machine(&ChangeLine::Unconfirmed {
                    workspace,
                    to: to.place(),
                    stops: facts.stops(&names),
                }),
            );
            refused.facts = Some(facts);
            return Err(refused);
        }
        return Ok(match change.format {
            OutputFormat::Json => emit(
                output::encode_document(&summary(&folder, None, Some(&facts))),
                change.format,
                locale,
            ),
            OutputFormat::Text => {
                // The terminals as `terminal list` shows them there.
                let table = step(
                    &from,
                    &[
                        "terminal",
                        "list",
                        "--workspace",
                        &source.id,
                        "--format",
                        "text",
                    ],
                    OutputFormat::Text,
                    locale,
                    context,
                );
                if table.status != 0 {
                    return Err(Refused::step("source", table, line()));
                }
                let mut text = words.cli_set_machine(&ChangeLine::Preview {
                    workspace,
                    from: from.place(),
                    to: to.place(),
                });
                text.push_str(&words.cli_close_facts(facts.stops(&names)));
                let mut text = text.into_bytes();
                text.extend_from_slice(&table.stdout);
                emit(Ok(text), change.format, locale)
            }
        });
    }
    if let Some((project, label)) = &source.project {
        go_on(context, "project")?;
        let shared = step(
            &to,
            &[
                "project", "create", "--label", label, "--id", project, "--format", "json",
            ],
            OutputFormat::Json,
            locale,
            context,
        );
        if shared.status != 0 {
            // An older CLI there does not know `--id`; an older Session there refuses it.
            if shared.kind() == Some("invalid-usage")
                || (shared.kind() == Some("incompatible")
                    && shared.reason() == Some("project-identity"))
            {
                let mut refused = Refused::new(
                    "project",
                    "target-too-old",
                    FailureKind::Incompatible,
                    words.cli_set_machine(&ChangeLine::TooOld {
                        workspace,
                        place: to.place(),
                    }),
                );
                refused.machine = Some(to.token().to_owned());
                refused.step = Some(shared);
                return Err(refused);
            }
            return Err(Refused::step(
                "project",
                shared,
                words.cli_set_machine(&ChangeLine::ProjectFailed { workspace }),
            ));
        }
    }
    go_on(context, "create")?;
    let mut landed = folder.clone();
    let mut created = create(&to, &source, false, &landed, locale, context);
    if landed != "~" && created_nothing(&created) {
        // That folder is not there, or its name cannot be spelled there: home it is, and an
        // automatic name that would follow home keeps the Workspace's name instead.
        landed = "~".to_owned();
        created = create(&to, &source, true, &landed, locale, context);
    }
    let new_id = created
        .at("/result/workspace/result/workspaceId")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let path = created
        .at("/result/directory/path")
        .and_then(Value::as_str)
        .map(str::to_owned);
    // Where the new Workspace is, so a Client can show it: its Session and first terminal context.
    let place = created
        .at("/result/workspace/source/sessionId")
        .and_then(Value::as_str)
        .zip(
            created
                .at("/result/workspace/result/terminalContextId")
                .and_then(Value::as_str),
        )
        .map(|(session, terminal)| (session.to_owned(), terminal.to_owned()));
    let completed = created.at("/result/outcome").and_then(Value::as_str) == Some("completed");
    let (Some(new_id), Some(path), Some((session, terminal)), true, 0) =
        (new_id.clone(), path, place, completed, created.status)
    else {
        let mut refused = Refused::step(
            "create",
            created,
            words.cli_set_machine(&ChangeLine::CreateFailed { workspace }),
        );
        refused.machine = Some(to.token().to_owned());
        refused.created = new_id;
        return Err(refused);
    };
    // An interrupt during the create keeps the old Workspace: the new one stands beside it.
    if let Err(mut refused) = go_on(context, "close") {
        refused.line = words.cli_set_machine(&ChangeLine::CloseFailed {
            workspace,
            id: &new_id,
        });
        refused.created = Some(new_id);
        return Err(refused);
    }
    let closed = step(
        &from,
        &[
            "workspace",
            "close",
            &source.id,
            "--confirm",
            "--format",
            "json",
        ],
        OutputFormat::Json,
        locale,
        context,
    );
    if closed.status != 0 {
        let mut refused = Refused::step(
            "close",
            closed,
            words.cli_set_machine(&ChangeLine::CloseFailed {
                workspace,
                id: &new_id,
            }),
        );
        refused.machine = Some(from.token().to_owned());
        refused.created = Some(new_id);
        return Err(refused);
    }
    Ok(match change.format {
        OutputFormat::Json => emit(
            output::encode_document(&summary(
                &landed,
                Some(json!({
                    "workspaceId": new_id,
                    "path": path,
                    "sessionId": session,
                    "terminalContextId": terminal,
                })),
                None,
            )),
            change.format,
            locale,
        ),
        OutputFormat::Text => {
            let shown = spelled(&landed, &path);
            emit(
                Ok(words
                    .cli_set_machine(&ChangeLine::Done {
                        workspace,
                        to: to.place(),
                        folder: &shown,
                        id: &new_id,
                    })
                    .into_bytes()),
                change.format,
                locale,
            )
        }
    })
}

fn emit(bytes: Result<Vec<u8>, output::TooLarge>, format: OutputFormat, locale: Locale) -> Output {
    match bytes {
        Ok(stdout) => Output {
            status: 0,
            stdout,
            stderr: String::new(),
            finalization: None,
        },
        Err(_) => cli::failure_output(
            Failure::new(FailureKind::OutputTooLarge),
            Some(format),
            locale,
        ),
    }
}

fn catalog_refusal(error: CatalogError, words: CliText) -> Box<Refused> {
    let (code, kind, message) = routing::mapped(Error::Catalog(error));
    Refused::new(
        "machine",
        code,
        kind,
        format!("{}\n", words.cli_machine_message(message)),
    )
}

/// The machine `selector` names: this computer for none or `local`, otherwise a saved machine.
fn end(
    catalog: Option<&Catalog>,
    selector: Option<&str>,
    words: CliText,
) -> Result<End, Box<Refused>> {
    let (Some(selector), Some(catalog)) = (selector.filter(|s| !names_this_computer(s)), catalog)
    else {
        return Ok(End::Here);
    };
    match catalog.resolve(selector) {
        Ok(profile) => Ok(End::Saved(profile.clone())),
        Err(error) => {
            let error = match error {
                CatalogError::Ambiguous => CatalogError::Ambiguous,
                _ => CatalogError::NotFound,
            };
            let mut refused = catalog_refusal(error, words);
            refused.machine = Some(selector.to_owned());
            Err(refused)
        }
    }
}

fn read_hub(context: &mut Context) -> Result<list::Hub, Box<Refused>> {
    context.deadline = Instant::now() + HUB;
    list::read(context).map_err(|_| cancelled("machine"))
}

fn cancelled(stage: &'static str) -> Box<Refused> {
    Refused::new(stage, "cancelled", FailureKind::Cancelled, String::new())
}

/// Stops before a step that changes something once an interrupt has arrived.
fn go_on(context: &mut Context, stage: &'static str) -> Result<(), Box<Refused>> {
    match context.interrupted() {
        Ok(false) => Ok(()),
        Ok(true) | Err(_) => Err(cancelled(stage)),
    }
}

/// Reads the Workspace `operand` names where it runs: its identity, name, Project and folder. A
/// Workspace in a linked worktree is refused, since the worktree stays with its repository.
fn read_source(
    from: &End,
    operand: &str,
    locale: Locale,
    context: &mut Context,
) -> Result<Source, Box<Refused>> {
    let words = CliText::new(locale);
    let failed = |answer| {
        Refused::step(
            "source",
            answer,
            words.cli_set_machine(&ChangeLine::SourceFailed { workspace: operand }),
        )
    };
    let get = step(
        from,
        &["workspace", "get", operand, "--format", "json"],
        OutputFormat::Json,
        locale,
        context,
    );
    if get.status != 0 {
        return Err(failed(get));
    }
    let text = |pointer: &str| get.at(pointer).and_then(Value::as_str).map(str::to_owned);
    let (Some(id), Some(label)) = (
        text("/result/workspace/workspaceId"),
        text("/result/workspace/label"),
    ) else {
        return Err(unreadable("source"));
    };
    let project = text("/result/workspace/projectId");
    if get.at("/result/workspace/worktree/linked") == Some(&Value::Bool(true)) {
        let mut refused = Refused::new(
            "source",
            "worktree",
            FailureKind::Rejected,
            words.cli_set_machine(&ChangeLine::Worktree),
        );
        refused.machine = Some(from.token().to_owned());
        return Err(refused);
    }
    let snapshot = step(
        from,
        &["api", "snapshot", "--format", "json"],
        OutputFormat::Json,
        locale,
        context,
    );
    if snapshot.status != 0 {
        return Err(failed(snapshot));
    }
    let navigation = snapshot
        .at("/result/snapshot/navigation")
        .ok_or_else(|| unreadable("source"))?;
    let find = |list: &str, key: &str, value: &str| {
        navigation
            .get(list)
            .and_then(Value::as_array)
            .and_then(|items| {
                items
                    .iter()
                    .find(|item| item.get(key).and_then(Value::as_str) == Some(value))
            })
    };
    let entry = find("workspaces", "workspaceId", &id).ok_or_else(|| unreadable("source"))?;
    let folder = entry
        .get("terminalDirectory")
        .and_then(Value::as_str)
        .or_else(|| {
            let directory = entry.get("primaryDirectoryId")?.as_str()?;
            find("directories", "directoryId", directory)?
                .get("path")?
                .as_str()
        })
        .map(str::to_owned);
    let project = match project {
        Some(project) => {
            let label = find("projects", "projectId", &project)
                .and_then(|item| item.get("label"))
                .and_then(Value::as_str)
                .ok_or_else(|| unreadable("source"))?
                .to_owned();
            Some((project, label))
        }
        None => None,
    };
    Ok(Source {
        id,
        label,
        automatic_label: entry.get("automaticLabel").and_then(Value::as_bool) == Some(true),
        project,
        folder,
    })
}

/// A step answered with a document this build cannot read.
fn unreadable(stage: &'static str) -> Box<Refused> {
    Refused::new(
        stage,
        "step-failed",
        FailureKind::ProtocolInvalid,
        String::new(),
    )
}

/// Creates the Workspace on `to` in `folder`, under its name unless an automatic name may follow
/// the folder there, and in its Project.
fn create(
    to: &End,
    source: &Source,
    keep_name: bool,
    folder: &str,
    locale: Locale,
    context: &mut Context,
) -> Answer {
    let mut arguments = vec!["workspace", "create"];
    if keep_name || !source.automatic_label {
        arguments.extend(["--label", source.label.as_str()]);
    }
    if let Some((project, _)) = &source.project {
        arguments.extend(["--project", project.as_str()]);
    }
    arguments.extend(["--cwd", folder, "--format", "json"]);
    step(to, &arguments, OutputFormat::Json, locale, context)
}

/// Whether a create refused its folder before creating anything.
fn created_nothing(answer: &Answer) -> bool {
    answer.kind() == Some("invalid-usage")
        || (answer.at("/result/workspace") == Some(&Value::Null)
            && answer.reason() == Some("directory-unavailable"))
}

/// The home folder of `end`: this computer's as `--cwd ~` reads it, or what a saved machine reported
/// with its free disk, which is its home folder's volume.
fn home_of(end: &End, hub: Option<&list::Hub>) -> Option<String> {
    match end {
        End::Here => user_path::resolve(
            "~",
            user_path::HOST_GRAMMAR,
            |name| std::env::var_os(name),
            std::env::current_dir,
        )
        .ok()
        .map(|path| path.as_str().to_owned()),
        End::Saved(profile) => match hub? {
            list::Hub::Read { capacities, .. } => capacities
                .get(&Some(profile.id.as_str().to_owned()))?
                .0
                .disk
                .as_ref()
                .map(|disk| disk.path.as_str().to_owned()),
            list::Hub::Absent | list::Hub::Unreadable => None,
        },
    }
}

/// The parts of `folder` below `home`, when `folder` is `home` or inside it. A Windows home takes
/// either separator and any letter case, as Windows does.
fn below(home: &str, folder: &str) -> Option<Vec<String>> {
    let windows = windows_form(home);
    let parts = |path: &'_ str| -> Vec<String> {
        path.split(|c| c == '/' || (windows && c == '\\'))
            .filter(|part| !part.is_empty())
            .map(|part| {
                if windows {
                    part.to_lowercase()
                } else {
                    part.to_owned()
                }
            })
            .collect()
    };
    let (home, compared) = (parts(home), parts(folder));
    if compared.len() < home.len() || compared[..home.len()] != home[..] {
        return None;
    }
    // The folder's own spelling is kept below home.
    let kept: Vec<&str> = folder
        .split(|c| c == '/' || (windows && c == '\\'))
        .filter(|part| !part.is_empty())
        .collect();
    Some(
        kept[home.len()..]
            .iter()
            .map(|part| (*part).to_owned())
            .collect(),
    )
}

/// Runs one step on `end`: here as a child of this program, or routed to a saved machine.
fn step(
    end: &End,
    arguments: &[&str],
    format: OutputFormat,
    locale: Locale,
    context: &mut Context,
) -> Answer {
    let arguments = arguments.iter().map(|word| (*word).to_owned()).collect();
    Answer::new(match end {
        End::Here => locally(arguments, format, locale, context),
        End::Saved(profile) => {
            routing::ask(profile.id.as_str(), arguments, format, locale, context)
        }
    })
}

/// Runs this program with `arguments` in the Session this command selected, its output captured.
fn locally(
    arguments: Vec<String>,
    format: OutputFormat,
    locale: Locale,
    context: &mut Context,
) -> Output {
    let failed = |kind| cli::failure_output(Failure::new(kind), Some(format), locale);
    let Ok(command) = crate::self_command::command(arguments) else {
        return failed(FailureKind::InternalFailure);
    };
    let result = crate::self_command::exchange(command, Instant::now() + LOCAL_STEP, || {
        context.interrupted()
    });
    if result.stdout_truncated || result.stderr_truncated {
        return failed(FailureKind::OutcomeUnknown);
    }
    match (
        result.failure,
        result.exit_code.and_then(|code| u8::try_from(code).ok()),
    ) {
        (None | Some(CommandFailure::ExitFailure), Some(status)) => Output {
            status,
            stdout: result.stdout,
            stderr: String::from_utf8_lossy(&result.stderr).into_owned(),
            finalization: None,
        },
        (Some(CommandFailure::SpawnFailed | CommandFailure::SpawnUnavailable), _) => {
            failed(FailureKind::InternalFailure)
        }
        (Some(CommandFailure::Cancelled), _) => failed(FailureKind::Cancelled),
        (Some(CommandFailure::Timeout), _) => failed(FailureKind::Timeout),
        _ => failed(FailureKind::OutcomeUnknown),
    }
}

#[cfg(test)]
mod tests;
