use super::Binary64SafeInteger;

#[test]
fn accepts_equivalent_binary64_integer_spellings() -> Result<(), serde_json::Error> {
    for encoded in ["1", "1.0", "1e0", "8e1", "1.0000000000000001", "-0"] {
        let decoded = serde_json::from_str::<Binary64SafeInteger>(encoded)?;
        let expected = if encoded == "8e1" {
            80
        } else if encoded == "-0" {
            0
        } else {
            1
        };
        assert_eq!(decoded.value(), expected, "numeric spelling {encoded}");
    }
    Ok(())
}

#[test]
fn rejects_fractional_negative_unsafe_overflow_and_nonfinite_numbers() {
    for encoded in ["1.5", "-1", "9007199254740992", "1e4000", "NaN", "Infinity"] {
        assert!(
            serde_json::from_str::<Binary64SafeInteger>(encoded).is_err(),
            "numeric spelling {encoded}"
        );
    }
}

#[test]
fn rejects_values_outside_the_unsigned_sixteen_bit_refinement() -> Result<(), serde_json::Error> {
    let decoded = serde_json::from_str::<Binary64SafeInteger>("65536")?;
    assert!(decoded.try_into_u16().is_err());
    Ok(())
}
