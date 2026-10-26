//! Offline schema export grammar: validate the destination without filesystem effects.
use super::OutputFormat;
use super::{
    ExpectedValue, FailureEnvelope, Options, ParseFailure, QueryCommand, Request, Resource,
    UsageContext, UsageReason, check_scope, identifier, query, usage_with,
};

pub(super) fn request(
    schema_id: Option<&str>,
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("api schema");
    check_scope(options, context, envelope)?;
    if schema_id.is_some_and(str::is_empty) {
        return Err(identifier(Resource::Schema, String::new(), envelope));
    }
    let Some(path) = options.output.as_ref() else {
        return Ok(query(
            QueryCommand::ApiSchema {
                schema_id: schema_id.map(str::to_owned),
            },
            options,
        ));
    };
    if !valid_path(path) {
        return Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--output",
                value: path.clone(),
                expected: ExpectedValue::OutputFilePath,
            },
            envelope,
        ));
    }
    Ok(Request::SchemaExport {
        schema_id: schema_id.map(str::to_owned),
        path: path.clone(),
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

/// Windows publication syntax is interpreted lexically on every host.
fn valid_path(value: &str) -> bool {
    if value.is_empty()
        || value.len() > 4096
        || value.chars().any(char::is_control)
        || value.ends_with(['/', '\\'])
    {
        return false;
    }
    let normalized = value.replace('\\', "/");
    let drive_tail = |value: &str| {
        let bytes = value.as_bytes();
        (bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/')
            .then_some(3)
    };
    let mut tail = normalized.as_str();
    if let Some(verbatim) = tail.strip_prefix("//?/") {
        if let Some(unc) = verbatim
            .strip_prefix("UNC/")
            .or_else(|| verbatim.strip_prefix("unc/"))
        {
            let Some((_, rest)) = unc.split_once('/') else {
                return false;
            };
            let Some((_, rest)) = rest.split_once('/') else {
                return false;
            };
            tail = rest;
        } else if let Some(offset) = drive_tail(verbatim) {
            tail = &verbatim[offset..];
        } else {
            return false;
        }
    } else if let Some(unc) = tail.strip_prefix("//") {
        if unc.starts_with("./") {
            return false;
        }
        let Some((server, rest)) = unc.split_once('/') else {
            return false;
        };
        let Some((share, rest)) = rest.split_once('/') else {
            return false;
        };
        if server.is_empty() || share.is_empty() {
            return false;
        }
        tail = rest;
    } else if let Some(offset) = drive_tail(tail) {
        tail = &tail[offset..];
    } else if tail.starts_with('/') {
        return false;
    }
    let components = tail
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if components.is_empty() || matches!(components.last(), Some(&"." | &"..")) {
        return false;
    }
    components.iter().all(|name| {
        if matches!(*name, "." | "..") {
            return true;
        }
        let stem = name
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        !name.contains([':', '*', '?', '"', '<', '>', '|'])
            && !name.ends_with(['.', ' '])
            && !matches!(
                stem.as_str(),
                "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
            )
            && !((stem.starts_with("COM") || stem.starts_with("LPT"))
                && matches!(
                    &stem[3..],
                    "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
                ))
    })
}
