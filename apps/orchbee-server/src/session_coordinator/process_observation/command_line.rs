//! Splits a Host-reported command line with the Windows parser that produced it. Other builds have
//! no such parser and report every line as unsplit instead of guessing shell quoting.

/// Argument vector for one Host-reported command line, or `None` when the Windows parser rejects
/// the line or this build is not Windows. Only Windows Hosts report process command lines.
#[cfg(windows)]
pub(super) fn argv(line: &str) -> Option<Vec<String>> {
    winsafe::CommandLineToArgv(line).ok()
}

/// Argument vector for one Host-reported command line; this build has no Windows parser.
#[cfg(not(windows))]
pub(super) fn argv(_line: &str) -> Option<Vec<String>> {
    None
}
