#[cfg(windows)]
mod agent_configuration_windows;
mod agent_doctor;
mod agent_integrations;
mod agent_provider;
mod cli;
mod client_view;
mod invocation;
#[cfg(any(target_os = "linux", windows))]
mod local_activation;
#[cfg(any(unix, windows))]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "the accepted private foundations are staged before TUI and activation integration"
    )
)]
mod local_client;
#[cfg(any(target_os = "linux", windows))]
mod local_startup;
mod localization;
#[cfg(windows)]
mod notifications;
mod presentation;
#[cfg(any(unix, windows))]
mod startup_wait;
#[cfg(any(unix, windows))]
mod terminal_device;
#[cfg(any(unix, windows))]
mod terminal_input;
#[cfg(any(unix, windows))]
mod terminal_lifecycle;
#[cfg(any(unix, windows))]
mod terminal_projection;
#[cfg(any(unix, windows))]
mod terminal_session;
#[cfg(any(unix, windows))]
mod terminal_text;
#[cfg(windows)]
mod terminal_title;
#[cfg(any(unix, windows))]
mod termination_signal;
#[cfg(any(unix, windows))]
mod tui_backend;
#[cfg(any(unix, windows))]
mod tui_layout;
#[cfg(any(unix, windows))]
mod tui_pointer;
#[cfg(any(unix, windows))]
mod tui_render;
#[cfg(any(unix, windows))]
mod tui_shell;
#[cfg(any(unix, windows))]
mod tui_theme;
#[cfg(windows)]
mod windows_console_input;
#[cfg(windows)]
mod windows_signal;

use std::{
    env,
    io::{self, Write},
    process::ExitCode,
};

use invocation::{Invocation, parse_selected_request};
use localization::{Locale, select_locale};
use presentation::{ProcessResult, present};

// Keep the additive shell integration seam type-checked without making it an invocation route.
#[cfg(any(unix, windows))]
const _: fn(localization::Locale, bool) -> Result<(), tui_shell::TuiFailure> = tui_shell::run;

fn main() -> ExitCode {
    let locale = select_locale(|name| env::var_os(name));
    let parsed = parse_selected_request(env::args_os().skip(1)).map(|selected| {
        #[cfg(windows)]
        local_activation::select_site(selected.site);
        selected.request
    });
    #[cfg(any(target_os = "linux", windows))]
    if let Ok(invocation::Request::Tui { mouse }) = &parsed {
        return match local_startup::run(locale, *mouse) {
            Ok(()) => ExitCode::SUCCESS,
            Err(failure) => emit(
                cli::process_result(
                    ProcessResult {
                        status: 1,
                        stdout: "",
                        stderr: locale.startup_failure(failure),
                    },
                    locale,
                ),
                locale,
            ),
        };
    }
    emit(
        match parsed {
            Err(failure) => cli::parse_failure(failure, locale),
            Ok(invocation::Request::Tui { .. }) => {
                cli::process_result(present(Ok(Invocation::Tui), locale), locale)
            }
            Ok(invocation::Request::Version) => {
                cli::process_result(present(Ok(Invocation::Version), locale), locale)
            }
            Ok(invocation::Request::RootHelp) => {
                cli::process_result(present(Ok(Invocation::Help), locale), locale)
            }
            Ok(invocation::Request::Help(topic)) => cli::help(topic, locale),
            Ok(invocation::Request::Skill) => cli::skill(locale),
            Ok(invocation::Request::Completion(shell)) => cli::completion(shell, locale),
            Ok(invocation::Request::TerminalAttach { command }) => {
                cli::terminal_attach(&command, locale)
            }
            Ok(invocation::Request::TerminalControl { command }) => {
                cli::terminal_control(&command, locale)
            }
            Ok(invocation::Request::TerminalObserve { command }) => {
                cli::terminal_observe(&command, locale)
            }
            #[cfg(windows)]
            Ok(invocation::Request::ClientQuery {
                command,
                client,
                format,
            }) => cli::workbench_query::run(&command, Some(&client), format, locale),
            Ok(invocation::Request::Query { command, format }) => {
                cli::query(command, format, locale)
            }
            Ok(invocation::Request::Mutation { command, format }) => {
                cli::mutation(command, format, locale)
            }
            #[cfg(windows)]
            Ok(invocation::Request::PanelTerminalCreation { command, format }) => {
                cli::panel_terminal_creation::run(&command, format, locale)
            }
            Ok(invocation::Request::Creation { command, format }) => {
                cli::run_creation(&command, format, locale)
            }
            #[cfg(windows)]
            Ok(invocation::Request::PanelRelocation { command, format }) => {
                cli::panel_relocation::run(&command, format, locale)
            }
            #[cfg(windows)]
            Ok(invocation::Request::LayoutExport { command, format }) => {
                cli::layouts::export(&command, format, locale)
            }
            #[cfg(windows)]
            Ok(invocation::Request::LayoutApply { command, format }) => {
                cli::layouts::apply(&command, format, locale)
            }
            #[cfg(windows)]
            Ok(invocation::Request::WorkspaceCreation { command, format }) => {
                cli::workspace_creation::run(&command, format, locale)
            }
            Ok(invocation::Request::SiteStart { format }) => cli::site_start(format, locale),
            #[cfg(windows)]
            Ok(invocation::Request::ConfigResetKeys { format }) => {
                cli::config_reset::run(format, locale)
            }
            #[cfg(windows)]
            Ok(invocation::Request::NamedSession { command, format }) => {
                cli::named_sessions::run(&command, format, locale)
            }
            Ok(invocation::Request::RuntimeStatus { scope, format }) => {
                cli::runtime_status::run(scope, format, locale)
            }
            Ok(invocation::Request::SiteStop { command, format }) => {
                cli::site_stop(&command, format, locale)
            }
            Ok(invocation::Request::SchemaExport {
                schema_id,
                path,
                format,
            }) => cli::export_schema(schema_id.as_deref(), &path, format, locale),
            Ok(invocation::Request::Client { command, format }) => {
                cli::run_client_control(&command, format, locale)
            }
            Ok(invocation::Request::PanelAgent { command, format }) => {
                cli::panel_agent::run(&command, format, locale)
            }
            Ok(invocation::Request::PanelMetadata { command, format }) => {
                cli::panel_metadata::run(&command, format, locale)
            }
            Ok(invocation::Request::WorkspaceMetadata { command, format }) => {
                cli::workspace_metadata::run(&command, format, locale)
            }
            Ok(invocation::Request::WorkspaceGroupClose { command, format }) => {
                cli::run_workspace_group_close(&command, format, locale)
            }
            Ok(invocation::Request::WorktreeQuery { command, format }) => {
                cli::run_worktree_query(&command, format, locale)
            }
            Ok(invocation::Request::Worktree { command, format }) => {
                cli::run_worktree(&command, format, locale)
            }
            Ok(invocation::Request::WindowTitle { title, format }) => {
                cli::window_title::run(title, format, locale)
            }
            Ok(invocation::Request::Notification { message, format }) => {
                cli::notification::run(message, format, locale)
            }
            Ok(invocation::Request::RuntimeConfiguration { action, format }) => {
                cli::runtime_configuration::run(action, format, locale)
            }
            Ok(invocation::Request::AgentManifests { command, format }) => {
                cli::agent_manifests::run(&command, format, locale)
            }
            Ok(invocation::Request::SiteDiagnose { format }) => cli::site_diagnose(format, locale),
            Ok(invocation::Request::AgentStart { command, format }) => {
                cli::run_agent_start(&command, format, locale)
            }
            Ok(invocation::Request::AgentNames { command, format }) => {
                cli::agent_names::run(command, format, locale)
            }
            Ok(invocation::Request::AgentIntegration { command, format }) => {
                cli::agent_integration(command, format, locale)
            }
        },
        locale,
    )
}

fn emit(result: cli::Output, locale: Locale) -> ExitCode {
    emit_with(
        result,
        locale,
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    )
}

/// The shared emit body behind `emit`, writable over injected streams so the
/// write-failure and finalization ordering is provable in tests.
fn emit_with(
    result: cli::Output,
    locale: Locale,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> ExitCode {
    let cli::Output {
        status,
        stdout: out,
        stderr: err,
        finalization,
    } = result;
    let mut failed = false;
    if write_stream(&mut *stdout, &out).is_err() {
        // The prepared document failed at the OS write: a localized diagnostic
        // on stderr is the only remaining channel; no second document follows.
        let _ = write_stream(&mut *stderr, locale.cli_output_failed().as_bytes());
        failed = true;
    } else if write_stream(&mut *stderr, err.as_bytes()).is_err() {
        failed = true;
    }
    // The owed finalization is consumed on every path — normal, write
    // failure, and the output-failure diagnostic itself — so the inherited
    // signal mask stays blocked through the last write attempt and is then
    // restored exactly. A restore failure is a terminal internal failure.
    if let Some(token) = finalization
        && token.release().is_err()
    {
        let _ = write_stream(
            &mut *stderr,
            locale
                .cli_failure(&cli::Failure::new(cli::FailureKind::InternalFailure))
                .as_bytes(),
        );
        return ExitCode::FAILURE;
    }
    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::from(status)
    }
}

fn write_stream(mut writer: impl Write, payload: &[u8]) -> io::Result<()> {
    if payload.is_empty() {
        return Ok(());
    }
    writer.write_all(payload)?;
    writer.flush()
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::{
        env, fs,
        os::unix::process::ExitStatusExt,
        process::{Command, Stdio},
    };

    use super::*;
    use cli::{source::ArmedSignals, source::SignalWatch};
    use nix::sys::signal::{self, Signal};

    /// A writer that fails every write — write-failure routing evidence.
    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "injected"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// A writer that raises SIGTERM on its first write — the second signal
    /// arriving inside the cancelled-output window.
    struct RaisingWriter {
        inner: fs::File,
        raised: bool,
    }

    impl Write for RaisingWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            if !self.raised {
                self.raised = true;
                signal::raise(Signal::SIGTERM).expect("raise second signal");
            }
            self.inner.write(buffer)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.inner.flush()
        }
    }

    fn answer(finalization: Option<cli::SourceFinalization>) -> cli::Output {
        cli::Output {
            status: 130,
            stdout: b"{\"schema\":\"test\"}\n".to_vec(),
            stderr: String::new(),
            finalization,
        }
    }

    #[test]
    fn emit_consumes_finalization_on_normal_path() {
        // A real token captured at the current mask: `emit` must consume it,
        // restore nothing observable, and report the answer's status.
        let mask = nix::sys::signal::SigSet::thread_get_mask().expect("thread mask");
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = emit_with(
            answer(Some(cli::SourceFinalization::new(mask))),
            Locale::EnUs,
            &mut out,
            &mut err,
        );
        assert_eq!(code, ExitCode::from(130));
        assert_eq!(out, b"{\"schema\":\"test\"}\n");
    }

    #[test]
    fn emit_consumes_finalization_on_write_failure() {
        // The stdout write fails: the localized output-failure diagnostic
        // lands on stderr, the token is still consumed, and the status is
        // the write-failure code — never the answer's.
        let mask = nix::sys::signal::SigSet::thread_get_mask().expect("thread mask");
        let mut out = FailingWriter;
        let mut err = Vec::new();
        let code = emit_with(
            answer(Some(cli::SourceFinalization::new(mask))),
            Locale::EnUs,
            &mut out,
            &mut err,
        );
        assert_eq!(code, ExitCode::FAILURE);
        assert!(
            !err.is_empty(),
            "the output-failure diagnostic must reach stderr"
        );
    }

    #[test]
    fn emit_reports_restore_failure_as_terminal_internal() {
        // Mock evidence: a token whose release reports a kernel failure must
        // route the localized internal-failure diagnostic and exit 1 — no
        // second machine document, no answer status.
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = emit_with(
            answer(Some(cli::SourceFinalization::failing())),
            Locale::EnUs,
            &mut out,
            &mut err,
        );
        assert_eq!(code, ExitCode::FAILURE);
        assert!(
            !err.is_empty(),
            "restore failure must be diagnosed on stderr"
        );
        assert_eq!(out, b"{\"schema\":\"test\"}\n");
    }

    /// Child-mode probe: a second signal arriving mid-output while the
    /// finalization token still holds the mask must not preempt the output —
    /// the complete document is written before the release unmasks the
    /// pending signal into its original disposition.
    const CANCELLED_CHILD_ENV: &str = "ORCHBEE_TEST_CANCELLED_OUTPUT_CHILD";

    #[test]
    fn cancelled_output_completes_before_unmask() {
        if let Ok(path) = env::var(CANCELLED_CHILD_ENV) {
            // Child: capture SIGINT into a token, then emit through a writer
            // that raises SIGTERM mid-write. The mask must hold through the
            // last write; release then delivers SIGTERM by disposition.
            let mut armed = ArmedSignals::arm().expect("arm");
            signal::raise(Signal::SIGINT).expect("capture first signal");
            assert_eq!(armed.take().expect("take"), Some(Signal::SIGINT as i32));
            let token = armed.into_finalization();
            let mut out = RaisingWriter {
                inner: fs::File::create(path).expect("child output file"),
                raised: false,
            };
            let mut err = io::sink();
            let _ = emit_with(answer(Some(token)), Locale::EnUs, &mut out, &mut err);
            // Reaching here means SIGTERM was lost, not held to release.
            std::process::abort();
        }
        let file = env::temp_dir().join(format!(
            "obemit-cancelled-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let output = Command::new(env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "tests::cancelled_output_completes_before_unmask",
                "--test-threads=1",
                "--nocapture",
            ])
            .env(CANCELLED_CHILD_ENV, &file)
            .stdin(Stdio::null())
            .output()
            .expect("spawn child test");
        assert_eq!(
            output.status.signal(),
            Some(Signal::SIGTERM as i32),
            "the mid-output signal must be delivered at release"
        );
        assert_eq!(
            fs::read(&file).expect("read child output"),
            b"{\"schema\":\"test\"}\n",
            "the complete document lands before the mask is released"
        );
    }

    /// The same child-mode probe armed through `arm_interrupts` — the Agent
    /// INT/TERM watch — proving the finalization token it produces holds the
    /// mask through the output window exactly like the source arm's token.
    const CANCELLED_INTERRUPTS_CHILD_ENV: &str = "ORCHBEE_TEST_CANCELLED_INTERRUPTS_OUTPUT_CHILD";

    #[test]
    fn cancelled_interrupts_output_completes_before_unmask() {
        if let Ok(path) = env::var(CANCELLED_INTERRUPTS_CHILD_ENV) {
            // Child: capture SIGINT into a token through the Agent INT/TERM
            // arm, then emit through a writer that raises SIGTERM mid-write.
            let mut armed = ArmedSignals::arm_interrupts().expect("arm interrupts");
            signal::raise(Signal::SIGINT).expect("capture first signal");
            assert_eq!(armed.take().expect("take"), Some(Signal::SIGINT as i32));
            let token = armed.into_finalization();
            let mut out = RaisingWriter {
                inner: fs::File::create(path).expect("child output file"),
                raised: false,
            };
            let mut err = io::sink();
            let _ = emit_with(answer(Some(token)), Locale::EnUs, &mut out, &mut err);
            std::process::abort();
        }
        let file = env::temp_dir().join(format!(
            "obemit-cancelled-interrupts-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let output = Command::new(env::current_exe().expect("test binary"))
            .args([
                "--exact",
                "tests::cancelled_interrupts_output_completes_before_unmask",
                "--test-threads=1",
                "--nocapture",
            ])
            .env(CANCELLED_INTERRUPTS_CHILD_ENV, &file)
            .stdin(Stdio::null())
            .output()
            .expect("spawn child test");
        assert_eq!(
            output.status.signal(),
            Some(Signal::SIGTERM as i32),
            "the mid-output signal must be delivered at release"
        );
        assert_eq!(
            fs::read(&file).expect("read child output"),
            b"{\"schema\":\"test\"}\n",
            "the complete document lands before the mask is released"
        );
    }
}

#[cfg(all(test, windows))]
mod test_support;
