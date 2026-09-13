use super::{
    ControlInput, PasteAcceptedBytes, PasteContent, TerminalControlAction, TerminalControlOutcome,
};
use crate::{ControllerId, Geometry, ViewerId};
use std::num::NonZeroU64;

#[test]
fn attachment_id_accessor_selects_the_target_viewer() -> Result<(), Box<dyn std::error::Error>> {
    let attachment_id = ViewerId::parse("33333333333333333333333333333333")?;
    let controller_id = ControllerId::parse("22222222222222222222222222222222")?;
    let acquire = TerminalControlAction::Acquire {
        attachment_id: attachment_id.clone(),
    };
    let input = TerminalControlAction::Input {
        attachment_id: attachment_id.clone(),
        controller_id: controller_id.clone(),
        sequence: NonZeroU64::new(1).expect("non-zero"),
        bytes: ControlInput::new(vec![0xab])?,
    };
    let resize = TerminalControlAction::Resize {
        attachment_id: attachment_id.clone(),
        controller_id: controller_id.clone(),
        sequence: NonZeroU64::new(1).expect("non-zero"),
        geometry: Geometry::new(80, 24, 8, 16)?,
    };
    let paste = TerminalControlAction::Paste {
        attachment_id: attachment_id.clone(),
        controller_id,
        sequence: NonZeroU64::new(1).expect("non-zero"),
        content: PasteContent::new(vec![0x00, 0x02, 0xff])?,
    };

    assert_eq!(acquire.attachment_id(), &attachment_id);
    assert_eq!(input.attachment_id(), &attachment_id);
    assert_eq!(resize.attachment_id(), &attachment_id);
    assert_eq!(paste.attachment_id(), &attachment_id);
    Ok(())
}

#[test]
fn json_codec_rejects_the_paste_action_in_both_directions() -> Result<(), Box<dyn std::error::Error>>
{
    let attachment_id = ViewerId::parse("33333333333333333333333333333333")?;
    let paste = TerminalControlAction::Paste {
        attachment_id,
        controller_id: ControllerId::parse("22222222222222222222222222222222")?,
        sequence: NonZeroU64::new(1).expect("non-zero"),
        content: PasteContent::new(b"echo".to_vec())?,
    };
    assert!(serde_json::to_string(&paste).is_err());

    let json = "{\"kind\":\"paste\",\"attachmentId\":\"33333333333333333333333333333333\",\"controllerId\":\"22222222222222222222222222222222\",\"sequence\":\"3\",\"content\":\"00ff\"}";
    assert!(serde_json::from_str::<TerminalControlAction>(json).is_err());
    Ok(())
}

#[test]
fn pasted_outcome_keeps_bounded_positive_accepted_bytes() -> Result<(), Box<dyn std::error::Error>>
{
    let outcome = TerminalControlOutcome::Pasted {
        accepted_bytes: PasteAcceptedBytes::new(262_157)?,
    };
    let encoded = serde_json::to_string(&outcome)?;
    assert_eq!(encoded, "{\"kind\":\"pasted\",\"acceptedBytes\":262157}");
    assert_eq!(
        serde_json::from_str::<TerminalControlOutcome>(&encoded)?,
        outcome
    );

    for payload in [
        "{\"kind\":\"pasted\",\"acceptedBytes\":0}",
        "{\"kind\":\"pasted\",\"acceptedBytes\":1048577}",
        "{\"kind\":\"pasted\",\"acceptedBytes\":12.5}",
        "{\"kind\":\"pasted\",\"acceptedBytes\":-1}",
        "{\"kind\":\"pasted\",\"acceptedBytes\":\"5\"}",
    ] {
        assert!(
            serde_json::from_str::<TerminalControlOutcome>(payload).is_err(),
            "accepted {payload}"
        );
    }
    Ok(())
}
