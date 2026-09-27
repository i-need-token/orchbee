//! Normalize the whole report before opening any runtime connection.
use super::{
    ExpectedValue, FailureEnvelope, NO_OPTIONS, OutputFormat, ParseFailure, Request, Resource,
    Scope, Target, UsageContext, UsageReason, check_scope, missing_required, target, usage_with,
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
        self.updates
            .first()
            .map(|entry| entry.0)
            .or_else(|| self.sequence.as_ref().map(|_| "--seq"))
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
    check_scope(
        options,
        Scope {
            metadata: true,
            output_source: true,
            ..NO_OPTIONS
        },
        context,
        envelope,
    )?;
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
    if tokens.is_empty() {
        return Err(missing_required(
            context,
            "'--token' / '--clear-token'",
            envelope,
        ));
    }
    let tokens = MetadataTokenPatch::new(tokens).map_err(|_| {
        invalid(
            "--token",
            &options.metadata.updates.len().to_string(),
            ExpectedValue::MetadataPatchSize,
        )
    })?;
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

#[cfg(test)]
mod tests {
    use super::*;
    fn parse_args(args: &[&str]) -> Result<Request, ParseFailure> {
        crate::invocation::parse_request(args.iter().map(std::ffi::OsString::from))
    }
    #[test]
    fn metadata_normalization_order_and_counter_limits_precede_admission() {
        let Request::WorkspaceMetadata { command, format } = parse_args(&[
            "workspace",
            "report-metadata",
            "main",
            "--source= hook:agent ",
            "--token",
            "state=old",
            "--clear-token",
            "state",
            "--token=state= \n 新\t值 ",
            "--token=empty=   ",
            "--seq=18446744073709551615",
            "--ttl-ms=86400000",
            "--format=json",
        ])
        .unwrap() else {
            panic!("metadata command")
        };
        assert_eq!(format, OutputFormat::Json);
        assert_eq!(command.source.as_str(), "hook:agent");
        assert_eq!(
            command.tokens.as_map()[&MetadataTokenKey::parse("state").unwrap()]
                .as_ref()
                .unwrap()
                .as_str(),
            "新值"
        );
        assert!(command.tokens.as_map()[&MetadataTokenKey::parse("empty").unwrap()].is_none());
        assert_eq!(command.sequence, Some(WorkbenchRevision::new(u64::MAX)));
        for tail in [
            vec!["--seq=18446744073709551616"],
            vec!["--seq=-1"],
            vec!["--seq=1", "--seq=2"],
            vec!["--ttl-ms=0"],
            vec!["--ttl-ms=86400001"],
            vec!["--token=bad.key=value"],
            vec!["--token=missing-equals"],
            vec!["--clear-token=bad.key"],
            vec!["--focus"],
            vec!["--timeout=1"],
        ] {
            let args = [
                vec![
                    "workspace",
                    "report-metadata",
                    "main",
                    "--source=hook",
                    "--token=a=b",
                ],
                tail,
            ]
            .concat();
            assert!(parse_args(&args).is_err(), "{args:?}");
        }
        for args in [
            vec!["workspace", "report-metadata", "main", "--token=a=b"],
            vec!["workspace", "report-metadata", "main", "--source=hook"],
            vec!["workspace", "list", "--token=a=b"],
            vec!["--seq=1", "--help"],
        ] {
            assert!(parse_args(&args).is_err(), "{args:?}");
        }
        let mut args = vec![
            "workspace".to_owned(),
            "report-metadata".into(),
            "main".into(),
            "--source=hook".into(),
        ];
        args.extend((0..17).map(|n| format!("--token=k{n}=v")));
        assert!(parse_args(&args.iter().map(String::as_str).collect::<Vec<_>>()).is_err());
    }
}
