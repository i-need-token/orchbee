//! The Windows child path limits, which the update checks before it changes anything. They were
//! measured on Windows 11 with the same `Command` calls the update makes: a program path of 259
//! UTF-16 units starts and 260 fails (os error 3); a working directory of 258 starts and 259 fails
//! (os error 267). The install check is tested at those boundaries for the longest program name.
use super::super::{install, path_limit};
use orchbee_local_runtime::CohortArtifact;
use std::path::PathBuf;

/// A path of exactly `units` UTF-16 units.
fn path_of(units: usize, letter: char) -> PathBuf {
    PathBuf::from(format!("C:\\{}", letter.to_string().repeat(units - 3)))
}

#[test]
fn the_child_path_limits_are_the_measured_boundaries() {
    assert!(path_limit::program_fits(&path_of(259, 'p')));
    assert!(!path_limit::program_fits(&path_of(260, 'p')));
    assert!(path_limit::directory_fits(&path_of(258, 'd')));
    assert!(!path_limit::directory_fits(&path_of(259, 'd')));
}

#[test]
fn a_version_whose_longest_program_is_too_long_is_refused_before_it_starts() {
    let version = "0.2.0";
    let longest = [
        CohortArtifact::Client,
        CohortArtifact::Server,
        CohortArtifact::Worker,
        CohortArtifact::TerminalHost,
    ]
    .iter()
    .map(|program| program.file_name().len())
    .max()
    .unwrap_or_default();
    // The program's path is the root followed by `\versions\<version>\<program>`.
    let tail = "\\versions\\".len() + version.len() + 1 + longest;
    let root = |program_units: usize| path_of(program_units - tail, 'r');
    assert!(install::programs_fit(&root(259), version));
    assert!(!install::programs_fit(&root(260), version));
}
