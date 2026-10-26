//! Explicit offline Windows schema-file publication, after complete output preparation.
use super::{Output, schema};
use crate::{invocation::OutputFormat, localization::Locale};

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
    #[cfg(windows)]
    {
        match publish(std::path::Path::new(path), &prepared.stdout) {
            Ok(()) => prepared,
            Err(error) => failure(path, error, format, locale),
        }
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        super::failure_output(
            super::Failure::new(super::FailureKind::ServiceUnavailable).details(
                crate::cli::stage_reason_details("platform", "unsupported-platform"),
            ),
            Some(format),
            locale,
        )
    }
}

#[cfg(windows)]
#[derive(Clone, Copy)]
enum Stage {
    Destination,
    CreateTemporary,
    WriteTemporary,
    SyncTemporary,
    Publish,
}
#[cfg(windows)]
impl Stage {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Destination => "destination",
            Self::CreateTemporary => "create-temporary",
            Self::WriteTemporary => "write-temporary",
            Self::SyncTemporary => "sync-temporary",
            Self::Publish => "publish",
        }
    }
}
#[cfg(windows)]
struct PublishFailure {
    stage: Stage,
    residue: Option<String>,
}
#[cfg(windows)]
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

#[cfg(windows)]
fn publish(path: &std::path::Path, bytes: &[u8]) -> Result<(), PublishFailure> {
    use std::{
        fs::{self, OpenOptions},
        io::Write,
        os::windows::fs::{MetadataExt, OpenOptionsExt},
    };
    let destination =
        std::path::absolute(path).map_err(|_| PublishFailure::new(Stage::Destination))?;
    if destination.to_str().is_none() || destination.file_name().is_none() {
        return Err(PublishFailure::new(Stage::Destination));
    }
    match fs::symlink_metadata(&destination) {
        Ok(metadata) if metadata.is_file() && metadata.file_attributes() & 0x400 == 0 => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        _ => return Err(PublishFailure::new(Stage::Destination)),
    }
    let parent = destination
        .parent()
        .ok_or_else(|| PublishFailure::new(Stage::Destination))?;
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).map_err(|_| PublishFailure::new(Stage::CreateTemporary))?;
    let temporary = parent.join(format!(
        ".orchbee-schema-{:032x}.tmp",
        u128::from_le_bytes(nonce)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .share_mode(0)
        .open(&temporary)
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

#[cfg(windows)]
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
        stderr: locale.schema_export_failure(error.stage.as_str(), path, error.residue.as_deref()),
        finalization: None,
    }
}
