use super::display;

#[test]
fn git_paths_render_with_native_separators() {
    let rendered = display("C:/Users/me/code/orchbee");
    if cfg!(windows) {
        assert_eq!(rendered, r"C:\Users\me\code\orchbee");
    } else {
        assert_eq!(rendered, "C:/Users/me/code/orchbee");
    }
    assert_eq!(display("relative/path"), "relative/path");
    assert_eq!(
        display("C:/x\u{1b}"),
        if cfg!(windows) {
            r"C:\x\u{001b}"
        } else {
            r"C:/x\u{001b}"
        }
    );
}
