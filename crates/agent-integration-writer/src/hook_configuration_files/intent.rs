//! The value a splice must produce, derived independently of the splice. The strict parser reads
//! the original document, the change applies to that value, and the result must equal what the
//! parser reads from the edited bytes. A splice that drops, alters or keeps a value against its
//! change therefore never reaches publication.
use std::fmt;

use serde::de::{Deserialize, Deserializer, Error as _, MapAccess, Visitor};
use serde_json::{Value, value::RawValue};

use super::splice::Change;

/// Whether `produced` holds exactly the value that `change` makes of `original`.
pub(super) fn agrees(original: &str, produced: &str, change: &Change) -> bool {
    let expected = Node::read(original).and_then(|node| expect(node, change, true));
    let produced = Node::read(produced);
    produced.is_some() && expected == Some(produced)
}

/// A JSON value as the strict parser reads it. Members compare without regard to order,
/// strings by their decoded text, and numbers by value where serde_json can hold them and by
/// spelling otherwise.
#[derive(Debug, PartialEq)]
enum Node {
    Object(Vec<(String, Node)>),
    Array(Vec<Node>),
    Text(String),
    Scalar(String),
}

impl Node {
    fn read(text: &str) -> Option<Self> {
        Self::of(&serde_json::from_str::<Box<RawValue>>(text).ok()?)
    }

    fn of(raw: &RawValue) -> Option<Self> {
        let text = raw.get().trim_start();
        Some(match text.as_bytes().first()? {
            b'{' => Self::Object(serde_json::from_str::<Members>(text).ok()?.0),
            b'[' => Self::Array(
                serde_json::from_str::<Vec<Box<RawValue>>>(text)
                    .ok()?
                    .iter()
                    .map(|element| Self::of(element))
                    .collect::<Option<_>>()?,
            ),
            b'"' => Self::Text(serde_json::from_str(text).ok()?),
            _ => Self::Scalar(
                serde_json::from_str::<Value>(text)
                    .map_or_else(|_| text.to_owned(), |value| value.to_string()),
            ),
        })
    }

    fn value(value: &Value) -> Option<Self> {
        Self::read(&serde_json::to_string(value).ok()?)
    }
}

/// Object members in key order; duplicates stay, so a duplicated member is a difference.
struct Members(Vec<(String, Node)>);

impl<'de> Deserialize<'de> for Members {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MembersVisitor;
        impl<'de> Visitor<'de> for MembersVisitor {
            type Value = Members;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<Members, M::Error> {
                let mut members = Vec::new();
                while let Some((key, raw)) = map.next_entry::<String, Box<RawValue>>()? {
                    let node =
                        Node::of(&raw).ok_or_else(|| M::Error::custom("unreadable value"))?;
                    members.push((key, node));
                }
                members.sort_by(|left, right| left.0.cmp(&right.0));
                Ok(Members(members))
            }
        }
        deserializer.deserialize_map(MembersVisitor)
    }
}

/// `node` as `change` leaves it, or `Some(None)` when the change removes it; `None` when the
/// change does not fit the node. A container below the root that loses every child to removal
/// goes too.
fn expect(node: Node, change: &Change, root: bool) -> Option<Option<Node>> {
    let (node, removed) = match (change, node) {
        (Change::Remove, _) => return Some(None),
        (Change::Replace(value), _) => return Node::value(value).map(Some),
        (Change::Object { members, append }, Node::Object(present)) => {
            let changes: Vec<_> = members
                .iter()
                .map(|(key, change)| ((*key).to_owned(), change))
                .collect();
            let (mut kept, removed) = children(present, &changes)?;
            for (key, value) in append {
                kept.push(((*key).to_owned(), Node::value(value)?));
            }
            kept.sort_by(|left, right| left.0.cmp(&right.0));
            (Node::Object(kept), removed)
        }
        (Change::Array { elements, append }, Node::Array(present)) => {
            let changes: Vec<_> = elements.iter().map(|(at, change)| (*at, change)).collect();
            let present = present.into_iter().enumerate().collect();
            let (kept, removed) = children(present, &changes)?;
            let mut kept: Vec<_> = kept.into_iter().map(|(_, node)| node).collect();
            for value in append {
                kept.push(Node::value(value)?);
            }
            (Node::Array(kept), removed)
        }
        _ => return None,
    };
    let empty = match &node {
        Node::Object(members) => members.is_empty(),
        Node::Array(elements) => elements.is_empty(),
        _ => false,
    };
    Some((!(empty && removed && !root)).then_some(node))
}

/// `present` as `changes` leave it, and whether a change removed a child. A change that names an
/// absent child does not fit, unless it removes it.
fn children<K: PartialEq>(
    present: Vec<(K, Node)>,
    changes: &[(K, &Change)],
) -> Option<(Vec<(K, Node)>, bool)> {
    let fits = changes.iter().all(|(wanted, change)| {
        matches!(change, Change::Remove) || present.iter().any(|(key, _)| key == wanted)
    });
    if !fits {
        return None;
    }
    let mut kept = Vec::with_capacity(present.len());
    let mut removed = false;
    for (key, node) in present {
        let next = match changes.iter().find(|(wanted, _)| *wanted == key) {
            None => Some(node),
            Some((_, change)) => expect(node, change, false)?,
        };
        match next {
            Some(node) => kept.push((key, node)),
            None => removed = true,
        }
    }
    Some((kept, removed))
}
