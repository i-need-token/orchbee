//! Explicit offline plugin source workflow; this invocation owns every Git/build Job.
use super::super::{
    Failure, FailureKind, Output,
    console_interrupt::{ArmedSignals, Finish},
    failure_output, output, query,
};
use crate::cli_text::CliText;
use crate::{
    invocation::{OutputFormat, plugins::PluginCommand},
    localization::Locale,
};
use orchbee_local_runtime::{
    CommandFailure, CommandOutput, PluginProvisionCommand, PluginProvisionExecutor,
    PluginProvisionFailure, PluginProvisionReport, PluginProvisionStage, PluginRegistry,
};
use std::{
    fmt::Write,
    time::{Duration, Instant},
};

#[cfg(windows)]
#[path = "provision/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "provision/unix.rs"]
mod platform;

struct Unsupported;

trait NativeRunner {
    fn run(
        &mut self,
        step: &PluginProvisionCommand,
        deadline: Instant,
        signals: &mut ArmedSignals,
    ) -> CommandOutput;
}

struct Executor {
    native: Box<dyn NativeRunner>,
    signals: ArmedSignals,
    deadline: Instant,
}
impl PluginProvisionExecutor for Executor {
    fn checkpoint(&mut self) -> Result<(), CommandFailure> {
        if self
            .signals
            .take()
            .map_err(|_| CommandFailure::Io)?
            .is_some()
        {
            Err(CommandFailure::Cancelled)
        } else if Instant::now() >= self.deadline {
            Err(CommandFailure::Timeout)
        } else {
            Ok(())
        }
    }
    fn run(&mut self, step: &PluginProvisionCommand) -> CommandOutput {
        self.native.run(step, self.deadline, &mut self.signals)
    }
}
pub(super) fn run(command: PluginCommand, format: OutputFormat, locale: Locale) -> Output {
    let native = match platform::runner() {
        Ok(native) => native,
        Err(Unsupported) => return super::super::unsupported_platform(Some(format), locale),
    };
    let timeout = match &command {
        PluginCommand::Install { timeout_ms, .. } => *timeout_ms,
        _ => 10_000,
    };
    let operation = if matches!(&command, PluginCommand::Install { .. }) {
        "install"
    } else {
        "uninstall"
    };
    let id = match query::request_id() {
        Ok(id) => id,
        Err(error) => return failure_output(error, Some(format), locale),
    };
    let signals = match ArmedSignals::arm_interrupts() {
        Ok(signals) => signals,
        Err(_) => {
            return failure_output(
                Failure::new(FailureKind::InternalFailure),
                Some(format),
                locale,
            );
        }
    };
    let mut executor = Executor {
        native,
        signals,
        deadline: Instant::now() + Duration::from_millis(u64::from(timeout)),
    };
    let result = PluginRegistry::from_environment(|name| std::env::var_os(name));
    let report = match result {
        Ok(registry) => match command {
            PluginCommand::Install {
                source, reference, ..
            } => registry.install(source, reference, id.as_str(), &mut executor),
            PluginCommand::Uninstall(target) => registry.uninstall(&target, &mut executor),
            _ => unreachable!("only source provisioning commands routed here"),
        },
        Err(error) => PluginProvisionReport {
            stage: PluginProvisionStage::Prepare,
            plugin: None,
            changed: false,
            cleanup_pending: vec![],
            failure: Some(PluginProvisionFailure::Registry(error)),
            output: None,
        },
    };
    let mut finalization = None;
    let (interrupted, cleanup_failed) = match executor.signals.finish() {
        Ok(Finish::Clear) => (false, false),
        Ok(Finish::Cancelled {
            finalization: owed, ..
        }) => {
            finalization = Some(owed);
            (true, false)
        }
        Err(_) => (false, true),
    };
    let status = if cleanup_failed {
        1
    } else if interrupted {
        130
    } else {
        match report.failure {
            None => 0,
            Some(PluginProvisionFailure::Command(CommandFailure::Timeout)) => 7,
            Some(
                PluginProvisionFailure::Command(CommandFailure::PhysicalUnknown)
                | PluginProvisionFailure::PhysicalUnknown,
            ) => 8,
            _ => 1,
        }
    };
    let document = serde_json::json!({"schema":"orchbee.plugin.provision","version":1,"source":{"kind":"local-configuration"},"operation":operation,"result":{"operationId":id,"report":report,"interrupted":interrupted,"cleanupFailed":cleanup_failed}});
    let encoded = match format {
        OutputFormat::Json => output::encode_document(&document),
        OutputFormat::Text => render(&report, id.as_str(), locale),
    };
    let mut result = match encoded {
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
    };
    result.finalization = finalization;
    result
}
fn render(
    report: &PluginProvisionReport,
    operation_id: &str,
    locale: Locale,
) -> Result<Vec<u8>, output::TooLarge> {
    use crate::cli_text::escape::escape;
    let write = |out: &mut output::Bounded| -> Result<(), std::fmt::Error> {
        writeln!(
            out,
            "{}: {operation_id}",
            CliText::new(locale).plugin_runtime_state("operation-id")
        )?;
        writeln!(
            out,
            "{}",
            CliText::new(locale).plugin_provision_stage(report.stage)
        )?;
        if report.changed {
            writeln!(out, "{}", CliText::new(locale).plugin_provision_committed())?;
        }
        if let Some(plugin) = &report.plugin {
            writeln!(
                out,
                "{}: {}",
                escape(&plugin.manifest.id),
                escape(&plugin.plugin_root)
            )?;
        }
        if let Some(error) = &report.failure {
            writeln!(
                out,
                "{}",
                CliText::new(locale).plugin_provision_failure(error)
            )?;
        }
        for path in &report.cleanup_pending {
            writeln!(
                out,
                "{}: {}",
                CliText::new(locale).plugin_cleanup_pending(),
                escape(path)
            )?;
        }
        if let Some(output) = &report.output {
            if let Some(code) = output.exit_code {
                let code = orchbee_terminal_protocol::format_exit_code(code);
                writeln!(
                    out,
                    "{}: {code}",
                    CliText::new(locale).plugin_runtime_state("exit-code")
                )?;
            }
            if !output.stdout.is_empty() {
                writeln!(out, "stdout: {}", escape(&output.stdout))?;
            }
            if !output.stderr.is_empty() {
                writeln!(out, "stderr: {}", escape(&output.stderr))?;
            }
            if output.stdout_truncated || output.stderr_truncated {
                writeln!(
                    out,
                    "{}",
                    CliText::new(locale).plugin_runtime_state("truncated")
                )?;
            }
        }
        Ok(())
    };
    output::render_text(|out| write(out).map_err(|_| output::TooLarge))
}
