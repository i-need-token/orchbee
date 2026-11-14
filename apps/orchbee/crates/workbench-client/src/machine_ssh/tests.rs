use super::*;
#[test]
fn only_an_openssh_release_line_of_8_5_or_later_lets_answers_be_remembered() {
    // The text `ssh -V` printed, the release it names, and whether answers may be remembered.
    type Case = (&'static str, Option<(u32, u32)>, bool);
    let cases: [Case; 14] = [
        (
            "OpenSSH_for_Windows_9.5p1, LibreSSL 3.8.2\r\n",
            Some((9, 5)),
            true,
        ),
        (
            "OpenSSH_for_Windows_8.1p1, LibreSSL 3.0.2\r\n",
            Some((8, 1)),
            false,
        ),
        (
            "OpenSSH_8.4p1 Debian-5+deb11u3, OpenSSL 1.1.1w  11 Sep 2023\n",
            Some((8, 4)),
            false,
        ),
        (
            "OpenSSH_8.5p1, OpenSSL 1.1.1k  25 Mar 2021\n",
            Some((8, 5)),
            true,
        ),
        ("OpenSSH_10.0p2, LibreSSL 3.3.6\n", Some((10, 0)), true),
        ("  OpenSSH_9.6\nsecond line\n", Some((9, 6)), true),
        ("", None, false),
        ("ssh: unknown option -- V\r\n", None, false),
        ("OpenSSH_\n", None, false),
        ("OpenSSH_x.y\n", None, false),
        ("OpenSSH_9\n", None, false),
        ("OpenSSH_9.\n", None, false),
        ("OpenSSH_99999999999.5\n", None, false),
        ("PuTTY Release 0.81\nOpenSSH_9.5p1\n", None, false),
    ];
    for (text, expected, remembers) in cases {
        let version = Version::parse(text);
        assert_eq!(
            version.map(|version| (version.major, version.minor)),
            expected,
            "{text:?}"
        );
        assert_eq!(
            version.is_some_and(Version::remembers),
            remembers,
            "{text:?}"
        );
    }
}
