//! `terminal send` input: literal text, semantic key strokes, and declared file or stdin sources,
//! checked against the send bounds before any runtime effect.

use super::{
    ExpectedValue, FailureEnvelope, MAXIMUM_SEND_RECORDS, MAXIMUM_SEND_TEXT_BYTES, Options,
    ParseFailure, TerminalSendInput, UsageContext, UsageReason, missing_required, send_key,
    usage_with,
};

/// Validate one semantic press before any runtime effects.
pub(super) fn parse_send_key(value: &str) -> Option<orchbee_client_server_protocol::SendKeyStroke> {
    send_key::parse(value)
}

/// `terminal send` admits exactly one source family: `--text`, one or more
/// repeatable `--key` values, `--file`, or `--stdin`. `--enter` is the
/// text-bearing modifier — legal under `--text`, `--file`, and `--stdin`,
/// never under `--key`. `--paste` selects the Host-bracketed paste record
/// and is legal only beside a declared `--file`/`--stdin` source. The text
/// byte budget counts the payload the wire actually carries — the literal
/// bytes plus the appended CR — so an over-limit record is refused before
/// any connection. `--file` and `--stdin` are declarations only: the parser
/// stores the source untouched and the materializer reads it later under
/// the shared invocation deadline.
pub(super) fn send_input(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<TerminalSendInput, ParseFailure> {
    // Exactly one source family: a second declaration is the conflict the
    // grammar rejects, named in a fixed order regardless of argv order.
    let declared: Vec<&'static str> = [
        ("--text", options.text.is_some()),
        ("--key", !options.keys.is_empty()),
        ("--file", options.file.is_some()),
        ("--stdin", options.stdin),
    ]
    .into_iter()
    .filter_map(|(flag, seen)| seen.then_some(flag))
    .collect();
    if declared.len() > 1 {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: declared[0],
                second: declared[1],
            },
            envelope,
        ));
    }
    if let Some(text) = &options.text {
        if options.paste {
            return Err(usage_with(
                UsageReason::UnexpectedOption {
                    context,
                    option: "--paste",
                },
                envelope,
            ));
        }
        let bytes = text.len() + usize::from(options.enter);
        if text.is_empty() || bytes > MAXIMUM_SEND_TEXT_BYTES {
            return Err(usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--text",
                    value: text.clone(),
                    expected: ExpectedValue::SendText,
                },
                envelope,
            ));
        }
        return Ok(TerminalSendInput::Text {
            text: text.clone(),
            enter: options.enter,
        });
    }
    if !options.keys.is_empty() {
        if options.enter {
            return Err(usage_with(
                UsageReason::UnexpectedOption {
                    context,
                    option: "--enter",
                },
                envelope,
            ));
        }
        if options.paste {
            return Err(usage_with(
                UsageReason::UnexpectedOption {
                    context,
                    option: "--paste",
                },
                envelope,
            ));
        }
        if options.keys.len() > MAXIMUM_SEND_RECORDS {
            return Err(usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: "--key",
                    value: options.keys[MAXIMUM_SEND_RECORDS].clone(),
                    expected: ExpectedValue::SendKeySequence,
                },
                envelope,
            ));
        }
        // Whole-sequence prevalidation: every value parses before the
        // request exists, so a bad later key never reaches the wire after
        // earlier keys.
        let mut keys = Vec::with_capacity(options.keys.len());
        for value in &options.keys {
            let Some(key) = parse_send_key(value) else {
                return Err(usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "--key",
                        value: value.clone(),
                        expected: ExpectedValue::SendKey,
                    },
                    envelope,
                ));
            };
            keys.push(key);
        }
        return Ok(TerminalSendInput::Keys(keys));
    }
    if options.file.is_some() || options.stdin {
        return Ok(if let Some(path) = &options.file {
            TerminalSendInput::File {
                path: path.clone(),
                enter: options.enter,
                paste: options.paste,
            }
        } else {
            TerminalSendInput::Stdin {
                enter: options.enter,
                paste: options.paste,
            }
        });
    }
    if options.paste {
        return Err(usage_with(
            UsageReason::UnexpectedOption {
                context,
                option: "--paste",
            },
            envelope,
        ));
    }
    if options.enter {
        return Err(usage_with(
            UsageReason::UnexpectedOption {
                context,
                option: "--enter",
            },
            envelope,
        ));
    }
    Err(missing_required(
        context,
        "'--text', '--key', '--file', or '--stdin'",
        envelope,
    ))
}
