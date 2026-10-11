use super::super::super::hook_configuration_windows;
use super::super::super::{model::ManagementMode, tests::windows_adapters::Fixture};
use super::*;
use crate::agent_configuration_windows::HookConfigurationKind;
use std::{
    cell::Cell,
    fs,
    time::{Duration, Instant},
};
struct Cancel(Cell<bool>);
impl crate::termination_signal::TerminationSource for Cancel {
    fn requested(&self) -> bool {
        self.0.get()
    }
}
#[test]
fn changed_preimages_arrivals_and_cancellation_preserve_user_data() {
    for scenario in [
        "snapshot",
        "pre-move",
        "moved",
        "published",
        "cancel",
        "metadata-cancel",
    ] {
        let fixture = Fixture::new();
        let target = fixture.0.join("hooks.json");
        let original = br#"{"user":"original","hooks":{}}"#;
        let foreign = br#"{"user":"newer","hooks":{}}"#;
        fs::write(&target, original).unwrap();
        let capsule = fixture.0.join("capsules/0.4.17");
        let cancel = Cancel(Cell::new(false));
        let context = ProviderContext {
            home: &fixture.0,
            capsule_root: &capsule,
            adapter_source: None,
            path_env: None,
            deadline: Instant::now() + Duration::from_secs(10),
            cancel: Some(&cancel),
            management_mode: ManagementMode::ManagedLocal,
        };
        let mut attempted = Vec::new();
        let mut residuals = Vec::new();
        let mut effect = false;
        let result = replace_observed(
            &target,
            Some(original.to_vec()),
            hook_configuration_windows::patch(
                HookConfigurationKind::Cursor,
                Some(original),
                &capsule,
                true,
            )
            .unwrap(),
            &context,
            EffectTrace {
                attempted: &mut attempted,
                residuals: &mut residuals,
                begun: &mut effect,
            },
            |boundary, path| {
                if boundary == "pre-move" {
                    // The retained original handle really denies in-place changes.
                    assert!(fs::write(path, foreign).is_err());
                }
                if scenario == "pre-move" && boundary == "pre-move" {
                    fs::rename(path, path.with_extension("user-saved")).unwrap();
                    fs::write(path, foreign).unwrap();
                } else if boundary == scenario {
                    fs::write(path, foreign).unwrap();
                } else if (scenario == "cancel" && boundary == "moved")
                    || (scenario == "metadata-cancel" && boundary == "metadata")
                {
                    cancel.0.set(true);
                }
            },
        );
        assert!(effect);
        if matches!(scenario, "cancel" | "metadata-cancel") {
            assert_eq!(result, Attempt::Cancelled);
            assert!(!target.exists());
            assert!(
                residuals
                    .iter()
                    .any(|p| fs::read(p).is_ok_and(|bytes| bytes == original))
            );
        } else {
            assert_ne!(result, Attempt::Attempted);
            assert_eq!(fs::read(&target).unwrap(), foreign);
            if matches!(scenario, "moved" | "published") {
                assert!(
                    residuals
                        .iter()
                        .any(|p| fs::read(p).is_ok_and(|bytes| bytes == original))
                );
            } else {
                assert!(residuals.is_empty());
            }
        }
    }
}
