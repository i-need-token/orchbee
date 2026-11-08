use super::*;

// The inherited-route rule otherwise has evidence only from a cross-environment real-window run.
#[test]
fn only_a_foreign_environment_tag_hides_the_inherited_route() {
    let inherited = |tag: Option<&'static str>| {
        move |name: &str| match name {
            ENVIRONMENT_VARIABLE => tag.map(OsString::from),
            "ORCHBEE_RUNTIME_BASE" | "ORCHBEE_RUNTIME_SCOPE" | "ORCHBEE_SESSION" => {
                Some(OsString::from(name))
            }
            _ => None,
        }
    };
    let own = environment_stem();
    let foreign = if own == "orchbee" {
        "ob-dev-0123456789abcdef"
    } else {
        "orchbee"
    };
    for (tag, follows) in [(None, true), (Some(own), true), (Some(foreign), false)] {
        let routed = session_route_environment(inherited(tag));
        for name in ROUTE_VARIABLES {
            assert_eq!(routed(name).is_some(), follows, "{tag:?} {name}");
        }
        assert_eq!(
            routed("ORCHBEE_SESSION"),
            Some(OsString::from("ORCHBEE_SESSION"))
        );
    }
}
