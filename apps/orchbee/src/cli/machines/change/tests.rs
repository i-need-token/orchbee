use super::below;

/// The folder a moved Workspace starts in hangs on this comparison: a folder at or below home keeps
/// its own spelling below home, as Windows compares it on a Windows home, and nothing else counts.
#[test]
fn a_folder_counts_below_home_as_its_machine_compares_paths() {
    let parts = |home, folder| below(home, folder);
    assert_eq!(
        parts(r"C:\Users\Ann", r"c:\users\ann\Code/Docs"),
        Some(vec!["Code".to_owned(), "Docs".to_owned()])
    );
    assert_eq!(parts(r"C:\Users\Ann", r"C:\Users\Ann\"), Some(vec![]));
    assert_eq!(parts(r"C:\Users\Ann", r"C:\Users\Anna\code"), None);
    assert_eq!(parts(r"C:\Users\Ann", r"D:\code"), None);
    assert_eq!(
        parts("/Users/ann", "/Users/ann/code/docs"),
        Some(vec!["code".to_owned(), "docs".to_owned()])
    );
    assert_eq!(parts("/Users/ann", "/Users/Ann/code"), None);
    assert_eq!(parts("/Users/ann", "/Users"), None);
}
