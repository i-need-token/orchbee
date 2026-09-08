use nix::{
    fcntl::{OFlag, openat},
    sys::stat::{Mode, SFlag, fstat},
    unistd::geteuid,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
    time::Instant,
};

use crate::rendezvous::{open_runtime_base, validate_directory};

const MANIFEST_LIMIT: u64 = 16 * 1024;
const ARTIFACTS: [NativeArtifact; 4] = [
    NativeArtifact::Client,
    NativeArtifact::Server,
    NativeArtifact::Worker,
    NativeArtifact::TerminalHost,
];

#[derive(Clone, Copy, Debug)]
pub enum NativeArtifact {
    Client,
    Server,
    Worker,
    TerminalHost,
}
impl NativeArtifact {
    pub const fn file_name(self) -> &'static str {
        match self {
            Self::Client => "orchbee",
            Self::Server => "orchbee-server",
            Self::Worker => "orchbee-worker",
            Self::TerminalHost => "orchbee-terminal-host",
        }
    }
}
#[derive(Debug)]
pub struct NativeCohort {
    directory: PathBuf,
    identity: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    artifacts: Artifacts,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Artifacts {
    orchbee: String,
    #[serde(rename = "orchbee-server")]
    server: String,
    #[serde(rename = "orchbee-worker")]
    worker: String,
    #[serde(rename = "orchbee-terminal-host")]
    host: String,
}
impl Artifacts {
    fn hash(&self, artifact: NativeArtifact) -> &str {
        match artifact {
            NativeArtifact::Client => &self.orchbee,
            NativeArtifact::Server => &self.server,
            NativeArtifact::Worker => &self.worker,
            NativeArtifact::TerminalHost => &self.host,
        }
    }
}

impl NativeCohort {
    pub fn current(deadline: Instant) -> io::Result<Self> {
        let executable = std::env::current_exe()?;
        Self::load(executable.parent().ok_or_else(invalid)?, deadline)
    }
    pub fn load(directory: &Path, deadline: Instant) -> io::Result<Self> {
        remaining(deadline)?;
        let descriptor = open_runtime_base(directory).map_err(io::Error::other)?;
        validate_directory(&descriptor, directory, geteuid().as_raw(), None)
            .map_err(io::Error::other)?;
        let mut encoded = Vec::new();
        open_file(&descriptor, "cohort.json", false)?
            .take(MANIFEST_LIMIT + 1)
            .read_to_end(&mut encoded)?;
        if encoded.len() as u64 > MANIFEST_LIMIT {
            return Err(invalid());
        }
        let manifest: Manifest = serde_json::from_slice(&encoded).map_err(|_| invalid())?;
        if manifest.schema_version != 1 {
            return Err(invalid());
        }
        let mut identity = Sha256::new();
        identity.update(b"orchbee-native-cohort-v1\n");
        for artifact in ARTIFACTS {
            let expected = manifest.artifacts.hash(artifact);
            if expected.len() != 64
                || !expected
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(invalid());
            }
            let mut file = open_file(&descriptor, artifact.file_name(), true)?;
            let mut hash = Sha256::new();
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                remaining(deadline)?;
                let count = file.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                hash.update(&buffer[..count]);
            }
            if format!("{:x}", hash.finalize()) != expected {
                return Err(invalid());
            }
            identity.update(artifact.file_name().as_bytes());
            identity.update([0]);
            identity.update(expected.as_bytes());
            identity.update(b"\n");
        }
        remaining(deadline)?;
        Ok(Self {
            directory: directory.to_path_buf(),
            identity: format!("{:x}", identity.finalize()),
        })
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub fn executable(&self, artifact: NativeArtifact) -> PathBuf {
        self.directory.join(artifact.file_name())
    }
}
fn open_file(directory: &std::os::fd::OwnedFd, name: &str, executable: bool) -> io::Result<File> {
    let fd = openat(
        directory,
        name,
        OFlag::O_RDONLY | OFlag::O_NOFOLLOW | OFlag::O_CLOEXEC | OFlag::O_NONBLOCK,
        Mode::empty(),
    )?;
    let status = fstat(&fd)?;
    if SFlag::from_bits_truncate(status.st_mode) != SFlag::S_IFREG
        || status.st_uid != geteuid().as_raw()
        || status.st_nlink != 1
        || status.st_mode & 0o7022 != 0
        || (executable && status.st_mode & 0o100 == 0)
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unsafe native cohort artifact",
        ));
    }
    #[cfg(target_os = "linux")]
    crate::linux_permissions::require_private(&fd)?;
    Ok(File::from(fd))
}
fn remaining(deadline: Instant) -> io::Result<()> {
    if Instant::now() >= deadline {
        return Err(io::ErrorKind::TimedOut.into());
    }
    Ok(())
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid native development cohort",
    )
}

#[cfg(test)]
mod tests;
