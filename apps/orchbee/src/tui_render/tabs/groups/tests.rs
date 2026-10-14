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
                    GroupInteractions::new(
                        &mut (),
                        &mut |_, _, _, _| crate::tui_pointer::ProjectedPointer::Rest,
                        &mut |_, index, header, rect| {
                            hits.push((index, header, usize::from(rect.x), usize::from(rect.width)))
                        },
                    ),
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
                            GroupInteractions::new(
                                &mut (),
                                &mut |_, _, _, _| crate::tui_pointer::ProjectedPointer::Rest,
                                &mut |_, index, header, rect| {
                                    hits.push((
                                        index,
                                        header,
                                        usize::from(rect.x),
                                        usize::from(rect.width),
                                    ))
                                },
                            ),
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

fn rendered_group_state(
    active: bool,
    selected: bool,
    collapsed: bool,
) -> (ratatui::buffer::Buffer, Vec<(usize, bool, usize, usize)>) {
    let group = TabGroup {
        group_id: WorkbenchTabGroupId::parse(&format!("{:032x}", 1)).unwrap(),
        label: "Group".into(),
        color: TabGroupColor::Blue,
    };
    let tabs = [GroupTabDisplay {
        tab: TabDisplay {
            position: 1,
            active,
            label: "member".into(),
        },
        group: Some(&group),
        collapsed,
        selected,
    }];
    let mut terminal = Terminal::new(TestBackend::new(80, 1)).unwrap();
    let mut hits = Vec::new();
    terminal
        .draw(|frame| {
            render(
                frame,
                frame.area(),
                &tabs,
                &Theme::rgb_test_dark(),
                "Unnamed group",
                GroupInteractions::new(
                    &mut (),
                    &mut |_, _, _, _| crate::tui_pointer::ProjectedPointer::Rest,
                    &mut |_, index, header, rect| {
                        hits.push((index, header, usize::from(rect.x), usize::from(rect.width)))
                    },
                ),
            )
        })
        .unwrap();
    (terminal.backend().buffer().clone(), hits)
}

#[test]
fn checked_and_current_tracks_keep_fixed_member_geometry() {
    let (unchecked, unchecked_hits) = rendered_group_state(true, false, false);
    let (checked, checked_hits) = rendered_group_state(true, true, false);
    let theme = Theme::rgb_test_dark();

    let (_, _, unchecked_x, unchecked_width) = unchecked_hits
        .iter()
        .copied()
        .find(|(_, header, _, _)| !header)
        .expect("expanded group paints its member");
    let (_, _, checked_x, checked_width) = checked_hits
        .iter()
        .copied()
        .find(|(_, header, _, _)| !header)
        .expect("expanded group paints its member");
    assert_eq!(unchecked_width, checked_width);
    let unchecked_x = u16::try_from(unchecked_x).unwrap();
    let checked_x = u16::try_from(checked_x).unwrap();
    assert_eq!(unchecked[(unchecked_x, 0)].symbol(), " ");
    assert_eq!(checked[(checked_x, 0)].symbol(), "*");
    assert_eq!(
        unchecked[(unchecked_x + 2, 0)].symbol(),
        theme.glyphs.active_marker
    );
    assert_eq!(
        checked[(checked_x + 2, 0)].symbol(),
        theme.glyphs.active_marker
    );
    assert_eq!(
        checked[(checked_x, 0)].fg,
        theme.color(crate::tui_theme::Role::AccentSelection)
    );
}

#[test]
fn collapsed_group_reserves_the_current_marker_track() {
    let (inactive, inactive_hits) = rendered_group_state(false, false, true);
    let (active, active_hits) = rendered_group_state(true, false, true);
    let theme = Theme::rgb_test_dark();

    assert_eq!(inactive_hits, active_hits);
    assert_eq!(inactive[(1, 0)].symbol(), " ");
    assert_eq!(active[(1, 0)].symbol(), theme.glyphs.active_marker);
    assert_eq!(inactive[(2, 0)].symbol(), theme.glyphs.disclosure_closed);
    assert_eq!(active[(2, 0)].symbol(), theme.glyphs.disclosure_closed);
}

#[test]
fn pointer_surfaces_preserve_group_color_and_member_checked_current_tracks() {
    use crate::{
        tui_pointer::ProjectedPointer,
        tui_theme::{Role, Theme},
    };

    let group = TabGroup {
        group_id: WorkbenchTabGroupId::parse(&format!("{:032x}", 1)).unwrap(),
        label: "Group".into(),
        color: TabGroupColor::Blue,
    };
    let tabs = [GroupTabDisplay {
        tab: TabDisplay {
            position: 1,
            active: true,
            label: "member".into(),
        },
        group: Some(&group),
        collapsed: false,
        selected: true,
    }];
    let theme = Theme::rgb_test_dark();

    for (header_target, pointer, role) in [
        (true, ProjectedPointer::Hovered, Role::SurfaceHover),
        (false, ProjectedPointer::Pressed, Role::SurfacePressed),
    ] {
        let mut terminal = Terminal::new(TestBackend::new(80, 1)).unwrap();
        let mut tokens = Vec::new();
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    &tabs,
                    &theme,
                    "Unnamed group",
                    GroupInteractions::new(
                        &mut (),
                        &mut |_, _, header, _| {
                            if header == header_target {
                                pointer
                            } else {
                                ProjectedPointer::Rest
                            }
                        },
                        &mut |_, index, header, rect| tokens.push((index, header, rect)),
                    ),
                )
            })
            .unwrap();

        let target = tokens
            .iter()
            .find(|(_, header, _)| *header == header_target)
            .expect("the targeted group token is painted")
            .2;
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(target.x, target.y)].bg, theme.color(role));
        if header_target {
            assert_eq!(
                buffer[(target.x + 2, target.y)].symbol(),
                theme.glyphs.disclosure_open
            );
            assert_eq!(
                buffer[(target.x + 2, target.y)].style().fg,
                theme.tab_group_style(TabGroupColor::Blue).fg
            );
        } else {
            assert_eq!(
                buffer[(target.x, target.y)].symbol(),
                theme.glyphs.active_marker
            );
            assert_eq!(
                buffer[(target.x + 2, target.y)].symbol(),
                theme.glyphs.active_marker
            );
            assert_eq!(
                buffer[(target.x, target.y)].fg,
                theme.color(Role::AccentSelection)
            );
        }
    }
}
