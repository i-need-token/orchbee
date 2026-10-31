use super::*;
use crate::cli_text::CliText;
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
        document["failure"]["details"]["reason"] = "unknown".into();
        assert!(!validator.is_valid(&document));
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let diagnostic = CliText::new(locale).cli_failure(&failure);
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

#[test]
fn a_client_refused_plugin_request_publishes_the_clients_typed_reason() {
    use orchbee_workbench_protocol::ClientViewControlReason;
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../schemas/orchbee.cli.failure.json")).unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    for (reason, token) in [
        (ClientViewControlReason::Busy, "busy"),
        (ClientViewControlReason::ModalOpen, "modal-open"),
        (
            ClientViewControlReason::InsufficientSpace,
            "insufficient-space",
        ),
        (ClientViewControlReason::StaleView, "stale-view"),
    ] {
        let mut refused = Failure::new(FailureKind::Rejected);
        refused.client_refusal = Some(reason);
        let failure = client_refusal(refused);
        let mut document = failure.document();
        validator.validate(&document).unwrap();
        let details = &document["failure"]["details"];
        assert_eq!(
            (
                &details["stage"],
                &details["reason"],
                &details["clientReason"]
            ),
            (&"plugin".into(), &"client-refused".into(), &token.into())
        );
        let english = CliText::new(Locale::EnUs).cli_failure(&failure);
        let chinese = CliText::new(Locale::ZhCn).cli_failure(&failure);
        assert!(english.contains("refused the plugin request"), "{english}");
        assert!(chinese.contains("拒绝了插件请求"), "{chinese}");
        assert!(!english.contains("Unavailable") && !chinese.contains("不可用"));
        document["failure"]["details"]
            .as_object_mut()
            .unwrap()
            .remove("clientReason");
        assert!(!validator.is_valid(&document));
    }
    // Failures that already own a contract, or carry no Client reason, are unchanged.
    for failure in [
        Reason::ClientChanged.failure(),
        Failure::new(FailureKind::Rejected),
    ] {
        let before = failure.document();
        assert_eq!(client_refusal(failure).document(), before);
    }
}
