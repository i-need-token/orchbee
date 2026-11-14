//! Host suggestions over real files in a scratch home per case, never the person's own `.ssh`:
//! OpenSSH's line grammar, alias filtering, include resolution and globs, and the bounds and loops
//! that leave no suggestions at all.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Files a case writes, by path below its scratch home.
type Files = Vec<(String, Vec<u8>)>;

struct Case {
    name: &'static str,
    files: fn(&Path) -> Files,
    expected: Vec<String>,
}

/// A scratch home of its own, removed when dropped, pass or panic.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let home = std::env::temp_dir().join(format!(
            "mm1-sshcfg-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&home).unwrap();
        Self(home)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _removed = fs::remove_dir_all(&self.0);
    }
}

fn file(path: &str, content: impl Into<Vec<u8>>) -> (String, Vec<u8>) {
    (path.to_owned(), content.into())
}

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| (*name).to_owned()).collect()
}

/// A configuration whose includes nest `links` deep, the deepest file naming `deep`.
fn chain(links: usize) -> Files {
    let mut files = vec![file(".ssh/config", "Include l1\n")];
    for link in 1..links {
        files.push(file(
            &format!(".ssh/l{link}"),
            format!("Include l{}\n", link + 1),
        ));
    }
    files.push(file(&format!(".ssh/l{links}"), "Host deep\n"));
    files
}

/// A file naming `host`, padded with a comment to exactly `bytes` bytes.
fn padded(host: &str, bytes: usize) -> Vec<u8> {
    let mut content = format!("Host {host}\n").into_bytes();
    content.resize(bytes, b'#');
    content
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "plain hosts in order",
            files: |_| {
                vec![file(
                    ".ssh/config",
                    "Host alpha\n  HostName 192.0.2.1\n  User someone\nHost beta\n\nHost gamma\n",
                )]
            },
            expected: names(&["alpha", "beta", "gamma"]),
        },
        Case {
            name: "patterns and negations skipped",
            files: |_| {
                vec![file(
                    ".ssh/config",
                    "Host *\nHost web-? db*\nHost !blocked good\n",
                )]
            },
            expected: names(&["good"]),
        },
        Case {
            name: "line grammar",
            files: |_| {
                vec![file(
                    ".ssh/config",
                    concat!(
                        "# Host commented\n",
                        "Host one two\n",
                        "Host=three\n",
                        "HOST = four\n",
                        "Host \"five\"\n",
                        "  hOsT\tsix # seven\r\n",
                        "Host \"broken\n",
                        "Host \"dev box\" eight\n",
                        "HostName nine\n",
                    ),
                )]
            },
            expected: names(&["one", "two", "three", "four", "five", "six", "eight"]),
        },
        Case {
            name: "duplicates once",
            files: |_| vec![file(".ssh/config", "Host dup\nHost other dup\nHost DUP\n")],
            expected: names(&["dup", "other"]),
        },
        Case {
            name: "invalid targets skipped",
            files: |_| {
                vec![file(
                    ".ssh/config",
                    "Host good -dash host:22 a,b user@ semi;colon ok.example\n",
                )]
            },
            expected: names(&["good", "ok.example"]),
        },
        Case {
            name: "at most 32",
            files: |_| {
                let hosts: Vec<String> = (0..40).map(|index| format!("h{index}")).collect();
                vec![file(".ssh/config", format!("Host {}\n", hosts.join(" ")))]
            },
            expected: (0..32).map(|index| format!("h{index}")).collect(),
        },
        Case {
            name: "non-UTF-8 line skipped",
            files: |_| {
                vec![file(
                    ".ssh/config",
                    b"Host \xff\xfe bad\nHost fine\n".to_vec(),
                )]
            },
            expected: names(&["fine"]),
        },
        Case {
            name: "relative include read in place, also inside a block",
            files: |_| {
                vec![
                    file(
                        ".ssh/config",
                        concat!(
                            "Host before\n",
                            "include extra\n",
                            "Host after\n",
                            "Match host after\n",
                            "  Include blocked\n",
                        ),
                    ),
                    file(".ssh/extra", "Host inner\n"),
                    file(".ssh/blocked", "Host matched\n"),
                ]
            },
            expected: names(&["before", "inner", "after", "matched"]),
        },
        Case {
            name: "tilde include",
            files: |_| {
                vec![
                    file(".ssh/config", "Include ~/elsewhere/hosts\n"),
                    file("elsewhere/hosts", "Host tilde\n"),
                ]
            },
            expected: names(&["tilde"]),
        },
        Case {
            name: "absolute include",
            files: |home| {
                let absolute = home.join("absolute").join("hosts");
                vec![
                    file(
                        ".ssh/config",
                        format!("Include \"{}\"\n", absolute.display()),
                    ),
                    file("absolute/hosts", "Host absolute\n"),
                ]
            },
            expected: names(&["absolute"]),
        },
        Case {
            name: "glob include sorted by name",
            files: |_| {
                vec![
                    file(".ssh/config", "Include config.d/*.conf g?/hosts\n"),
                    file(".ssh/config.d/b.conf", "Host bee\n"),
                    file(".ssh/config.d/a.conf", "Host ay\n"),
                    file(".ssh/config.d/c.txt", "Host see\n"),
                    file(".ssh/config.d/.hidden.conf", "Host hidden\n"),
                    file(".ssh/g2/hosts", "Host two\n"),
                    file(".ssh/g1/hosts", "Host one\n"),
                ]
            },
            expected: names(&["ay", "bee", "one", "two"]),
        },
        Case {
            name: "a file included twice outside its own chain",
            files: |_| {
                vec![
                    file(
                        ".ssh/config",
                        "Include shared\nHost middle\nInclude shared\n",
                    ),
                    file(".ssh/shared", "Host shared\n"),
                ]
            },
            expected: names(&["shared", "middle"]),
        },
        Case {
            name: "include loop",
            files: |_| {
                vec![
                    file(".ssh/config", "Host top\nInclude a\n"),
                    file(".ssh/a", "Host ay\nInclude b\n"),
                    file(".ssh/b", "Include ~/.ssh/a\n"),
                ]
            },
            expected: Vec::new(),
        },
        Case {
            name: "depth 16",
            files: |_| chain(16),
            expected: names(&["deep"]),
        },
        Case {
            name: "depth over 16",
            files: |_| chain(17),
            expected: Vec::new(),
        },
        Case {
            name: "file of exactly 256 KiB",
            files: |_| vec![file(".ssh/config", padded("big", 256 * 1024))],
            expected: names(&["big"]),
        },
        Case {
            name: "oversized file",
            files: |_| vec![file(".ssh/config", padded("big", 256 * 1024 + 1))],
            expected: Vec::new(),
        },
        Case {
            name: "over 1 MiB in all",
            files: |_| {
                let mut files = vec![file(".ssh/config", "Include part*\n")];
                for part in 1..=5 {
                    files.push(file(
                        &format!(".ssh/part{part}"),
                        padded(&format!("p{part}"), 210 * 1024),
                    ));
                }
                files
            },
            expected: Vec::new(),
        },
        Case {
            name: "over 64 files",
            files: |_| {
                let mut files = vec![file(".ssh/config", "Include many/*\n")];
                for index in 0..64 {
                    files.push(file(
                        &format!(".ssh/many/f{index:02}"),
                        format!("Host f{index}\n"),
                    ));
                }
                files
            },
            expected: Vec::new(),
        },
        Case {
            name: "missing include",
            files: |_| {
                vec![file(
                    ".ssh/config",
                    "Include nowhere\nInclude absent.d/*\nInclude ~someone/hosts\nHost still\n",
                )]
            },
            expected: names(&["still"]),
        },
        Case {
            name: "no config",
            files: |_| Vec::new(),
            expected: Vec::new(),
        },
    ]
}

#[test]
fn suggestions_follow_the_ssh_config_and_its_includes_within_bounds() {
    for case in cases() {
        let scratch = Scratch::new();
        for (path, content) in (case.files)(&scratch.0) {
            let path = scratch.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
        assert_eq!(host_suggestions(&scratch.0), case.expected, "{}", case.name);
    }
}
