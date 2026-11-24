//! What the desktop page may ask of its saved machines, read from the page's record: each act is
//! the request Settings › Machines makes in the TUI.
//
// module-layer: input

use crate::{
    localization::Locale,
    machine_color::MachineColor,
    machine_settings::{Action, Request},
};
use serde_json::Value;

/// The longest machine id or label an intent may carry, in bytes.
const LONGEST: usize = 1_024;

/// One request of the page.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Intent {
    /// A Settings › Machines request other than Add.
    Settings(Request),
}

/// The intent `value` names, or `None` for one this build does not carry.
#[must_use]
pub fn parse_intent(value: &Value) -> Option<Intent> {
    let text = |name: &str| {
        value
            .get(name)
            .and_then(Value::as_str)
            .filter(|text| text.len() <= LONGEST)
            .map(str::to_owned)
    };
    let machine = text("machine")?;
    let act = |action| Request::Act {
        machine: machine.clone(),
        action,
    };
    let request = match value.get("act")?.as_str()? {
        "disconnect" => act(Action::Disconnect),
        "connect" => act(Action::Connect),
        "retry" => act(Action::Retry),
        "retry-now" => act(Action::RetryNow),
        "log-in" => act(Action::LogIn),
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

/// The locale a page names by its language tag.
#[must_use]
pub fn parse_locale(tag: &str) -> Option<Locale> {
    match tag {
        "en-US" => Some(Locale::EnUs),
        "zh-CN" => Some(Locale::ZhCn),
        _ => None,
    }
}
