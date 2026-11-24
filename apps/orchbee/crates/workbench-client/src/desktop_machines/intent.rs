//! What the desktop page may ask of its saved machines, read from the page's record: each act is
//! the request Settings › Machines makes in the TUI, or the person's act on the login question.
//
// module-layer: input

use crate::{
    localization::Locale,
    machine_color::MachineColor,
    machine_settings::{Action, Request},
    machines::ProfileId,
};
use orchbee_machine_protocol::{MAXIMUM_ANSWER_BYTES, wipe};
use orchbee_workbench_protocol::WorkbenchWorkspaceId;
use serde_json::Value;
use std::fmt;

/// The longest machine id or label an intent may carry, in bytes.
const LONGEST: usize = 1_024;

/// One request of the page.
#[derive(Debug, Eq, PartialEq)]
pub enum Intent {
    /// A Settings › Machines request other than Add.
    Settings(Request),
    /// Opens Add Machine, giving up an add in progress.
    AddOpen,
    /// Adds `address` as `name`, blank for the host name, in `color`, the default when absent.
    Add {
        address: String,
        name: String,
        color: Option<MachineColor>,
    },
    /// Closes Add Machine, giving up an add in progress.
    AddClose,
    /// The person's choice on the notice of command login `login`: log in now, or not, which
    /// fails its commands; either closes a notice whose login ended.
    Offer { login: u32, accept: bool },
    /// The person's act on the login question `question` of episode `login`; a question no longer
    /// shown takes none.
    Login {
        login: u32,
        question: u32,
        act: LoginAct,
    },
    /// Change machine: moves `workspace`, named `label`, from the machine whose Session holds it to
    /// `to`; `None` names this computer. The process's mover carries it.
    Move {
        workspace: String,
        label: String,
        from: Option<String>,
        to: Option<String>,
    },
    /// The person's answer to the move's question `question`: move, or give the move up.
    MoveAnswer { question: u64, confirm: bool },
}

/// The person's act on the login question.
#[derive(Debug, Eq, PartialEq)]
pub enum LoginAct {
    /// Send the typed answer, after the episode learns the remember box when it was offered.
    Answer {
        answer: Typed,
        remember: Option<bool>,
    },
    /// Trust a new host: Orchbee answers `yes` itself.
    Trust,
    /// End the login.
    Cancel,
    /// Close the question of a login that ended without the person.
    Close,
}

/// An answer as the page typed it: printable characters within the answer's bound, wiped when
/// dropped and never printed.
#[derive(Eq, PartialEq)]
pub struct Typed(Vec<u8>);

impl Typed {
    /// Takes `text` when every character can be typed into a login field.
    fn admit(text: String) -> Option<Self> {
        let typed = Self(text.into_bytes());
        let text = std::str::from_utf8(&typed.0).ok()?;
        (typed.0.len() <= MAXIMUM_ANSWER_BYTES && !text.chars().any(char::is_control))
            .then_some(typed)
    }

    pub fn characters(&self) -> impl Iterator<Item = char> + '_ {
        std::str::from_utf8(&self.0).unwrap_or_default().chars()
    }
}

impl Drop for Typed {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

impl fmt::Debug for Typed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("Typed")
    }
}

/// The intent `value` names, or `None` for one this build does not carry. An answer is taken out
/// of `value`, so no copy of it stays behind.
#[must_use]
pub fn parse_intent(value: &mut Value) -> Option<Intent> {
    let act = value.get("act")?.as_str()?.to_owned();
    if let Some(act) = act.strip_prefix("login-") {
        return parse_login(act, value);
    }
    let text = |name: &str| {
        value
            .get(name)
            .and_then(Value::as_str)
            .filter(|text| text.len() <= LONGEST)
            .map(str::to_owned)
    };
    match act.as_str() {
        "add-open" => return Some(Intent::AddOpen),
        "offer-accept" | "offer-decline" => {
            let login = value
                .get("login")
                .and_then(Value::as_u64)
                .and_then(|id| u32::try_from(id).ok())?;
            return Some(Intent::Offer {
                login,
                accept: act == "offer-accept",
            });
        }
        "add-close" => return Some(Intent::AddClose),
        "move" => {
            // A saved machine's identity, or null for this computer.
            let machine = |name: &str| match value.get(name) {
                None | Some(Value::Null) => Some(None),
                Some(machine) => {
                    ProfileId::parse(machine.as_str()?).map(|id| Some(id.as_str().to_owned()))
                }
            };
            let workspace = WorkbenchWorkspaceId::parse(value.get("workspace")?.as_str()?).ok()?;
            return Some(Intent::Move {
                workspace: workspace.as_str().to_owned(),
                label: text("label")?,
                from: machine("from")?,
                to: machine("to")?,
            });
        }
        "move-answer" => {
            return Some(Intent::MoveAnswer {
                question: value.get("question")?.as_u64()?,
                confirm: value.get("confirm")?.as_bool()?,
            });
        }
        "add" => {
            let color = match value.get("color") {
                None | Some(Value::Null) => None,
                Some(color) => Some(MachineColor::parse(color.as_str()?)?),
            };
            return Some(Intent::Add {
                address: text("address")?,
                name: text("name")?,
                color,
            });
        }
        _ => {}
    }
    let machine = text("machine")?;
    let act_on = |action| Request::Act {
        machine: machine.clone(),
        action,
    };
    let request = match act.as_str() {
        "disconnect" => act_on(Action::Disconnect),
        "connect" => act_on(Action::Connect),
        "retry" => act_on(Action::Retry),
        "retry-now" => act_on(Action::RetryNow),
        "log-in" => act_on(Action::LogIn),
        "rename" => Request::Rename {
            machine,
            label: text("label")?,
        },
        "set-color" => Request::SetColor {
            machine,
            color: MachineColor::parse(value.get("color")?.as_str()?)?,
        },
        "forget-password" => Request::ForgetPassword(machine),
        "stop" => Request::Stop(machine),
        "remove" => Request::Remove(machine),
        _ => return None,
    };
    Some(Intent::Settings(request))
}

fn parse_login(act: &str, value: &mut Value) -> Option<Intent> {
    let id = |name: &str| {
        value
            .get(name)
            .and_then(Value::as_u64)
            .and_then(|id| u32::try_from(id).ok())
    };
    let (login, question) = (id("login")?, id("question")?);
    let act = match act {
        "answer" => {
            let remember = match value.get("remember") {
                None | Some(Value::Null) => None,
                Some(remember) => Some(remember.as_bool()?),
            };
            let Value::String(answer) = value.get_mut("answer")?.take() else {
                return None;
            };
            LoginAct::Answer {
                answer: Typed::admit(answer)?,
                remember,
            }
        }
        "trust" => LoginAct::Trust,
        "cancel" => LoginAct::Cancel,
        "close" => LoginAct::Close,
        _ => return None,
    };
    Some(Intent::Login {
        login,
        question,
        act,
    })
}

/// The locale a page names by its language tag.
#[must_use]
pub fn parse_locale(tag: &str) -> Option<Locale> {
    match tag {
        "en-US" => Some(Locale::EnUs),
        "zh-CN" => Some(Locale::ZhCn),
        _ => None,
    }
}
