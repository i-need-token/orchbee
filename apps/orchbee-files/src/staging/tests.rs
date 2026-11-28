//! Staging names: recognized exactly, and within a name's bound.

use super::*;

#[test]
fn a_staging_name_is_recognized_and_stays_within_a_name_s_bound() {
    let staged = name("report.pdf").unwrap();
    assert!(staged.starts_with(".report.pdf.orchbee-") && is_staging(&staged));
    let staged = name(&"é".repeat(200)).unwrap();
    assert!(staged.len() <= 255 && is_staging(&staged));
    for other in [
        "report.pdf",
        ".report.pdf.orchbee-",
        ".report.pdf.orchbee-0123456789ABCDEF",
        "report.pdf.orchbee-0123456789abcdef",
        ".orchbee-0123456789abcdef",
    ] {
        assert!(!is_staging(other), "{other}");
    }
}
