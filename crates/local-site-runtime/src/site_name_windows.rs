//! Stable local selection identity; it conveys no runtime or controller authority.
use crate::windows_permissions as permissions;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::{self, Read, Write},
    path::Path,
};

pub(crate) const NAMED_PREFIX: &str = "orchbee-session-";
const FILE_NAME: &str = "session-name.json";
const MAXIMUM_BYTES: u64 = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowsSiteName(String);

impl WindowsSiteName {
    pub fn parse(name: &str) -> io::Result<Self> {
        if name.is_empty()
            || name.len() > 64
            || matches!(name, "." | "..")
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid Windows Session name",
            ));
        }
        Ok(Self(name.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn scope_for_user(&self, sid: &str) -> String {
        if self.0 == "default" {
            return format!("orchbee-native-{:x}", Sha256::digest(sid.as_bytes()))[..47].to_owned();
        }
        let mut hash = Sha256::new();
        hash.update(b"orchbee.windows.site-name.v1\0");
        hash.update(sid.as_bytes());
        hash.update(b"\0");
        hash.update(self.0.as_bytes());
        format!("{NAMED_PREFIX}{:x}", hash.finalize())[..63].to_owned()
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    version: u8,
    name: String,
}

pub(crate) fn read(runtime: &Path) -> io::Result<WindowsSiteName> {
    let file = permissions::open_file(&runtime.join(FILE_NAME), false, false)?;
    let mut bytes = Vec::new();
    file.take(MAXIMUM_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAXIMUM_BYTES {
        return Err(io::ErrorKind::InvalidData.into());
    }
    let record: Record = serde_json::from_slice(&bytes).map_err(|_| io::ErrorKind::InvalidData)?;
    let name = WindowsSiteName::parse(&record.name).map_err(|_| io::ErrorKind::InvalidData)?;
    if record.version != 1 || name.as_str() == "default" {
        return Err(io::ErrorKind::InvalidData.into());
    }
    validate_scope(runtime, &name)?;
    Ok(name)
}

pub(crate) fn validate_scope(runtime: &Path, name: &WindowsSiteName) -> io::Result<()> {
    let scope = name.scope_for_user(&orchbee_local_ipc::current_user_sid()?);
    if runtime.file_name().and_then(|value| value.to_str()) != Some(scope.as_str()) {
        return Err(io::ErrorKind::InvalidData.into());
    }
    Ok(())
}

pub(crate) fn register(runtime: &Path, name: &WindowsSiteName) -> io::Result<()> {
    validate_scope(runtime, name)?;
    if name.as_str() == "default" {
        return Ok(());
    }
    match read(runtime) {
        Ok(existing) if existing == *name => return Ok(()),
        Ok(_) => return Err(io::ErrorKind::InvalidData.into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let bytes = serde_json::to_vec(&Record {
        version: 1,
        name: name.as_str().to_owned(),
    })
    .map_err(io::Error::other)?;
    let mut file = permissions::open_file(&runtime.join(FILE_NAME), true, true)?;
    file.write_all(&bytes)?;
    file.sync_all()
}
