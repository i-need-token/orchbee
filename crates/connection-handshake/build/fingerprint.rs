//! The development protocol fingerprint that `schemas/connection/README.md` defines, shared by the
//! build script and the reference test so both run the same code. It reads only the listed
//! directories and never follows a symbolic link; choosing the directories belongs to the registry.

use std::{
    fmt::Write as _,
    fs, io,
    path::{Path, PathBuf},
};

use sha2::{Digest, Sha256};

const DOMAIN: &[u8] = b"orchbee-protocol-fingerprint-v1\n";
const INPUT_EXTENSIONS: [&str; 3] = [".rs", ".zig", ".json"];
const EXCLUDED_COMPONENTS: [&str; 2] = ["tests", "__tests__"];
const EXCLUDED_FILE: &str = "tests.rs";

/// Computes the fingerprint of `directories`, repository-relative `/`-separated paths under
/// `repository`.
///
/// # Errors
///
/// Returns `InvalidInput` when the directory list is not strictly bytewise ascending, repeats or
/// nests a directory, or names one that is missing, not a directory, or not a plain relative
/// path; and `InvalidData` for a symbolic link, a non-Unicode name, or another non-file entry
/// inside an input directory. Reading errors pass through.
pub fn compute(repository: &Path, directories: &[&str]) -> io::Result<[u8; 32]> {
    for (index, directory) in directories.iter().enumerate() {
        let plain = !directory.is_empty()
            && directory
                .split('/')
                .all(|component| !component.is_empty() && component != "." && component != "..")
            && !directory.contains('\\');
        if !plain {
            return Err(invalid_input(format!(
                "{directory} is not a plain relative path"
            )));
        }
        let earlier = directories.get(..index).unwrap_or_default();
        if let Some(previous) = earlier.last()
            && previous >= directory
        {
            return Err(invalid_input(format!(
                "{directory} must follow {previous} in strictly bytewise order"
            )));
        }
        // `a-b` sorts between `a` and `a/b`, so nesting is checked against every earlier entry.
        if let Some(outer) = earlier
            .iter()
            .find(|outer| directory.starts_with(&format!("{outer}/")))
        {
            return Err(invalid_input(format!("{directory} is inside {outer}")));
        }
        let metadata = fs::symlink_metadata(native(repository, directory))?;
        if !metadata.is_dir() {
            return Err(invalid_input(format!("{directory} is not a directory")));
        }
    }

    let mut files = Vec::new();
    for directory in directories {
        collect(repository, directory, &mut files)?;
    }
    files.sort_unstable();

    let mut fingerprint = Sha256::new();
    fingerprint.update(DOMAIN);
    for path in &files {
        let digest = Sha256::digest(fs::read(native(repository, path))?);
        let mut hexadecimal = String::with_capacity(64);
        for byte in digest {
            write!(hexadecimal, "{byte:02x}").map_err(io::Error::other)?;
        }
        fingerprint.update(path.as_bytes());
        fingerprint.update([0]);
        fingerprint.update(hexadecimal.as_bytes());
        fingerprint.update(b"\n");
    }
    Ok(fingerprint.finalize().into())
}

fn collect(repository: &Path, directory: &str, files: &mut Vec<String>) -> io::Result<()> {
    for entry in fs::read_dir(native(repository, directory))? {
        let entry = entry?;
        let name = entry.file_name().into_string().map_err(|name| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{directory}/{} is not Unicode", name.to_string_lossy()),
            )
        })?;
        let path = format!("{directory}/{name}");
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{path} is a symbolic link"),
            ));
        }
        let excluded = path
            .split('/')
            .any(|component| EXCLUDED_COMPONENTS.contains(&component));
        if kind.is_dir() {
            if !excluded {
                collect(repository, &path, files)?;
            }
        } else if kind.is_file() {
            let input = INPUT_EXTENSIONS
                .iter()
                .any(|extension| name.ends_with(extension));
            if input && !excluded && name != EXCLUDED_FILE {
                files.push(path);
            }
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{path} is neither a file nor a directory"),
            ));
        }
    }
    Ok(())
}

fn native(repository: &Path, path: &str) -> PathBuf {
    path.split('/')
        .fold(repository.to_path_buf(), |native, component| {
            native.join(component)
        })
}

fn invalid_input(message: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
