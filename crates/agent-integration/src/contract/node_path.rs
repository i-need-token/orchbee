//! Lossless executing-node paths. Unicode paths travel as `{"text": …}`; other Unix paths as
//! `{"unixBytesHex": …}` and other Windows paths as `{"windowsUtf16": […]}`. A path is rebuilt
//! only on the platform family that produced it.

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::path::{Path, PathBuf};

const MAXIMUM_PATH_UNITS: usize = 4096;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
enum NodePath {
    Text(String),
    UnixBytesHex(String),
    WindowsUtf16(Vec<u16>),
}

fn encode(path: &Path) -> NodePath {
    if let Some(text) = path.to_str() {
        return NodePath::Text(text.to_owned());
    }
    platform_encode(path)
}

#[cfg(unix)]
fn platform_encode(path: &Path) -> NodePath {
    use std::{fmt::Write, os::unix::ffi::OsStrExt};
    NodePath::UnixBytesHex(path.as_os_str().as_bytes().iter().fold(
        String::new(),
        |mut output, byte| {
            let _ = write!(output, "{byte:02x}");
            output
        },
    ))
}

#[cfg(windows)]
fn platform_encode(path: &Path) -> NodePath {
    use std::os::windows::ffi::OsStrExt;
    NodePath::WindowsUtf16(path.as_os_str().encode_wide().collect())
}

#[cfg(not(any(unix, windows)))]
fn platform_encode(path: &Path) -> NodePath {
    NodePath::Text(path.to_string_lossy().into_owned())
}

fn decode(value: NodePath) -> Option<PathBuf> {
    match value {
        NodePath::Text(text) if text.len() <= MAXIMUM_PATH_UNITS && !text.contains('\0') => {
            Some(PathBuf::from(text))
        }
        NodePath::Text(_) => None,
        NodePath::UnixBytesHex(hex) => unix_path(&hex),
        NodePath::WindowsUtf16(units) => windows_path(&units),
    }
}

#[cfg(unix)]
fn unix_path(hex: &str) -> Option<PathBuf> {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
    if hex.len() % 2 != 0 || hex.len() > MAXIMUM_PATH_UNITS * 2 {
        return None;
    }
    let bytes = (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(hex.get(index..index + 2)?, 16).ok())
        .collect::<Option<Vec<u8>>>()?;
    (!bytes.is_empty() && !bytes.contains(&0)).then(|| PathBuf::from(OsStr::from_bytes(&bytes)))
}

#[cfg(not(unix))]
fn unix_path(_hex: &str) -> Option<PathBuf> {
    None
}

#[cfg(windows)]
fn windows_path(units: &[u16]) -> Option<PathBuf> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    (!units.is_empty() && units.len() <= MAXIMUM_PATH_UNITS && !units.contains(&0))
        .then(|| PathBuf::from(OsString::from_wide(units)))
}

#[cfg(not(windows))]
fn windows_path(_units: &[u16]) -> Option<PathBuf> {
    None
}

fn rebuilt<E: de::Error>(value: NodePath) -> Result<PathBuf, E> {
    decode(value).ok_or_else(|| E::custom("path is not representable on this node"))
}

pub(super) fn serialize<S: Serializer>(path: &Path, serializer: S) -> Result<S::Ok, S::Error> {
    encode(path).serialize(serializer)
}

pub(super) fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<PathBuf, D::Error> {
    rebuilt(NodePath::deserialize(deserializer)?)
}

pub(super) mod optional {
    use super::{NodePath, rebuilt};
    use serde::{Deserialize, Deserializer, Serializer};
    use std::path::PathBuf;

    pub(in crate::contract) fn serialize<S: Serializer>(
        path: &Option<PathBuf>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match path {
            Some(path) => super::serialize(path, serializer),
            None => serializer.serialize_none(),
        }
    }

    pub(in crate::contract) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<PathBuf>, D::Error> {
        Option::<NodePath>::deserialize(deserializer)?
            .map(rebuilt)
            .transpose()
    }
}

pub(super) mod sequence {
    use super::{NodePath, encode, rebuilt};
    use serde::{Deserialize, Deserializer, Serializer, ser::SerializeSeq};
    use std::path::PathBuf;

    pub(in crate::contract) fn serialize<S: Serializer>(
        paths: &[PathBuf],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(paths.len()))?;
        for path in paths {
            sequence.serialize_element(&encode(path))?;
        }
        sequence.end()
    }

    pub(in crate::contract) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<PathBuf>, D::Error> {
        Vec::<NodePath>::deserialize(deserializer)?
            .into_iter()
            .map(rebuilt)
            .collect()
    }
}
