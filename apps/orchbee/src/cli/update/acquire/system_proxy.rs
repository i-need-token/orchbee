//! The per-user system proxy as curl arguments for one HTTPS transfer, when the environment names
//! no proxy. Only the manual settings count: an automatic configuration script or automatic
//! detection is never read. A setting that is absent, off, of another type or not understood gives
//! no argument, so a download never fails because a setting could not be read.
use std::ffi::OsString;

/// One value of the per-user proxy settings as the platform read it, before it means anything.
#[cfg_attr(
    not(any(windows, test)),
    expect(
        dead_code,
        reason = "only Windows reads system proxy settings, where downloads run"
    )
)]
pub(in crate::cli::update) enum Value {
    Absent,
    Number(u32),
    Text(String),
    /// Another type, or a value that could not be read.
    Other,
}

/// The three manual proxy values: whether the proxy is on, its server or per-scheme server list,
/// and its bypass list.
pub(in crate::cli::update) struct Settings {
    pub(in crate::cli::update) enabled: Value,
    pub(in crate::cli::update) server: Value,
    pub(in crate::cli::update) bypass: Value,
}

/// Either variable present means the environment chooses the proxy and curl reads it unchanged;
/// the system settings are then not read at all.
const PROXY_VARIABLES: [&str; 2] = ["HTTPS_PROXY", "ALL_PROXY"];
/// Present, it stays curl's bypass list unchanged and applies to the system proxy too, so the
/// system bypass list is not passed.
const NO_PROXY_VARIABLE: &str = "NO_PROXY";
/// The ports Windows assumes when a server names none; curl would assume 1080 for both.
const HTTP_PORT: u16 = 80;
const SOCKS_PORT: u16 = 1080;

/// The curl arguments for the system proxy: none while `environment` holds [`PROXY_VARIABLES`],
/// otherwise `--proxy` and, unless `environment` holds [`NO_PROXY_VARIABLE`], `--noproxy` from what
/// `settings` reads. `settings` runs only when the environment names no proxy.
pub(super) fn arguments(
    environment: &dyn Fn(&str) -> Option<OsString>,
    settings: impl FnOnce() -> Option<Settings>,
) -> Vec<String> {
    if PROXY_VARIABLES
        .into_iter()
        .any(|name| environment(name).is_some())
    {
        return Vec::new();
    }
    let Some(proxy) = settings().as_ref().and_then(proxy) else {
        return Vec::new();
    };
    let mut arguments = vec!["--proxy".to_owned(), proxy.server];
    if environment(NO_PROXY_VARIABLE).is_none() && !proxy.bypass.is_empty() {
        arguments.push("--noproxy".to_owned());
        arguments.push(proxy.bypass.join(","));
    }
    arguments
}

/// The proxy an HTTPS transfer takes and the hosts that skip it.
struct Proxy {
    server: String,
    bypass: Vec<String>,
}

fn proxy(settings: &Settings) -> Option<Proxy> {
    if !matches!(settings.enabled, Value::Number(flag) if flag != 0) {
        return None;
    }
    let Value::Text(server) = &settings.server else {
        return None;
    };
    let bypass = match &settings.bypass {
        Value::Absent => Vec::new(),
        Value::Text(list) => bypass(list),
        Value::Number(_) | Value::Other => return None,
    };
    Some(Proxy {
        server: https_server(server)?,
        bypass,
    })
}

/// The proxy URL for HTTPS. One server serves every scheme as an HTTP proxy. A per-scheme list
/// (`http=a:1;https=b:2;socks=c:3`) gives its `https` entry as an HTTP proxy, else its `socks`
/// entry as SOCKS4, which is the version Windows speaks, else nothing, as Windows connects directly
/// then. A list that mixes both forms is not understood.
fn https_server(value: &str) -> Option<String> {
    let value = value.trim();
    if !value.contains('=') {
        return address(value, "http", HTTP_PORT);
    }
    let mut https = None;
    let mut socks = None;
    for entry in value
        .split(';')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let (scheme, address) = entry.split_once('=')?;
        let slot = match scheme.trim().to_ascii_lowercase().as_str() {
            "https" => &mut https,
            "socks" => &mut socks,
            _ => continue,
        };
        slot.get_or_insert(address.trim());
    }
    match (https, socks) {
        (Some(server), _) => address(server, "http", HTTP_PORT),
        (None, Some(server)) => address(server, "socks4", SOCKS_PORT),
        (None, None) => None,
    }
}

/// `host[:port]` as `<scheme>://host:port`, with `port` when none is named. An HTTP proxy may carry
/// an `http://` prefix. The host is a name, an IPv4 address or a bracketed IPv6 address; user
/// information, a path or any other scheme is not understood.
fn address(value: &str, scheme: &str, default_port: u16) -> Option<String> {
    let value = match value.get(..7) {
        Some(prefix) if scheme == "http" && prefix.eq_ignore_ascii_case("http://") => &value[7..],
        _ => value,
    };
    // `rest` is empty or `:port`.
    let (host, rest) = match value.strip_prefix('[') {
        Some(inside) => {
            let (inner, rest) = inside.split_once(']')?;
            if inner.is_empty() || !inner.bytes().all(|b| b.is_ascii_hexdigit() || b == b':') {
                return None;
            }
            (&value[..inner.len() + 2], rest)
        }
        None => {
            let split = value.find(':').unwrap_or(value.len());
            let (host, rest) = value.split_at(split);
            if !host_name(host) {
                return None;
            }
            (host, rest)
        }
    };
    let port = match rest {
        "" => default_port,
        _ => parse_port(rest.strip_prefix(':')?)?,
    };
    Some(format!("{scheme}://{host}:{port}"))
}

fn parse_port(text: &str) -> Option<u16> {
    if text.is_empty() || text.len() > 5 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse::<u16>().ok().filter(|port| *port > 0)
}

/// A host name or IPv4 address in the characters curl's lists and URLs take as they are.
fn host_name(text: &str) -> bool {
    !text.is_empty()
        && !text.starts_with(['.', '-'])
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

/// The bypass entries curl can express, in order. A name or address stays as it is; `*.name`
/// becomes `name`, which curl matches with its subdomains (and so also `name` itself); IPv4
/// patterns ending in `*` octets become CIDR ranges; `*` alone skips the proxy for every host.
/// `<local>` (host names without a dot) and any other pattern have no curl form and are left out;
/// every download host is a dotted name.
fn bypass(list: &str) -> Vec<String> {
    list.split(';')
        .map(str::trim)
        .filter_map(|entry| {
            if entry == "*" {
                return Some(entry.to_owned());
            }
            if let Some(range) = ipv4_range(entry) {
                return Some(range);
            }
            let name = entry.strip_prefix("*.").unwrap_or(entry);
            host_name(name).then(|| name.to_owned())
        })
        .collect()
}

/// `a.*`, `a.b.*`, `a.b.c.*` (trailing `*` octets may repeat up to four parts) as a CIDR range.
fn ipv4_range(entry: &str) -> Option<String> {
    let parts: Vec<&str> = entry.split('.').collect();
    if parts.len() > 4 || !entry.ends_with('*') {
        return None;
    }
    let fixed = parts.iter().take_while(|part| **part != "*").count();
    if fixed == 0 || parts[fixed..].iter().any(|part| *part != "*") {
        return None;
    }
    let mut octets = [0_u8; 4];
    for (octet, part) in octets.iter_mut().zip(&parts[..fixed]) {
        if part.is_empty() || part.len() > 3 || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *octet = part.parse().ok()?;
    }
    let [a, b, c, d] = octets;
    Some(format!("{a}.{b}.{c}.{d}/{}", fixed * 8))
}

#[cfg(test)]
mod tests;
