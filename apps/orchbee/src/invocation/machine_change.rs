//! `workspace set-machine`: the Workspace operand, the machine it goes to, and whether the command
//! only previews. The `--machine` prefix names the machine that holds the Workspace and never
//! routes this command; it performs no catalog read, hub request or Session exchange.

use super::{
    ExpectedValue, FailureEnvelope, Options, OutputFormat, ParseFailure, Request, Resource,
    UsageContext, UsageReason, check_scope, machines::valid_selector, missing_required, required,
    target, usage_with,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SetMachine {
    /// The Workspace operand as typed; the machine that holds the Workspace resolves it.
    pub(crate) workspace: String,
    /// `--to`: `local`, or a saved machine's ID or exact name.
    pub(crate) to: String,
    /// The `--machine` prefix: the machine that holds the Workspace; `None` is this computer.
    pub(crate) from: Option<String>,
    /// `--dry-run`: names what would close and changes nothing; otherwise `--confirm` was given.
    pub(crate) dry_run: bool,
    pub(crate) format: OutputFormat,
}

pub(super) fn parse(
    value: &str,
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("workspace set-machine");
    check_scope(options, context, envelope)?;
    target(value.to_owned(), Resource::Workspace, envelope)?;
    let to = required(&options.to, context, "'--to'", envelope)?;
    // `local` is a valid selector too, and always names this computer.
    if !valid_selector(&to) {
        return Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--to",
                value: to,
                expected: ExpectedValue::MachineSelector,
            },
            envelope,
        ));
    }
    // As with `workspace close`, a dry run discards nothing and needs no `--confirm`; the change
    // itself ends the Workspace's terminals here.
    if !options.dry_run && !options.confirm {
        return Err(missing_required(context, "'--confirm'", envelope));
    }
    Ok(Request::WorkspaceSetMachine(SetMachine {
        workspace: value.to_owned(),
        to,
        from: None,
        dry_run: options.dry_run,
        format: options.format.unwrap_or(OutputFormat::Text),
    }))
}
