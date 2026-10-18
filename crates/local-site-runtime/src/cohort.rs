#[cfg(unix)]
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

#[cfg(unix)]
use crate::rendezvous::{open_runtime_base, validate_directory};

const MANIFEST_LIMIT: u64 = 16 * 1024;
const SCHEMA_VERSION: u32 = 2;
const OBSERVATION_ADAPTERS: &str = "agent-observation-adapters";
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
        #[cfg(windows)]
        {
            match self {
                Self::Client => "orchbee.exe",
                Self::Server => "orchbee-server.exe",
                Self::Worker => "orchbee-worker.exe",
                Self::TerminalHost => "orchbee-terminal-host.exe",
            }
        }
        #[cfg(unix)]
        match self {
            Self::Client => "orchbee",
            Self::Server => "orchbee-server",
            Self::Worker => "orchbee-worker",
            Self::TerminalHost => "orchbee-terminal-host",
        }
    }
}

/// A native cohort admission failure with the exact required artifact when
/// opening that sibling reported `NotFound`.
#[derive(Debug)]
pub struct NativeCohortError {
    source: io::Error,
    missing_artifact: Option<NativeArtifact>,
}
impl NativeCohortError {
    fn new(source: io::Error, missing_artifact: Option<NativeArtifact>) -> Self {
        Self {
            source,
            missing_artifact,
        }
    }

    /// The observed operating-system error class.
    pub fn kind(&self) -> io::ErrorKind {
        self.source.kind()
    }

    /// The observed raw operating-system error code, when available.
    pub fn raw_os_error(&self) -> Option<i32> {
        self.source.raw_os_error()
    }

    /// The required sibling proven absent by the artifact-open operation.
    pub fn missing_artifact(&self) -> Option<NativeArtifact> {
        self.missing_artifact
    }

    /// Recover the original I/O error for callers that only use its standard
    /// error interface.
    pub fn into_io_error(self) -> io::Error {
        self.source
    }
}
impl std::fmt::Display for NativeCohortError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.source, formatter)
    }
}
impl std::error::Error for NativeCohortError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
impl From<io::Error> for NativeCohortError {
    fn from(source: io::Error) -> Self {
        Self::new(source, None)
    }
}

#[derive(Debug)]
pub struct NativeCohort {
    directory: PathBuf,
    identity: String,
    #[cfg(windows)]
    artifact_hashes: [String; 4],
    observation_adapters: ObservationAdaptersSidecar,
}

/// The observation-adapters payload a schema-2 cohort manifest declares. Startup
/// never opens, traverses, or validates it; the first integration consumer
/// validates the directory privately against the declared digest and answers a
/// typed unavailable/invalid result, so an absent or damaged sidecar cannot
/// block Site startup or Agent reads.
#[derive(Debug)]
pub struct ObservationAdaptersSidecar {
    directory: PathBuf,
    declared_payload_sha256: String,
}
impl ObservationAdaptersSidecar {
    /// The fixed sibling directory the staged payload occupies.
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    /// The lowercase SHA-256 payload digest the manifest declares.
    pub fn declared_payload_sha256(&self) -> &str {
        &self.declared_payload_sha256
    }
}

/// The version gate runs before the schema-2 shape so a schema-1 or
/// unknown-version manifest answers `incompatible` rather than the generic
/// invalid result its missing members would also produce.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestVersion {
    schema_version: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Manifest {
    #[allow(dead_code)]
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
    #[serde(rename = "agent-observation-adapters")]
    observation_adapters: String,
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
        Self::current_with_diagnostic(deadline).map_err(NativeCohortError::into_io_error)
    }

    /// Validate this executable's sibling cohort while retaining an exact
    /// missing artifact when a required executable was observed absent.
    ///
    /// # Errors
    /// Returns the underlying I/O or validation failure. `missing_artifact`
    /// is populated only when opening one of the declared native siblings
    /// returned `NotFound`.
    pub fn current_with_diagnostic(deadline: Instant) -> Result<Self, NativeCohortError> {
        let executable = std::env::current_exe()?;
        let directory = executable
            .parent()
            .ok_or_else(|| NativeCohortError::from(invalid()))?;
        Self::load_with_diagnostic(directory, deadline)
    }

    pub fn load(directory: &Path, deadline: Instant) -> io::Result<Self> {
        Self::load_with_diagnostic(directory, deadline).map_err(NativeCohortError::into_io_error)
    }

    /// Validate a cohort in `directory`, retaining the exact required
    /// artifact when opening a sibling executable reports `NotFound`.
    ///
    /// # Errors
    /// Returns the underlying I/O or validation failure. `missing_artifact`
    /// is populated only for a missing declared native sibling.
    pub fn load_with_diagnostic(
        directory: &Path,
        deadline: Instant,
    ) -> Result<Self, NativeCohortError> {
        remaining(deadline)?;
        #[cfg(unix)]
        let descriptor = open_runtime_base(directory).map_err(io::Error::other)?;
        #[cfg(unix)]
        validate_directory(&descriptor, directory, geteuid().as_raw(), None)
            .map_err(io::Error::other)?;
        #[cfg(windows)]
        let descriptor = WindowsCohortDirectory {
            _handles: crate::windows_permissions::open_tree(directory)?,
        };
        let mut encoded = Vec::new();
        open_file(
            &descriptor,
            "cohort.json",
            false,
            &directory.join("cohort.json"),
        )?
        .take(MANIFEST_LIMIT + 1)
        .read_to_end(&mut encoded)?;
        if encoded.len() as u64 > MANIFEST_LIMIT {
            return Err(invalid());
        }
        let version: ManifestVersion = serde_json::from_slice(&encoded).map_err(|_| invalid())?;
        if version.schema_version != SCHEMA_VERSION {
            return Err(incompatible());
        }
        let manifest: Manifest = serde_json::from_slice(&encoded).map_err(|_| invalid())?;
        if !declared_hash(&manifest.artifacts.observation_adapters) {
            return Err(invalid());
        }
        let mut identity = Sha256::new();
        identity.update(b"orchbee-native-cohort-v1\n");
        for artifact in ARTIFACTS {
            let expected = manifest.artifacts.hash(artifact);
            if !declared_hash(expected) {
                return Err(invalid());
            }
            let mut file = open_file(
                &descriptor,
                artifact.file_name(),
                true,
                &directory.join(artifact.file_name()),
            )
            .map_err(|source| {
                let missing_artifact =
                    (source.kind() == io::ErrorKind::NotFound).then_some(artifact);
                NativeCohortError::new(source, missing_artifact)
            })?;
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
            #[cfg(windows)]
            artifact_hashes: ARTIFACTS.map(|artifact| manifest.artifacts.hash(artifact).to_owned()),
            observation_adapters: ObservationAdaptersSidecar {
                directory: directory.join(OBSERVATION_ADAPTERS),
                declared_payload_sha256: manifest.artifacts.observation_adapters,
            },
        })
    }
    pub fn identity(&self) -> &str {
        &self.identity
    }
    /// The digest independently verified during this Windows cohort admission.
    /// Comparing retained observations never rereads mutable manifest bytes.
    #[cfg(windows)]
    pub fn artifact_sha256(&self, artifact: NativeArtifact) -> &str {
        &self.artifact_hashes[match artifact {
            NativeArtifact::Client => 0,
            NativeArtifact::Server => 1,
            NativeArtifact::Worker => 2,
            NativeArtifact::TerminalHost => 3,
        }]
    }
    pub fn executable(&self, artifact: NativeArtifact) -> PathBuf {
        self.directory.join(artifact.file_name())
    }
    /// The lazily admitted observation-adapters declaration: path and declared
    /// digest only — never an opened or validated payload.
    pub fn observation_adapters_sidecar(&self) -> &ObservationAdaptersSidecar {
        &self.observation_adapters
    }
}
#[cfg(unix)]
fn open_file(
    directory: &std::os::fd::OwnedFd,
    name: &str,
    executable: bool,
    path: &Path,
) -> io::Result<File> {
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
    #[cfg(target_os = "macos")]
    crate::macos_permissions::require_private(&fd, path)?;
    Ok(File::from(fd))
}
fn remaining(deadline: Instant) -> io::Result<()> {
    if Instant::now() >= deadline {
        return Err(io::ErrorKind::TimedOut.into());
    }
    Ok(())
}
fn declared_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "invalid native development cohort",
    )
}
fn incompatible() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "incompatible native cohort manifest version",
    )
}

#[cfg(all(test, unix))]
mod tests;

#[cfg(windows)]
struct WindowsCohortDirectory {
    _handles: Vec<File>,
}
#[cfg(windows)]
fn open_file(
    _directory: &WindowsCohortDirectory,
    _name: &str,
    _executable: bool,
    path: &Path,
) -> io::Result<File> {
    crate::windows_permissions::open_file(path, false, false)
}
