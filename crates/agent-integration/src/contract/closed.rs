//! Closed report vocabularies whose writer-side spellings are static strings. Their wire forms
//! carry owned text and convert only into a known member.

use super::{CAPSULE_SOURCE_KINDS, CapsuleStatus, FILESYSTEM_OPERATIONS, StepKind, node_path};
use serde::{Deserialize, Deserializer, de};
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CapsuleStatusWire {
    ready: bool,
    #[serde(with = "node_path")]
    root: PathBuf,
    source_kind: String,
    binding_failure: bool,
    payload_sha256: String,
}

impl TryFrom<CapsuleStatusWire> for CapsuleStatus {
    type Error = &'static str;

    fn try_from(wire: CapsuleStatusWire) -> Result<Self, Self::Error> {
        Ok(Self {
            ready: wire.ready,
            root: wire.root,
            source_kind: member(&wire.source_kind, &CAPSULE_SOURCE_KINDS)?,
            binding_failure: wire.binding_failure,
            payload_sha256: wire.payload_sha256,
        })
    }
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "kebab-case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
enum StepKindWire {
    Command {
        executable: String,
        argv: Vec<String>,
    },
    Filesystem {
        operation: String,
        #[serde(with = "node_path")]
        path: PathBuf,
    },
}

impl TryFrom<StepKindWire> for StepKind {
    type Error = &'static str;

    fn try_from(wire: StepKindWire) -> Result<Self, Self::Error> {
        Ok(match wire {
            StepKindWire::Command { executable, argv } => Self::Command { executable, argv },
            StepKindWire::Filesystem { operation, path } => Self::Filesystem {
                operation: member(&operation, &FILESYSTEM_OPERATIONS)?,
                path,
            },
        })
    }
}

impl<'de> Deserialize<'de> for CapsuleStatus {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_from(CapsuleStatusWire::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

impl<'de> Deserialize<'de> for StepKind {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::try_from(StepKindWire::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

fn member(value: &str, vocabulary: &[&'static str]) -> Result<&'static str, &'static str> {
    vocabulary
        .iter()
        .copied()
        .find(|known| *known == value)
        .ok_or("value outside the closed report vocabulary")
}
