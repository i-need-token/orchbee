use orchbee_terminal_protocol::{TerminalRowPromptClass, TerminalTextWindowRow};

use super::{Error, MAXIMUM_ROWS, valid_row_range, validate_join};

#[test]
fn row_bound_is_per_call_and_preserves_the_u32_point_domain() {
    let maximum = MAXIMUM_ROWS as u64;
    assert!(valid_row_range(maximum * 2, &(0..maximum)));
    assert!(valid_row_range(maximum * 2, &(maximum..maximum * 2)));
    assert!(!valid_row_range(maximum + 1, &(0..maximum + 1)));
    assert!(!valid_row_range(maximum, &(0..0)));
    assert!(!valid_row_range(maximum, &(maximum..maximum + 1)));
    let beyond_point_domain = u64::from(u32::MAX) + 2;
    assert!(!valid_row_range(
        beyond_point_domain,
        &(beyond_point_domain - 1..beyond_point_domain),
    ));
}

#[test]
fn independently_read_chunks_require_an_exact_wrap_join() {
    let prior = row(2, true, false);
    let matching = row(3, false, true);
    assert!(validate_join::<()>(std::slice::from_ref(&prior), &[matching]).is_ok());
    let mismatched = row(3, false, false);
    assert!(matches!(
        validate_join::<()>(&[prior], &[mismatched]),
        Err(Error::ProtocolInvalid)
    ));
}

fn row(index: u64, wrap: bool, wrap_continuation: bool) -> TerminalTextWindowRow {
    TerminalTextWindowRow {
        index,
        prompt_class: TerminalRowPromptClass::None,
        wrap,
        wrap_continuation,
        identity_ascii: true,
        text: "x".into(),
        mappings: Vec::new(),
    }
}
