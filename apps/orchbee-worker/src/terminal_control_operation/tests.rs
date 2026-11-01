use super::{AttachmentId, InputMessage, NonZeroU64, controlled_semantic_input};
use orchbee_capability_node_protocol::{SemanticInput, SemanticInputKind};
use orchbee_terminal_protocol::{FocusInput, TextInput, encode_input_message};

#[test]
fn semantic_input_takes_its_authority_from_the_grant_not_the_client()
-> Result<(), Box<dyn std::error::Error>> {
    let grant = AttachmentId::from_bytes([1; 16])?;
    let revision = NonZeroU64::new(7).ok_or("zero revision")?;
    let forged = encode_input_message(&InputMessage::TextInput(TextInput {
        attachment_id: AttachmentId::from_bytes([9; 16])?,
        capability_revision: NonZeroU64::new(999).ok_or("zero revision")?,
        text: "typed".to_owned(),
    }))?;
    assert_eq!(
        controlled_semantic_input(
            &SemanticInput::new(SemanticInputKind::Text, forged.payload.clone())?,
            grant,
            revision,
        ),
        Some(InputMessage::TextInput(TextInput {
            attachment_id: grant,
            capability_revision: revision,
            text: "typed".to_owned(),
        })),
    );
    // A payload the declared kind's codec refuses never becomes a Host submission.
    assert_eq!(
        controlled_semantic_input(
            &SemanticInput::new(SemanticInputKind::Focus, forged.payload)?,
            grant,
            revision,
        ),
        None,
    );
    // Rebinding keeps what the Client meant, for the fixed-size record too.
    let focus = encode_input_message(&InputMessage::FocusInput(FocusInput {
        attachment_id: AttachmentId::from_bytes([9; 16])?,
        capability_revision: revision,
        focused: true,
    }))?;
    assert_eq!(
        controlled_semantic_input(
            &SemanticInput::new(SemanticInputKind::Focus, focus.payload)?,
            grant,
            revision,
        ),
        Some(InputMessage::FocusInput(FocusInput {
            attachment_id: grant,
            capability_revision: revision,
            focused: true,
        })),
    );
    Ok(())
}
