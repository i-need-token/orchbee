//! Pure host selection and bounded OSC payload construction; lifecycle owns writes.
use orchbee_workbench_protocol::Notification;

/// Frozen host facts supplied only after the terminal-notification capability is admitted.
pub(crate) struct Host {
    pub(crate) program: Option<String>,
    pub(crate) term: Option<String>,
    pub(crate) kitty: bool,
    pub(crate) tmux: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Backend {
    Osc9,
    Kitty,
}
fn backend(program: Option<&str>, term: Option<&str>, kitty: bool) -> Option<Backend> {
    if let Some("ghostty" | "iTerm.app" | "WezTerm") = program {
        return Some(Backend::Osc9);
    }
    if kitty {
        return Some(Backend::Kitty);
    }
    match term {
        Some("xterm-ghostty") => Some(Backend::Osc9),
        Some("xterm-kitty") => Some(Backend::Kitty),
        Some(value) if value.contains("wezterm") => Some(Backend::Osc9),
        _ => None,
    }
}
pub(crate) fn sequence(message: &Notification, host: &Host) -> std::io::Result<Vec<u8>> {
    let selected = backend(host.program.as_deref(), host.term.as_deref(), host.kitty)
        .ok_or(std::io::ErrorKind::Unsupported)?;
    Ok(encode(selected, message, host.tmux))
}
fn encode(backend: Backend, message: &Notification, tmux: bool) -> Vec<u8> {
    let clean = |value: &str| {
        value
            .chars()
            .filter(|ch| !ch.is_control())
            .collect::<String>()
    };
    let title = clean(&message.title);
    let body = message.body.as_deref().map(clean);
    let sequence = match (backend, body.as_deref()) {
        (Backend::Osc9, Some(body)) => format!("\x1b]9;{title}: {body}\x1b\\"),
        (Backend::Osc9, None) => format!("\x1b]9;{title}\x1b\\"),
        (Backend::Kitty, Some(body)) => {
            format!("\x1b]99;i=1:d=0;{title}\x1b\\\x1b]99;i=1:p=body;{body}\x1b\\")
        }
        (Backend::Kitty, None) => format!("\x1b]99;;{title}\x1b\\"),
    };
    if !tmux {
        return sequence.into_bytes();
    }
    let mut bytes = b"\x1bPtmux;".to_vec();
    for byte in sequence.bytes() {
        bytes.push(byte);
        if byte == 27 {
            bytes.push(byte);
        }
    }
    bytes.extend_from_slice(b"\x1b\\");
    bytes
}
#[cfg(test)]
mod tests;
