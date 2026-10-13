use super::*;
use crate::localization::Locale;

#[test]
fn preparation_failures_are_typed_localized_and_closed_to_the_new_version() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../schemas/orchbee.cli.failure.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for reason in [
        Reason::SelectorUnavailable,
        Reason::RegistryUnavailable,
        Reason::PluginNotFound,
        Reason::PluginDisabled,
        Reason::PaneNotFound,
        Reason::UnsupportedPlatform,
        Reason::InvalidTitle,
        Reason::InvalidGeometry,
        Reason::PopupFocusRequired,
        Reason::PopupTargetUnsupported,
        Reason::ClientListingUnavailable,
        Reason::ClientNotReady,
        Reason::BindingUnavailable,
        Reason::InvalidPopupSpec,
        Reason::ClientChanged,
        Reason::ContextInvalid,
        Reason::TransportLoss,
        Reason::InvalidReply,
        Reason::PaneChanged,
        Reason::PluginUnavailable,
    ] {
        let failure = reason.failure();
        let mut document = failure.document();
        validator.validate(&document).unwrap();
        for version in [1, 2] {
            document["version"] = version.into();
            assert!(!validator.is_valid(&document));
        }
        document["version"] = 3.into();
        document["failure"]["details"]["reason"] = "unknown".into();
        assert!(!validator.is_valid(&document));
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let diagnostic = locale.cli_failure(&failure);
            assert!(
                !diagnostic.contains("()")
                    && !diagnostic.contains("（）")
                    && !diagnostic.contains("\"\""),
                "{diagnostic}"
            );
            if matches!(reason, Reason::PopupFocusRequired) {
                assert!(diagnostic.contains("--no-focus"), "{diagnostic}");
            }
        }
    }
}
