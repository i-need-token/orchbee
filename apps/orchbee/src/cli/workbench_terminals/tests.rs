use super::*;
use crate::cli::query::terminal_metadata::{Metadata, Read};
use crate::cli_text::CliText;
use orchbee_terminal_protocol::decode_terminal_text_frame;
use serde_json::json;
use std::sync::Arc;

fn observed(title: &str, current_directory: &str) -> Read {
    let frame = decode_terminal_text_frame(include_bytes!(
        "../../../../../schemas/terminal/v1/fixtures/terminal-frame.bin"
    ))
    .unwrap();
    Read {
        frame: Metadata {
            route: frame.route,
            columns: frame.geometry.columns,
            rows: frame.geometry.rows,
            alternate_screen: false,
            scrollback_rows: frame.scrollback_rows,
            title: Some(Arc::from(title)),
            current_directory: Some(Arc::from(current_directory)),
        },
        processes: None,
    }
}

#[test]
fn panel_terminal_text_keeps_paths_copyable_and_escapes_controls_without_changing_json() {
    let panel_id = "4".repeat(32);
    let title = "C:\\title \"quoted\" 🐝\u{001b}\u{202e}\nnext";
    let current_directory = "C:\\项目\\dir \"quoted\"";
    let document = Document(BTreeMap::from([(
        panel_id.clone(),
        Observation::Observed(Box::new(observed(title, current_directory))),
    )]));

    let mut result = json!({"panel":{"panelId":panel_id}});
    document.enrich(&mut result);
    assert_eq!(result["panel"]["terminalTitle"], title);
    assert_eq!(result["panel"]["terminalTitleStripped"], title);
    assert_eq!(result["panel"]["cwd"], current_directory);

    let rendered_title = r#"C:\title "quoted" 🐝\u{001b}\u{202e}\nnext"#;
    for locale in [Locale::EnUs, Locale::ZhCn] {
        let bytes = crate::cli::output::render_text(|out| document.render(out, locale)).unwrap();
        let text = String::from_utf8(bytes).unwrap();
        let expected = format!(
            "{}\n{panel_id}  {rendered_title}  {current_directory}\n",
            CliText::new(locale).cli_terminal_observations_heading()
        );
        assert_eq!(text, expected);
        assert!(!text.contains('\u{001b}'));
        assert!(!text.contains('\u{202e}'));
    }
}
