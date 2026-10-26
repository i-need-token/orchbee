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

fn valid_path(value: &str) -> bool {
    let path = std::path::Path::new(value);
    if value.is_empty()
        || value.len() > 4096
        || value.chars().any(char::is_control)
        || value.ends_with(['/', '\\'])
        || path.file_name().is_none()
        || matches!(value.rsplit(['/', '\\']).next(), Some("." | ".."))
    {
        return false;
    }
    {
        use std::path::{Component, Prefix};
        if let Some(Component::Prefix(prefix)) = path.components().next() {
            if !path.is_absolute()
                || !matches!(
                    prefix.kind(),
                    Prefix::Disk(_)
                        | Prefix::VerbatimDisk(_)
                        | Prefix::UNC(_, _)
                        | Prefix::VerbatimUNC(_, _)
                )
            {
                return false;
            }
        } else if path.has_root() {
            return false;
        }
        if !path.components().all(|component| match component {
            Component::Normal(name) => name.to_str().is_some_and(|name| {
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
            }),
            _ => true,
        }) {
            return false;
        }
    }
    true
}
