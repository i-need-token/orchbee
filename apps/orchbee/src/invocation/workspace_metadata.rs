//! Normalize the whole report before opening any runtime connection.
use super::{
    ExpectedValue, FailureEnvelope, OutputFormat, ParseFailure, Request, Resource, Target,
    UsageContext, UsageReason, check_scope, missing_required, target, usage_with,
};
use orchbee_workbench_protocol::{
    MAXIMUM_METADATA_TTL_MILLISECONDS, MetadataSource, MetadataTokenKey, MetadataTokenPatch,
    MetadataTokenValue, WorkbenchRevision,
};
use std::collections::BTreeMap;
#[derive(Default)]
pub(super) struct Options {
    pub(super) updates: Vec<(&'static str, String)>,
    pub(super) sequence: Option<String>,
    pub(super) ttl: Option<String>,
}
impl Options {
    pub(super) fn first_option(&self) -> Option<&'static str> {
        self.first_option_with_sequence(true)
    }
    pub(super) fn first_option_with_sequence(
        &self,
        include_sequence: bool,
    ) -> Option<&'static str> {
        self.updates
            .first()
            .map(|entry| entry.0)
            .or_else(|| {
                self.sequence
                    .as_ref()
                    .filter(|_| include_sequence)
                    .map(|_| "--seq")
            })
            .or_else(|| self.ttl.as_ref().map(|_| "--ttl-ms"))
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceMetadataCommand {
    pub(crate) target: Target,
    pub(crate) source: MetadataSource,
    pub(crate) tokens: MetadataTokenPatch,
    pub(crate) sequence: Option<WorkbenchRevision>,
    pub(crate) ttl_milliseconds: Option<u32>,
}
pub(super) fn parse(
    value: &str,
    options: &super::Options,
    envelope: FailureEnvelope,
) -> Result<Request, ParseFailure> {
    let context = UsageContext::Path("workspace report-metadata");
    check_scope(options, context, envelope)?;
    let report = fields(options, context, envelope, true)?;
    let ReportFields {
        source,
        tokens,
        sequence,
        ttl_milliseconds,
    } = report;
    let tokens = tokens.expect("required token patch");
    Ok(Request::WorkspaceMetadata {
        command: WorkspaceMetadataCommand {
            target: target(value.to_owned(), Resource::Workspace, envelope)?,
            source,
            tokens,
            sequence,
            ttl_milliseconds,
        },
        format: options.format.unwrap_or(OutputFormat::Text),
    })
}

pub(super) struct ReportFields {
    pub(super) source: MetadataSource,
    pub(super) tokens: Option<MetadataTokenPatch>,
    pub(super) sequence: Option<WorkbenchRevision>,
    pub(super) ttl_milliseconds: Option<u32>,
}
pub(super) fn fields(
    options: &super::Options,
    context: UsageContext,
    envelope: FailureEnvelope,
    require_tokens: bool,
) -> Result<ReportFields, ParseFailure> {
    let invalid = |option, value: &str, expected| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option,
                value: value.to_owned(),
                expected,
            },
            envelope,
        )
    };
    let raw = options
        .output_source
        .as_deref()
        .ok_or_else(|| missing_required(context, "'--source'", envelope))?;
    let source = MetadataSource::parse(raw.trim())
        .map_err(|_| invalid("--source", raw, ExpectedValue::MetadataSource))?;
    let mut tokens = BTreeMap::new();
    for (option, raw) in &options.metadata.updates {
        let (key, value) = if *option == "--clear-token" {
            (raw.as_str(), None)
        } else {
            let (key, value) = raw
                .split_once('=')
                .ok_or_else(|| invalid(option, raw, ExpectedValue::MetadataToken))?;
            (key, MetadataTokenValue::normalize(value))
        };
        let key = MetadataTokenKey::parse(key)
            .map_err(|_| invalid(option, raw, ExpectedValue::MetadataToken))?;
        tokens.insert(key, value);
    }
    if tokens.is_empty() && require_tokens {
        return Err(missing_required(
            context,
            "'--token' / '--clear-token'",
            envelope,
        ));
    }
    let tokens = if tokens.is_empty() {
        None
    } else {
        Some(MetadataTokenPatch::new(tokens).map_err(|_| {
            invalid(
                "--token",
                &options.metadata.updates.len().to_string(),
                ExpectedValue::MetadataPatchSize,
            )
        })?)
    };
    let sequence = options
        .metadata
        .sequence
        .as_ref()
        .map(|raw| {
            raw.parse::<u64>()
                .map(WorkbenchRevision::new)
                .map_err(|_| invalid("--seq", raw, ExpectedValue::MetadataSequence))
        })
        .transpose()?;
    let ttl_milliseconds = options
        .metadata
        .ttl
        .as_ref()
        .map(|raw| {
            raw.parse::<u32>()
                .ok()
                .filter(|ttl| (1..=MAXIMUM_METADATA_TTL_MILLISECONDS).contains(ttl))
                .ok_or_else(|| invalid("--ttl-ms", raw, ExpectedValue::MetadataTtl))
        })
        .transpose()?;
    Ok(ReportFields {
        source,
        tokens,
        sequence,
        ttl_milliseconds,
    })
}

#[cfg(test)]
mod tests;
