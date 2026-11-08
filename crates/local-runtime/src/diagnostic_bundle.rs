//! Diagnostic bundle rules (ORC-397): the redaction a bundle applies to every document it carries
//! and the archive it writes. The person creates a bundle and sends it themselves; nothing is
//! uploaded.
//!
//! `config.toml` and `internal/state/preferences.json` keep their shape, with every command,
//! argument, environment value and credential replaced by a placeholder, and an unrecognized
//! settings key's value replaced as well. In every carried string the home folder and the value of
//! each folder variable Orchbee reads become placeholders, and a URL loses its user information and
//! query. Comments never reach the bundle, since settings are written again from their values.
mod archive;
pub use archive::diagnostic_archive;
mod redaction;
pub use redaction::DiagnosticRedactor;
mod settings;
pub use settings::{redact_preferences, redact_settings};
#[cfg(test)]
mod tests;

/// The placeholder for a command or executable.
pub const COMMAND_PLACEHOLDER: &str = "<command>";
/// The placeholder for one command argument.
pub const ARGUMENT_PLACEHOLDER: &str = "<argument>";
/// The placeholder for an environment variable value.
pub const ENVIRONMENT_PLACEHOLDER: &str = "<environment>";
/// The placeholder for a credential: a secret-named value or a URL's user information.
pub const CREDENTIAL_PLACEHOLDER: &str = "<credential>";
/// The placeholder for a URL query, which can carry a token.
pub const QUERY_PLACEHOLDER: &str = "<query>";
/// The placeholder for the home folder.
pub const HOME_PLACEHOLDER: &str = "<home>";
/// The placeholder for the value of a settings key this version does not recognize.
pub const UNRECOGNIZED_PLACEHOLDER: &str = "<unrecognized>";
