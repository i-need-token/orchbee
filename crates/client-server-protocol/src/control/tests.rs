use super::{ControlInput, TerminalControlAction};
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
        controller_id,
        sequence: NonZeroU64::new(1).expect("non-zero"),
        geometry: Geometry::new(80, 24, 8, 16)?,
    };

    assert_eq!(acquire.attachment_id(), &attachment_id);
    assert_eq!(input.attachment_id(), &attachment_id);
    assert_eq!(resize.attachment_id(), &attachment_id);
    Ok(())
}
