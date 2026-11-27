//! A temporary repository with its own identity, no signing and no hooks, so the Git configuration
//! of whoever runs the tests cannot change what a test observes. It is removed when dropped. Every
//! Git command a test starts, the engine's own included, runs [`isolate`]d from that configuration.

use super::super::{
    command::{self, Access},
    discovery::Found,
    errors::Classifier,
    output::GitOutput,
    runner::{self, Limits, Ran},
};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

/// Keeps a Git command from reading the configuration of whoever runs the tests: no system or
/// global configuration or attributes, none passed through the environment, and a home that does
/// not exist, where Git would find the global ignore and attributes files.
pub(in crate::git) fn isolate(command: &mut Command) {
    let home = std::env::temp_dir().join("orchbee-files-tests-no-home");
    command
        .env_remove("GIT_CONFIG_PARAMETERS")
        .env_remove("GIT_CONFIG_COUNT")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", home.join(".gitconfig"))
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", &home);
}

pub(super) struct Fixture {
    folder: PathBuf,
    pub(super) output: Mutex<GitOutput>,
}

impl Fixture {
    pub(super) fn new() -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let parent = orchbee_git_process::canonical_path(&std::env::temp_dir()).unwrap();
        let folder = Path::new(parent.as_native()).join(format!(
            "orchbee-files-git-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&folder).unwrap();
        Self {
            folder,
            output: Mutex::new(GitOutput::new()),
        }
    }

    /// A path below the fixture's folder in native spelling; `""` for the folder itself.
    pub(super) fn path(&self, relative: &str) -> String {
        relative
            .split('/')
            .filter(|part| !part.is_empty())
            .fold(self.folder.clone(), |path, part| path.join(part))
            .to_str()
            .unwrap()
            .to_owned()
    }

    /// Creates a repository at `relative` on branch `main` and returns its native path.
    pub(super) fn repository(&self, relative: &str) -> String {
        let path = self.path(relative);
        fs::create_dir_all(&path).unwrap();
        self.git(relative, &["init", "-q", "-b", "main"]);
        self.configure(relative);
        path
    }

    /// The repository at `relative` as discovery reports a working tree that is not linked: its
    /// `.git` is both of its Git directories.
    pub(super) fn found(&self, relative: &str) -> Found {
        let toplevel = self.path(relative);
        let git_dir = Path::new(&toplevel).join(".git");
        Found {
            common_dir: git_dir.clone(),
            git_dir,
            toplevel,
            safe_directory: None,
        }
    }

    /// Gives the repository at `relative` the fixture's identity, no signing and no hooks.
    pub(super) fn configure(&self, relative: &str) {
        for (name, value) in [
            ("user.name", "Orchbee Fixture"),
            ("user.email", "fixture@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.autocrlf", "false"),
            ("core.hooksPath", ".no-hooks"),
        ] {
            self.git(relative, &["config", name, value]);
        }
    }

    pub(super) fn write(&self, relative: &str, content: &str) {
        let path = PathBuf::from(self.path(relative));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    /// Runs Git in `relative` and returns how it ended.
    pub(super) fn run(&self, relative: &str, arguments: &[&str]) -> Ran {
        let mut command = command::git(Path::new(&self.path(relative)), Access::Write, None);
        command.args(arguments);
        runner::run(command, Vec::new(), &self.limits(), &self.output).unwrap()
    }

    /// Runs Git in `relative`, which must succeed, and returns its standard output.
    pub(super) fn git(&self, relative: &str, arguments: &[&str]) -> String {
        let ran = self
            .run(relative, arguments)
            .checked(Classifier::General)
            .unwrap_or_else(|error| panic!("git {arguments:?}: {error:?}"));
        String::from_utf8(ran.stdout).unwrap()
    }

    pub(super) fn limits(&self) -> Limits {
        Limits::new(
            "test",
            Duration::from_secs(60),
            Arc::new(AtomicBool::new(false)),
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.folder);
    }
}
