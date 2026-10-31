//! Target operand syntax: typed identities, compact selectors, and display labels, classified
//! before any live resolution.

use super::{FailureEnvelope, ParseFailure, ParseFailureKind, Resource, Target};

const MAXIMUM_LABEL_SCALARS: usize = 512;

pub(super) const ZERO_IDENTITY: &str = "00000000000000000000000000000000";

fn lowercase_hex(bytes: &[u8]) -> bool {
    bytes
        .iter()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

pub(super) fn valid_typed_id(value: &str) -> bool {
    value.len() == 32 && lowercase_hex(value.as_bytes())
}

/// Display labels remain literal, but these spellings claim target syntax.
pub(crate) fn label_requires_id(value: &str) -> bool {
    valid_typed_id(value)
        || value
            .strip_prefix('@')
            .and_then(|rest| rest.split_once(':'))
            .is_some_and(|(kind, _)| Resource::selector_token(kind).is_some())
}

/// Compact identities have a distinct `0x` marker and all significant digits.
fn valid_selector_identity(payload: &str) -> bool {
    payload.strip_prefix("0x").is_some_and(|hex| {
        (1..=32).contains(&hex.len()) && !hex.starts_with('0') && lowercase_hex(hex.as_bytes())
    })
}

fn valid_label(value: &str) -> bool {
    let scalars = value.chars().count();
    (1..=MAXIMUM_LABEL_SCALARS).contains(&scalars)
}

pub(super) fn target(
    value: String,
    resource: Resource,
    envelope: FailureEnvelope,
) -> Result<Target, ParseFailure> {
    let identifier = |target: String| ParseFailure {
        kind: ParseFailureKind::InvalidIdentifier { resource, target },
        envelope,
        usage: None,
    };
    // A recognized `@<kind>:` prefix claims selector syntax wherever it
    // appears: the kind must equal the operand's resource and the payload must
    // be an exact compact identity. Both failures are parse-time `invalid-identifier`.
    // An `@` token without a recognized prefix falls through to the
    // label rule unchanged.
    let selector = value
        .strip_prefix('@')
        .and_then(|rest| rest.split_once(':'))
        .and_then(|(kind, payload)| {
            Resource::selector_token(kind).map(|kind| (kind, valid_selector_identity(payload)))
        });
    if let Some((kind, admitted)) = selector {
        if !admitted || kind != resource {
            return Err(identifier(value));
        }
        return Ok(Target::Selector(value));
    }
    if valid_typed_id(&value) {
        if value == ZERO_IDENTITY {
            return Err(identifier(value));
        }
        return Ok(Target::Id(value));
    }
    match resource {
        Resource::Panel | Resource::Terminal => Err(identifier(value)),
        _ if valid_label(&value) => Ok(Target::Label(value)),
        _ => Err(identifier(value)),
    }
}
