use crate::{
    invocation::{Invocation, UsageFailure},
    localization::Locale,
};

const VERSION_OUTPUT: &str = concat!("orchbee ", env!("CARGO_PKG_VERSION"), "\n");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ProcessResult {
    pub(crate) status: u8,
    pub(crate) stderr: &'static str,
    pub(crate) stdout: &'static str,
}

pub(crate) const fn present(
    invocation: Result<Invocation, UsageFailure>,
    locale: Locale,
) -> ProcessResult {
    match invocation {
        Ok(Invocation::Help) => ProcessResult {
            status: 0,
            stderr: "",
            stdout: locale.help(),
        },
        Ok(Invocation::Tui) => ProcessResult {
            status: 1,
            stderr: locale.tui_unavailable(),
            stdout: "",
        },
        Ok(Invocation::Version) => ProcessResult {
            status: 0,
            stderr: "",
            stdout: VERSION_OUTPUT,
        },
        Err(UsageFailure) => ProcessResult {
            status: 2,
            stderr: locale.usage_failure(),
            stdout: "",
        },
    }
}

#[cfg(test)]
mod tests;
