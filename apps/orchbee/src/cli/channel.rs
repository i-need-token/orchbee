//! Offline release preference; installed cohort identity is a separate observation.
use super::{Failure, FailureKind, Output, configuration, failure_output, output};
use crate::{invocation::OutputFormat, localization::Locale};
use orchbee_local_runtime::read_user_settings;
use serde_json::json;

pub(crate) fn show(format: OutputFormat, locale: Locale) -> Output {
    let settings = match read_user_settings(|name| std::env::var_os(name)) {
        Ok(settings) => settings,
        Err(error) => {
            return failure_output(
                configuration::Error::from(error).failure(),
                Some(format),
                locale,
            );
        }
    };
    let channel = settings.update_channel.as_str();
    let encoded = match format {
        OutputFormat::Text => output::encode_text(&format!("{channel}\n")),
        OutputFormat::Json => output::encode_document(&json!({
            "schema": "orchbee.channel.show",
            "version": 1,
            "source": { "kind": "local-configuration" },
            "result": {
                "channel": channel,
                "source": settings.update_channel_source,
                "path": settings.path.as_deref().and_then(std::path::Path::to_str),
                "exists": settings.exists,
            },
        })),
    };
    match encoded {
        Ok(stdout) => Output {
            status: 0,
            stdout,
            stderr: String::new(),
            finalization: None,
        },
        Err(_) => failure_output(
            Failure::new(FailureKind::OutputTooLarge),
            Some(format),
            locale,
        ),
    }
}
