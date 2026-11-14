//! The CLI's public words for the Panel maximized state. The Server's records keep their zoom
//! names on the wire; a machine document says maximize wherever it carries one of them.

use std::borrow::Cow;

use serde_json::Value;

/// A record member's wire name, its public name and the type of value it holds.
type Member = (&'static str, &'static str, fn(&Value) -> bool);

/// Record members renamed with the type of value they hold, so a user-chosen key of another type
/// (a metadata token is a string) keeps its name.
const MEMBERS: &[Member] = &[
    ("zoomed", "maximized", Value::is_boolean),
    ("previouslyZoomed", "previouslyMaximized", Value::is_boolean),
    ("unzoomed", "unmaximized", |value| {
        value.is_object() || value.is_null()
    }),
];

/// Control intent and outcome kinds.
const KINDS: &[(&str, &str)] = &[
    ("zoom", "maximize"),
    ("toggle-zoom", "toggle-maximize"),
    ("zoom-toggled", "maximize-toggled"),
    ("zoom-unchanged", "maximize-unchanged"),
];

/// The document in public words; borrowed unchanged when it carries no zoom name.
pub(crate) fn public(document: &Value) -> Cow<'_, Value> {
    if carries(document) {
        let mut document = document.clone();
        rename(&mut document);
        Cow::Owned(document)
    } else {
        Cow::Borrowed(document)
    }
}

fn carries(value: &Value) -> bool {
    match value {
        Value::Object(object) => object.iter().any(|(key, value)| {
            renamed(key, value).is_some()
                || (key == "kind" && kind(value).is_some())
                || carries(value)
        }),
        Value::Array(items) => items.iter().any(carries),
        _ => false,
    }
}

fn rename(value: &mut Value) {
    match value {
        Value::Object(object) => {
            let keys = object
                .iter()
                .filter_map(|(key, value)| renamed(key, value).map(|to| (key.clone(), to)))
                .collect::<Vec<_>>();
            for (from, to) in keys {
                if let Some(member) = object.remove(&from) {
                    object.insert(to.to_owned(), member);
                }
            }
            if let Some(to) = object.get("kind").and_then(kind) {
                object.insert("kind".to_owned(), Value::from(to));
            }
            object.values_mut().for_each(rename);
        }
        Value::Array(items) => items.iter_mut().for_each(rename),
        _ => {}
    }
}

fn renamed(key: &str, value: &Value) -> Option<&'static str> {
    MEMBERS
        .iter()
        .find(|(from, _, holds)| *from == key && holds(value))
        .map(|(_, to, _)| *to)
}

fn kind(value: &Value) -> Option<&'static str> {
    let value = value.as_str()?;
    KINDS
        .iter()
        .find(|(from, _)| *from == value)
        .map(|(_, to)| *to)
}
