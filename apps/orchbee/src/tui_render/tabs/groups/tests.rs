use super::*;
use crate::tui_theme::{ColorLevel, GlyphSet};
use orchbee_workbench_protocol::WorkbenchTabGroupId;
use ratatui::{Terminal, backend::TestBackend};

#[test]
fn unnamed_group_has_localized_visible_header_and_identity_hit_region() {
    let group = TabGroup {
        group_id: WorkbenchTabGroupId::parse(&format!("{:032x}", 1)).unwrap(),
        label: String::new(),
        color: TabGroupColor::Grey,
    };
    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        let label = locale.shell().unnamed_tab_group;
        let mut terminal = Terminal::new(TestBackend::new(80, 1)).unwrap();
        let mut hits = Vec::new();
        let tabs = [GroupTabDisplay {
            tab: TabDisplay {
                position: 1,
                active: true,
                label: "1".into(),
            },
            group: Some(&group),
            collapsed: true,
            selected: false,
        }];
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    &tabs,
                    &Theme::terminal_native(),
                    label,
                    &mut |index, header, x, cells| hits.push((index, header, x, cells)),
                )
            })
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        // TestBackend includes a padding cell after each full-width glyph.
        assert!(text.replace(' ', "").contains(&label.replace(' ', "")));
        assert_eq!(hits.len(), 1);
        assert!(hits[0].1 && hits[0].3 > 0);
    }
}

#[test]
fn headers_clip_by_cells_keep_active_member_and_never_emit_control_bytes() {
    let group = TabGroup {
        group_id: WorkbenchTabGroupId::parse(&format!("{:032x}", 1)).unwrap(),
        label: "组👩‍💻\x1b[2J\nwide".into(),
        color: TabGroupColor::Orange,
    };
    for width in [1, 4, 8, 16, 40, 80, 120] {
        for collapsed in [false, true] {
            for glyphs in [GlyphSet::UTF8, GlyphSet::ASCII] {
                let theme = Theme::terminal_native()
                    .with_glyphs(glyphs)
                    .quantize(ColorLevel::Monochrome);
                let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
                let tabs: Vec<_> = (0..5)
                    .map(|index| GroupTabDisplay {
                        tab: TabDisplay {
                            position: index + 1,
                            active: index == 3,
                            label: "编译 very long terminal".into(),
                        },
                        group: Some(&group),
                        collapsed,
                        selected: false,
                    })
                    .collect();
                let mut hits = Vec::new();
                terminal
                    .draw(|frame| {
                        render(
                            frame,
                            frame.area(),
                            &tabs,
                            &theme,
                            "Unnamed group",
                            &mut |index, header, x, cells| hits.push((index, header, x, cells)),
                        )
                    })
                    .unwrap();
                assert!(
                    hits.iter()
                        .any(|(index, header, _, _)| *index == 3 && *header == collapsed),
                    "width={width}, collapsed={collapsed}, glyphs={glyphs:?}, hits={hits:?}"
                );
                assert!(
                    hits.iter()
                        .all(|(_, _, x, cells)| *cells > 0 && x + cells <= usize::from(width))
                );
                if collapsed {
                    assert_eq!(hits.len(), 1);
                }
                let text: String = terminal
                    .backend()
                    .buffer()
                    .content()
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect();
                assert!(!text.contains('\x1b') && !text.contains('\n'));
                assert!(
                    terminal
                        .backend()
                        .buffer()
                        .content()
                        .iter()
                        .all(|cell| cell.fg == ratatui::style::Color::Reset
                            && cell.bg == ratatui::style::Color::Reset)
                );
            }
        }
    }
}
