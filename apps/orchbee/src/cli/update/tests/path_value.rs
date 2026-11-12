//! The user `Path` edit of the PATH step: exactly one entry for `<root>\bin` when none names it,
//! every other entry kept byte for byte, names compared without case or trailing separators, and
//! entries naming other folders inside the store root removed. Uninstall's removal is its exact
//! inverse. No test reads or writes a registry.
use super::super::path_value::{Edit, MAXIMUM_LENGTH, edit, removal};

const ROOT: &str = r"C:\Users\Ada\AppData\Local\Programs\Orchbee";
const BIN: &str = r"C:\Users\Ada\AppData\Local\Programs\Orchbee\bin";

fn write(value: &str, removed: bool) -> Edit {
    Edit::Write {
        value: value.to_owned(),
        removed,
    }
}

#[test]
fn the_edit_adds_one_bin_entry_and_keeps_every_other_entry() {
    let digest = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let cases = [
        // Absent or empty: the value becomes the one entry.
        ("", ROOT, write(BIN, false)),
        // Added at the front; the rest is untouched, including empty entries and the final `;`.
        (
            r"C:\Windows;C:\Tools",
            ROOT,
            write(&format!(r"{BIN};C:\Windows;C:\Tools"), false),
        ),
        (
            r"C:\Tools;",
            ROOT,
            write(&format!(r"{BIN};C:\Tools;"), false),
        ),
        ("a;;b", ROOT, write(&format!("{BIN};a;;b"), false)),
        (
            r" C:\Tools ;C:\张三\bin",
            ROOT,
            write(&format!(r"{BIN}; C:\Tools ;C:\张三\bin"), false),
        ),
        // Already named, in any case, quoted, with forward or trailing separators: unchanged.
        (&format!(r"C:\Tools;{BIN}"), ROOT, Edit::Unchanged),
        (
            &format!(r"{}\BIN\\", ROOT.to_uppercase()),
            ROOT,
            Edit::Unchanged,
        ),
        (
            &format!("\"{}/bin/\";x", ROOT.replace('\\', "/")),
            ROOT,
            Edit::Unchanged,
        ),
        (BIN, &format!(r"{ROOT}\"), Edit::Unchanged),
        // Entries naming other folders inside the root, the earlier layout's, leave in the same write.
        (
            &format!(r"{ROOT}\{digest};C:\Tools;{ROOT}\versions\0.1.0\"),
            ROOT,
            write(&format!(r"{BIN};C:\Tools"), true),
        ),
        (
            &format!(r"{ROOT}\bin;{ROOT}\{digest}"),
            ROOT,
            write(&format!(r"{ROOT}\bin"), true),
        ),
        (&format!(r"{ROOT}\{digest}"), ROOT, write(BIN, true)),
        // The root itself, siblings sharing its prefix, dot segments and variables are not inside it.
        (ROOT, ROOT, write(&format!("{BIN};{ROOT}"), false)),
        (
            &format!(r"{ROOT}-old\bin;{ROOT}2"),
            ROOT,
            write(&format!(r"{BIN};{ROOT}-old\bin;{ROOT}2"), false),
        ),
        (
            &format!(r"{ROOT}\bin\..\bin;{ROOT}\..\Other"),
            ROOT,
            write(&format!(r"{BIN};{ROOT}\bin\..\bin;{ROOT}\..\Other"), false),
        ),
        (
            r"%LOCALAPPDATA%\Programs\Orchbee\bin",
            ROOT,
            write(
                &format!(r"{BIN};%LOCALAPPDATA%\Programs\Orchbee\bin"),
                false,
            ),
        ),
        // A store at a drive or share root owns no other folder on it.
        (
            r"D:\Tools;D:\versions\0.1.0",
            r"D:\",
            write(r"D:\bin;D:\Tools;D:\versions\0.1.0", false),
        ),
        (
            r"\\server\share\Tools",
            r"\\server\share",
            write(r"\\server\share\bin;\\server\share\Tools", false),
        ),
        (
            r"\\server\share\store\old;\\server\share\Tools",
            r"\\server\share\store",
            write(r"\\server\share\store\bin;\\server\share\Tools", true),
        ),
    ];
    for (current, root, expected) in cases {
        assert_eq!(edit(current, root), expected, "{current} under {root}");
    }
}

#[test]
fn the_edit_refuses_a_root_no_later_edit_would_recognize() {
    for root in [
        r"C:\Users\Ada\..\Ada\Orchbee",
        r"C:\Users\.\Orchbee",
        r"\\?\C:\Users\Ada\Orchbee",
        r"\\.\C:\Users\Ada\Orchbee",
    ] {
        assert_eq!(edit(r"C:\Tools", root), Edit::UnnamedRoot, "{root}");
    }
}

#[test]
fn the_edit_refuses_a_value_past_the_bound() {
    let filler = |length: usize| format!(r"C:\{}", "x".repeat(length - 3));
    let room = MAXIMUM_LENGTH - BIN.len() - 1;
    let fits = filler(room);
    assert_eq!(edit(&fits, ROOT), write(&format!("{BIN};{fits}"), false));
    assert_eq!(edit(&filler(room + 1), ROOT), Edit::TooLong);
    // Length counts UTF-16 units, not bytes.
    let wide = format!(r"C:\{}", "张".repeat(room - 3));
    assert_eq!(edit(&wide, ROOT), write(&format!("{BIN};{wide}"), false));
}

#[test]
fn the_removal_undoes_the_edit_and_keeps_every_other_entry() {
    // Every value the PATH step starts from comes back byte for byte after its edit and the removal.
    for current in [
        "",
        r"C:\Windows;C:\Tools",
        r"C:\Tools;",
        "a;;b",
        ";",
        r" C:\Tools ;C:\张三\bin",
        ROOT,
        &format!(r"{ROOT}-old\bin;{ROOT}2"),
        &format!(r"{ROOT}\bin\..\bin;{ROOT}\..\Other"),
        r"%LOCALAPPDATA%\Programs\Orchbee\bin",
    ] {
        let Edit::Write { value, .. } = edit(current, ROOT) else {
            panic!("the edit writes {current}");
        };
        assert_eq!(removal(&value, ROOT), write(current, true), "{current}");
    }
    let cases = [
        // Nothing the PATH step wrote: unchanged.
        ("", ROOT, Edit::Unchanged),
        (r"C:\Windows;;C:\Tools;", ROOT, Edit::Unchanged),
        (&format!(r"{ROOT};{ROOT}-old\bin"), ROOT, Edit::Unchanged),
        // The entry in any spelling the edit recognizes, and every folder inside the root, go.
        (
            &format!(
                r#""{}/BIN/";x;{ROOT}\versions\0.1.0"#,
                ROOT.replace('\\', "/")
            ),
            ROOT,
            write("x", true),
        ),
        (&format!(r"x;{BIN};y;{BIN}\"), ROOT, write("x;y", true)),
        (BIN, &format!(r"{ROOT}\"), write("", true)),
        // A store at a drive root owns only its `bin` entry there.
        (
            r"D:\bin;D:\Tools;D:\versions\0.1.0",
            r"D:\",
            write(r"D:\Tools;D:\versions\0.1.0", true),
        ),
    ];
    for (current, root, expected) in cases {
        assert_eq!(removal(current, root), expected, "{current} under {root}");
    }
}
