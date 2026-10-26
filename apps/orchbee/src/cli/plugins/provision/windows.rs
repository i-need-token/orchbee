//! Native Windows Git/build execution; constructing its runner performs no effects.
use super::{
    ArmedSignals, CommandOutput, Instant, NativeRunner, PluginProvisionCommand, Unsupported,
};
use orchbee_local_runtime::{resolve_windows_program, run_windows_command};
use std::process::Command;

struct Runner;

pub(super) fn runner() -> Result<Box<dyn NativeRunner>, Unsupported> {
    Ok(Box::new(Runner))
}

impl NativeRunner for Runner {
    fn run(
        &mut self,
        step: &PluginProvisionCommand,
        deadline: Instant,
        signals: &mut ArmedSignals,
    ) -> CommandOutput {
        let mut command = Command::new(resolve_windows_program(&step.argv[0], &step.directory));
        command.args(&step.argv[1..]).current_dir(&step.directory);
        // Build steps are source provisioning, not participants in a live Session.
        for (name, _) in std::env::vars_os() {
            if name
                .to_string_lossy()
                .to_ascii_uppercase()
                .starts_with("ORCHBEE_")
            {
                command.env_remove(name);
            }
        }
        command
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "never");
        run_windows_command(command, Some(deadline), || {
            signals.take().map(|s| s.is_some())
        })
    }
}
