//! Offline schema publication stages and residue receipts follow complete output preparation.
use super::{Output, schema};
use crate::cli_text::CliText;
use crate::{invocation::OutputFormat, localization::Locale};

#[cfg(windows)]
#[path = "schema_export/windows.rs"]
mod platform;
#[cfg(not(windows))]
#[path = "schema_export/unix.rs"]
mod platform;

pub(crate) fn run(
    schema_id: Option<&str>,
    path: &str,
    format: OutputFormat,
    locale: Locale,
) -> Output {
    let prepared = schema::answer(schema_id, format, locale);
    if prepared.status != 0 {
        return prepared;
    }
    if let Err(failure) = super::execution::admit(super::execution::Capability::SchemaPublication) {
        return super::failure_output(failure, Some(format), locale);
    }
    match publish(std::path::Path::new(path), &prepared.stdout, "schema") {
        Ok(()) => prepared,
        Err(error) => failure(path, error, format, locale),
    }
}

/// The publication step that failed; the diagnostic bundle shares these stages.
#[derive(Clone, Copy)]
pub(super) enum Stage {
    Destination,
    CreateTemporary,
    WriteTemporary,
    SyncTemporary,
    Publish,
}
impl Stage {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Destination => "destination",
            Self::CreateTemporary => "create-temporary",
            Self::WriteTemporary => "write-temporary",
            Self::SyncTemporary => "sync-temporary",
            Self::Publish => "publish",
        }
    }
}
pub(super) struct PublishFailure {
    pub(super) stage: Stage,
    /// A temporary file that could not be removed.
    pub(super) residue: Option<String>,
}
impl PublishFailure {
    const fn new(stage: Stage) -> Self {
        Self {
            stage,
            residue: None,
        }
    }
    fn cleanup(stage: Stage, temporary: &std::path::Path) -> Self {
        let residue = match std::fs::remove_file(temporary) {
            Ok(()) => None,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => Some(temporary.to_string_lossy().into_owned()),
        };
        Self { stage, residue }
    }
}

/// Writes `bytes` to a temporary file beside `path`, named by `kind`, and renames it over the
/// destination, which may only be a regular file.
pub(super) fn publish(
    path: &std::path::Path,
    bytes: &[u8],
    kind: &str,
) -> Result<(), PublishFailure> {
    use std::{fs, io::Write};
    let destination =
        std::path::absolute(path).map_err(|_| PublishFailure::new(Stage::Destination))?;
    if destination.to_str().is_none() || destination.file_name().is_none() {
        return Err(PublishFailure::new(Stage::Destination));
    }
    match fs::symlink_metadata(&destination) {
        Ok(metadata) => platform::check_destination(&metadata)
            .map_err(|_| PublishFailure::new(Stage::Destination))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Err(PublishFailure::new(Stage::Destination)),
    }
    let parent = destination
        .parent()
        .ok_or_else(|| PublishFailure::new(Stage::Destination))?;
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).map_err(|_| PublishFailure::new(Stage::CreateTemporary))?;
    let temporary = parent.join(format!(
        ".orchbee-{kind}-{:032x}.tmp",
        u128::from_le_bytes(nonce)
    ));
    let mut file = platform::create_temporary(&temporary)
        .map_err(|_| PublishFailure::new(Stage::CreateTemporary))?;
    if file.write_all(bytes).is_err() {
        drop(file);
        return Err(PublishFailure::cleanup(Stage::WriteTemporary, &temporary));
    }
    if file.sync_all().is_err() {
        drop(file);
        return Err(PublishFailure::cleanup(Stage::SyncTemporary, &temporary));
    }
    drop(file);
    fs::rename(&temporary, &destination)
        .map_err(|_| PublishFailure::cleanup(Stage::Publish, &temporary))
}

fn failure(path: &str, error: PublishFailure, format: OutputFormat, locale: Locale) -> Output {
    use serde_json::json;
    let document = json!({
        "schema": "orchbee.cli.failure.schema-export",
        "version": 1,
        "source": schema::embedded_source(),
        "failure": {
            "kind": "write-failed",
            "details": {
                "stage": error.stage.as_str(),
                "path": path,
                "temporaryPath": error.residue.as_deref(),
            }
        }
    });
    let stdout = if format == OutputFormat::Json {
        match super::output::encode_document(&document) {
            Ok(bytes) => bytes,
            Err(_) => {
                return super::failure_output(
                    super::Failure::new(super::FailureKind::InternalFailure),
                    Some(format),
                    locale,
                );
            }
        }
    } else {
        Vec::new()
    };
    Output {
        status: 1,
        stdout,
        stderr: CliText::new(locale).schema_export_failure(
            error.stage.as_str(),
            path,
            error.residue.as_deref(),
        ),
        finalization: None,
    }
}
