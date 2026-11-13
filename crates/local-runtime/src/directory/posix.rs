//! POSIX terminal directory observations: plain absolute paths and OSC 7 `file:` URIs naming
//! this machine.

/// Decodes a reported POSIX directory as an observation; callers separately admit launchable path
/// syntax. A plain value must be an absolute path and is kept as spelled. A `file://` URI names a
/// local directory only when its host is empty, `localhost`, or the name `local_host` reads,
/// ignoring ASCII case: a shell on another machine, such as one reached over SSH, reports a path
/// that names no directory here. `local_host` runs only for any other host. The URI path is
/// percent-decoded once and must be absolute; a query, fragment, control character or invalid
/// UTF-8 refuses the whole value.
#[must_use]
pub fn posix_reported_directory(
    value: &str,
    local_host: impl FnOnce() -> Option<String>,
) -> Option<String> {
    let path = if let Some(uri) = value.strip_prefix("file://") {
        if uri.contains(['?', '#']) {
            return None;
        }
        let (host, path) = uri.split_at(uri.find('/')?);
        let local = host.is_empty()
            || host.eq_ignore_ascii_case("localhost")
            || local_host()
                .is_some_and(|name| !name.is_empty() && host.eq_ignore_ascii_case(&name));
        if !local {
            return None;
        }
        super::percent_decoded(path)?
    } else {
        value.to_owned()
    };
    if !path.starts_with('/')
        || path.len() > super::OBSERVATION_LIMIT
        || path.chars().any(char::is_control)
    {
        return None;
    }
    Some(path)
}

/// This machine's host name, as shells write it into an OSC 7 URI. Linux publishes it in procfs;
/// a build without that file reads none, so only an empty or `localhost` URI host names this
/// machine there.
#[cfg(unix)]
pub(super) fn local_host_name() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let name = std::fs::read_to_string("/proc/sys/kernel/hostname").ok()?;
        Some(name.trim_end_matches('\n').to_owned()).filter(|name| !name.is_empty())
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}
