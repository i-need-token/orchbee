//! Spelling advice and command discovery stay inside the admitted grammar scopes.

use super::suggestions;

#[test]
fn unknown_option_advice_uses_root_family_and_leaf_grammar_scopes() {
    use crate::invocation::{UsageContext, UsageReason};

    let advice = |context, option: &str| {
        suggestions(&UsageReason::UnknownOption {
            context,
            option: option.to_owned(),
        })
    };

    assert!(
        advice(UsageContext::Root, "--hlep")
            .iter()
            .any(|candidate| candidate == "--help")
    );
    #[cfg(windows)]
    {
        assert!(
            advice(UsageContext::Root, "--sesion")
                .iter()
                .any(|candidate| candidate == "--session")
        );
        assert!(
            advice(UsageContext::Root, "--machne")
                .iter()
                .any(|candidate| candidate == "--machine")
        );
    }
    #[cfg(not(windows))]
    {
        assert!(
            !advice(UsageContext::Root, "--sesion")
                .iter()
                .any(|candidate| candidate == "--session")
        );
        assert!(
            !advice(UsageContext::Root, "--machne")
                .iter()
                .any(|candidate| candidate == "--machine")
        );
    }
    assert!(
        advice(UsageContext::Path("agent"), "--prvider")
            .iter()
            .any(|candidate| candidate == "--provider")
    );
    assert!(
        advice(UsageContext::Path("agent integration"), "--adapter-souce")
            .iter()
            .any(|candidate| candidate == "--adapter-source")
    );
    assert!(
        advice(UsageContext::Path("terminal list"), "--formt")
            .iter()
            .any(|candidate| candidate == "--format")
    );
    assert!(
        advice(UsageContext::Path("terminal list"), "--formt=json")
            .iter()
            .any(|candidate| candidate == "--format")
    );
    for option in ["-x", "-x=value"] {
        assert!(advice(UsageContext::Root, option).is_empty(), "{option}");
        assert!(
            advice(UsageContext::Path("agent"), option).is_empty(),
            "{option}"
        );
    }

    assert!(advice(UsageContext::Root, &format!("--{}", "x".repeat(65))).is_empty());
    assert!(advice(UsageContext::Root, "--hélp").is_empty());
    assert!(advice(UsageContext::Root, "--for").len() <= 3);
}

#[test]
fn usage_advice_offers_synonyms_and_the_label_form() {
    use crate::invocation::{UsageContext, UsageReason};
    let unknown = |context, word: &str| {
        suggestions(&UsageReason::UnknownCommand {
            context,
            word: word.to_owned(),
        })
    };
    assert_eq!(
        unknown(UsageContext::Path("workspace"), "delete")[0],
        "close"
    );
    assert_eq!(unknown(UsageContext::Path("tab"), "rm")[0], "close");
    let stray = |context, value: &str| {
        suggestions(&UsageReason::UnexpectedArgument {
            context,
            value: value.to_owned(),
        })
    };
    assert_eq!(
        stray(UsageContext::Path("tab rename"), "main"),
        ["--label main"]
    );
    assert_eq!(
        stray(UsageContext::Path("project create"), "demo"),
        ["--label demo"]
    );
    assert!(stray(UsageContext::Path("terminal list"), "x").is_empty());
}
