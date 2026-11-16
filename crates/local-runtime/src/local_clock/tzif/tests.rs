//! Expected offsets come from published zone rules; the instants are Unix seconds computed outside
//! this crate.
use super::Zone;

/// A TZif header per RFC 8536 with no leap-second or indicator records.
fn header(version: u8, transitions: usize, types: usize) -> Vec<u8> {
    let mut bytes = b"TZif".to_vec();
    bytes.push(version);
    bytes.extend([0; 15]);
    for count in [0, 0, 0, transitions, types, 1] {
        bytes.extend(u32::try_from(count).unwrap().to_be_bytes());
    }
    bytes
}

/// Type records with an empty designation, then that one NUL designation byte.
fn types(bytes: &mut Vec<u8>, offsets: &[i32]) {
    for offset in offsets {
        bytes.extend(offset.to_be_bytes());
        bytes.extend([0, 0]);
    }
    bytes.push(0);
}

/// A version 2 file: a minimal version 1 block, the 64-bit block, then `footer`.
fn version_2(transitions: &[(i64, u8)], offsets: &[i32], footer: &str) -> Vec<u8> {
    let mut bytes = header(b'2', 0, 1);
    types(&mut bytes, &[0]);
    bytes.extend(header(b'2', transitions.len(), offsets.len()));
    for (instant, _) in transitions {
        bytes.extend(instant.to_be_bytes());
    }
    bytes.extend(transitions.iter().map(|&(_, index)| index));
    types(&mut bytes, offsets);
    bytes.extend(format!("\n{footer}\n").bytes());
    bytes
}

fn offsets(zone: &Zone, instants: &[i64]) -> Vec<Option<i32>> {
    instants
        .iter()
        .map(|&instant| zone.offset_at(instant))
        .collect()
}

#[test]
fn transitions_choose_the_offset_and_type_zero_precedes_them() {
    let zone = Zone::parse(&version_2(
        &[(1_000, 1), (2_000, 2), (3_000, 0)],
        &[3_600, 7_200, -1_800],
        "",
    ))
    .unwrap();
    assert_eq!(
        offsets(
            &zone,
            &[999, 1_000, 1_999, 2_000, 2_999, 3_000, 1_768_478_400]
        ),
        [3_600, 7_200, 7_200, -1_800, -1_800, 3_600, 3_600].map(Some)
    );
}

#[test]
fn a_version_1_file_keeps_its_last_offset() {
    let mut bytes = header(0, 1, 2);
    bytes.extend(1_000_i32.to_be_bytes());
    bytes.push(1);
    types(&mut bytes, &[0, 3_600]);
    let zone = Zone::parse(&bytes).unwrap();
    assert_eq!(
        offsets(&zone, &[999, 1_000, 1_768_478_400]),
        [0, 3_600, 3_600].map(Some)
    );
}

#[test]
fn the_footer_rule_governs_past_the_last_transition() {
    // America/New_York: 2026 daylight time runs from 8 March 07:00 UTC to 1 November 06:00 UTC.
    let zone = Zone::parse(&version_2(
        &[(631_152_000, 1)],
        &[0, -18_000],
        "EST5EDT,M3.2.0,M11.1.0",
    ))
    .unwrap();
    assert_eq!(
        offsets(
            &zone,
            &[
                631_151_999,
                1_768_478_400,
                1_772_953_199,
                1_772_953_200,
                1_793_512_799,
                1_793_512_800,
            ]
        ),
        [0, -18_000, -18_000, -14_400, -14_400, -18_000].map(Some)
    );
}

#[test]
fn southern_daylight_time_spans_the_new_year() {
    // Australia/Sydney: 2026 daylight time ends 4 April 16:00 UTC and starts 3 October 16:00 UTC.
    let zone = Zone::parse(&version_2(&[], &[36_000], "AEST-10AEDT,M10.1.0,M4.1.0/3")).unwrap();
    assert_eq!(
        offsets(
            &zone,
            &[
                1_768_478_400,
                1_775_318_399,
                1_775_318_400,
                1_782_864_000,
                1_791_043_199,
                1_791_043_200,
                1_798_759_800,
            ]
        ),
        [39_600, 39_600, 36_000, 36_000, 36_000, 39_600, 39_600].map(Some)
    );
}

#[test]
fn quoted_names_signed_minutes_and_negative_rule_times() {
    // America/Nuuk follows the European change at 01:00 UTC: 29 March and 25 October in 2026.
    let nuuk = Zone::parse(&version_2(
        &[],
        &[-7_200],
        "<-02>2<-01>,M3.5.0/-1,M10.5.0/0",
    ))
    .unwrap();
    assert_eq!(
        offsets(
            &nuuk,
            &[1_774_745_999, 1_774_746_000, 1_792_889_999, 1_792_890_000]
        ),
        [-7_200, -3_600, -3_600, -7_200].map(Some)
    );
    let kolkata = Zone::parse(&version_2(&[], &[19_800], "IST-5:30")).unwrap();
    assert_eq!(kolkata.offset_at(1_709_208_000), Some(19_800));
}

#[test]
fn malformed_files_are_refused() {
    let valid = version_2(&[(1_000, 0)], &[0], "UTC0");
    assert!(Zone::parse(&valid).is_some());
    assert!(Zone::parse(&valid[..valid.len() - 1]).is_none());
    let mut magic = valid.clone();
    magic[0] = b'X';
    assert!(Zone::parse(&magic).is_none());
    // Daylight time without its rule, an unknown type and descending transitions.
    assert!(Zone::parse(&version_2(&[], &[0], "EST5EDT")).is_none());
    assert!(Zone::parse(&version_2(&[(1_000, 1)], &[0], "")).is_none());
    assert!(Zone::parse(&version_2(&[(2_000, 0), (1_000, 0)], &[0], "")).is_none());
}
