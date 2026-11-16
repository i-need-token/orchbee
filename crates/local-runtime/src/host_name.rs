//! This machine's network host name, as `gethostname` reports it: the name zsh, bash and fish write
//! into an OSC 7 URI and show at a prompt. It is not a display name such as the macOS computer
//! name, which may hold spaces and other characters a host name cannot.

/// This machine's host name, or `None` when it cannot be read, is empty or is not UTF-8.
#[must_use]
pub fn local_host_name() -> Option<String> {
    nix::unistd::gethostname()
        .ok()?
        .into_string()
        .ok()
        .filter(|name| !name.is_empty())
}
