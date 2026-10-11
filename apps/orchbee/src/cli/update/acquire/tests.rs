use super::*;
#[test]
fn curl_stream_limit_and_https_resolution_do_not_silently_degrade() {
    for value in [
        "curl 8.4.0 Windows",
        "curl 8.14.1 Windows",
        "curl 9.0.0 Windows",
    ] {
        assert!(bounded_curl(value.as_bytes()));
    }
    for value in [
        "curl 8.3.0 Windows",
        "curl 7.88.0 Windows",
        "curl unknown",
        "not curl 9.0.0",
    ] {
        assert!(!bounded_curl(value.as_bytes()));
    }
    assert!(Source::parse("https://example.test").is_err());
    assert!(Source::parse("http://example.test/release.json").is_err());
    let source = Source::parse("https://example.test/stable/release.json")
        .unwrap_or_else(|_| panic!("valid source"));
    assert!(
        matches!(source.artifact("orchbee.exe"), Ok(Source::Https(url)) if url == "https://example.test/stable/orchbee.exe")
    );
}
