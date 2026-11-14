//! Shared projection text uses the same rows and ordinals as its machine document.
use super::{Inspection, List};
use crate::cli_text::CliText;
use crate::{
    cli::{
        output::{Bounded, TooLarge},
        present::{compact_table, truncate_terminal_cell},
    },
    invocation::QueryCommand,
    localization::Locale,
};

pub(in crate::cli) fn render_list(
    out: &mut Bounded,
    list: &List,
    locale: Locale,
) -> Result<(), TooLarge> {
    let headers = CliText::new(locale).cli_projection_headers();
    let handle_width = list
        .items
        .iter()
        .filter_map(|i| i.panel_handle.as_ref())
        .map(String::len)
        .max()
        .unwrap_or(0)
        .max(crate::cli_text::escape::display_width(headers[1]));
    let name_width = list
        .items
        .iter()
        .filter_map(|i| i.name.as_deref())
        .map(crate::cli_text::escape::display_width)
        .max()
        .unwrap_or(0)
        .clamp(crate::cli_text::escape::display_width(headers[3]), 12);
    // 80 cells less the fixed columns (#, AGENT, STATE) and five two-cell gaps.
    let cwd_width = 45usize.saturating_sub(handle_width + name_width);
    compact_table(
        out,
        &headers,
        &[4, handle_width, 12, name_width, 9, cwd_width],
        &list.items,
        |item| {
            vec![
                item.ordinal.to_string(),
                item.panel_handle.clone().unwrap_or_else(|| "-".into()),
                truncate_terminal_cell(item.row.provider(), 12),
                truncate_terminal_cell(item.name.as_deref().unwrap_or("-"), name_width),
                CliText::new(locale)
                    .cli_projection_status(item.row.status())
                    .into(),
                truncate_terminal_cell(item.cwd.as_deref().unwrap_or("-"), cwd_width),
            ]
        },
    )?;
    if let Some(cursor) = &list.next {
        out.push(&CliText::new(locale).cli_agent_next_cursor(cursor.as_str()))?;
    }
    Ok(())
}
pub(in crate::cli) fn render_inspection(
    out: &mut Bounded,
    doc: &Inspection,
    command: &QueryCommand,
    locale: Locale,
) -> Result<(), TooLarge> {
    let headers = CliText::new(locale).cli_projection_headers();
    out.write(format_args!(
        "{}: {}\n{}: {}\n",
        headers[2],
        crate::cli_text::escape::unquoted(doc.row.provider()),
        headers[4],
        CliText::new(locale).cli_projection_status(doc.row.status()),
    ))?;
    crate::cli::present::render(out, &doc.detail, command, locale)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        agent_projection::tests::{rows, state},
        cli::agent_projection::Item,
    };

    #[test]
    fn list_text_and_json_share_global_number_provider_state_and_nullable_directory() {
        let row = rows(Vec::new(), vec![state(12, Some("grok-build"))]).remove(0);
        let list = List {
            items: vec![Item {
                ordinal: 51,
                row,
                panel_handle: Some("@panel:0xb".into()),
                name: Some("reviewer".into()),
                cwd: Some("C:\\项目\\very-long-directory-which-is-clipped".into()),
                client: None,
            }],
            revision: orchbee_agent_observation_protocol::AgentSafeInteger::new(2),
            next: None,
            client: None,
        };
        let json = list.document();
        assert_eq!(json["items"][0]["ordinal"], 51);
        assert_eq!(json["items"][0]["provider"], "grok");
        assert_eq!(json["items"][0]["source"], "terminal");
        assert_eq!(json["items"][0]["name"], "reviewer");
        assert!(json["items"][0]["canonical"].is_null());
        for locale in [Locale::EnUs, Locale::ZhCn] {
            let text = String::from_utf8(
                crate::cli::output::render_text(|out| render_list(out, &list, locale)).unwrap(),
            )
            .unwrap();
            assert!(text.contains("@panel:0xb"));
            assert!(text.contains("grok"));
            assert!(text.contains("reviewer"));
            assert!(text.contains(CliText::new(locale).cli_projection_status("idle")));
            assert_eq!(text.lines().count(), 2);
            assert!(
                text.lines()
                    .all(|line| crate::cli_text::escape::display_width(line) <= 80),
                "{text}"
            );
        }
    }
}
