use super::*;
use crate::tui_theme::{ColorLevel, GlyphSet};
use orchbee_workbench_protocol::{MetadataState, WorkbenchTabGroupId};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, style::Color};

type Hits = Vec<(usize, StripPart, Rect)>;

fn draw(
    tabs: &[GroupTabDisplay<'_>],
    theme: &Theme,
    width: u16,
    pointer: impl Fn(usize, StripPart) -> ProjectedPointer,
) -> (Buffer, Hits) {
    draw_from(tabs, theme, width, None, pointer)
}

fn draw_from(
    tabs: &[GroupTabDisplay<'_>],
    theme: &Theme,
    width: u16,
    start: Option<StripStart>,
    pointer: impl Fn(usize, StripPart) -> ProjectedPointer,
) -> (Buffer, Hits) {
    let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
    let mut hits = Vec::new();
    terminal
        .draw(|frame| {
            render(
                frame,
                frame.area(),
                tabs,
                theme,
                "Unnamed group",
                0,
                start,
                GroupInteractions::new(
                    &mut (),
                    &mut |_, index, part, _| pointer(index, part),
                    &mut |_, index, part, rect| hits.push((index, part, rect)),
                ),
            );
        })
        .unwrap();
    (terminal.backend().buffer().clone(), hits)
}

fn text(buffer: &Buffer) -> String {
    buffer.content().iter().map(|cell| cell.symbol()).collect()
}

fn tab<'a>(label: &str, active: bool, group: Option<&'a TabGroup>) -> GroupTabDisplay<'a> {
    GroupTabDisplay {
        tab: TabDisplay {
            position: 1,
            active,
            label: label.to_owned().into(),
        },
        group,
        collapsed: false,
        selected: false,
        pinned: false,
        agent: None,
    }
}

fn group(label: &str, color: TabGroupColor) -> TabGroup {
    TabGroup {
        group_id: WorkbenchTabGroupId::parse(&format!("{:032x}", 1)).unwrap(),
        label: label.into(),
        color,
    }
}

fn rest(_: usize, _: StripPart) -> ProjectedPointer {
    ProjectedPointer::Rest
}

fn is_page(part: StripPart) -> bool {
    matches!(part, StripPart::Page { .. })
}

#[test]
fn overflow_markers_keep_the_active_tab_visible_and_page_the_strip() {
    for glyphs in [GlyphSet::UTF8, GlyphSet::ASCII] {
        let theme = Theme::terminal_native().with_glyphs(glyphs);
        for active in [0, 4, 8] {
            let tabs: Vec<_> = (0..9)
                .map(|index| tab(&format!("task-{index}"), index == active, None))
                .collect();
            let (buffer, hits) = draw(&tabs, &theme, 36, rest);
            let text = text(&buffer);
            assert!(text.contains(&format!("task-{active}")), "{text}");
            assert!(hits.iter().any(|(index, _, _)| *index == active));
            let (pages, painted): (Vec<_>, Vec<_>) =
                hits.iter().partition(|(_, part, _)| is_page(*part));
            assert_eq!(
                pages.len(),
                usize::from(active > 0) + usize::from(active < 8)
            );
            // Each marker is its own control and pages to Tabs beyond the painted ones.
            if active > 0 {
                assert!(text.starts_with(theme.glyphs.ellipsis), "{text}");
                let (target, _, marker) = pages.iter().find(|(_, _, rect)| rect.x == 0).unwrap();
                assert!(
                    painted
                        .iter()
                        .all(|(index, _, rect)| rect.x > marker.right() && index > target)
                );
            }
            if active < 8 {
                assert!(text.trim_end().ends_with(theme.glyphs.ellipsis), "{text}");
                let (target, _, marker) = pages
                    .iter()
                    .find(|(_, _, rect)| rect.right() == 36)
                    .unwrap();
                assert!(
                    painted
                        .iter()
                        .all(|(index, _, rect)| rect.right() < marker.x && index <= target)
                );
            }
        }
    }
}

#[test]
fn paged_windows_start_at_their_token_and_page_by_whole_windows() {
    let theme = Theme::terminal_native();
    let page = |hits: &Hits, leading: bool| {
        hits.iter()
            .find(|(_, part, rect)| is_page(*part) && (rect.x == 0) == leading)
            .map(|(index, part, _)| (*index, *part))
    };
    let group = group("group-name-long", TabGroupColor::Blue);
    let tabs: Vec<_> = (0..9)
        .map(|index| {
            tab(
                &format!("task-{index}"),
                index == 0,
                (2..6).contains(&index).then_some(&group),
            )
        })
        .collect();
    // The first page ends inside the shortened group header, so the next page opens with it.
    let (buffer, hits) = draw(&tabs, &theme, 36, rest);
    assert!(text(&buffer).starts_with(" task-0"), "{}", text(&buffer));
    assert_eq!(page(&hits, true), None);
    assert_eq!(
        page(&hits, false),
        Some((2, StripPart::Page { header: true }))
    );
    // A paged window starts at its token wherever the current Tab is, and its markers page one
    // whole window back to the first page and forward to the first Tab it does not show.
    let start = StripStart {
        index: 2,
        header: true,
    };
    let (buffer, hits) = draw_from(&tabs, &theme, 36, Some(start), rest);
    let paged = text(&buffer);
    assert!(paged.starts_with("…  group-name-long"), "{paged}");
    assert!(!paged.contains("task-0"), "{paged}");
    assert!(
        hits.iter()
            .all(|(index, part, _)| *index != 0 || is_page(*part))
    );
    assert_eq!(
        page(&hits, true),
        Some((0, StripPart::Page { header: false }))
    );
    assert_eq!(
        page(&hits, false),
        Some((3, StripPart::Page { header: false }))
    );
    // The start applies only while the Tabs overflow a strip wide enough for its markers.
    for (count, width) in [(2, 36), (9, 7)] {
        let (_, hits) = draw_from(
            &tabs[..count],
            &theme,
            width,
            Some(StripStart {
                index: 1,
                header: false,
            }),
            rest,
        );
        assert!(
            hits.iter()
                .any(|(index, part, _)| *index == 0 && *part == StripPart::Tab)
        );
        assert!(hits.iter().all(|(_, part, _)| !is_page(*part)));
    }
}

#[test]
fn unnamed_group_has_localized_visible_header_and_identity_hit_region() {
    let group = group("", TabGroupColor::Grey);
    for locale in [
        crate::localization::Locale::EnUs,
        crate::localization::Locale::ZhCn,
    ] {
        let label = locale.shell().unnamed_tab_group;
        let mut collapsed = tab("1", true, Some(&group));
        collapsed.collapsed = true;
        let mut terminal = Terminal::new(TestBackend::new(80, 1)).unwrap();
        let mut hits = Vec::new();
        terminal
            .draw(|frame| {
                render(
                    frame,
                    frame.area(),
                    std::slice::from_ref(&collapsed),
                    &Theme::terminal_native(),
                    label,
                    0,
                    None,
                    GroupInteractions::new(
                        &mut (),
                        &mut |_, _, _, _| ProjectedPointer::Rest,
                        &mut |_, index, part, rect| hits.push((index, part, rect)),
                    ),
                );
            })
            .unwrap();
        // TestBackend includes a padding cell after each full-width glyph.
        let text = text(terminal.backend().buffer());
        assert!(text.replace(' ', "").contains(&label.replace(' ', "")));
        assert_eq!(hits.len(), 1);
        assert!(hits[0].1 == StripPart::Header && hits[0].2.width > 0);
    }
}

#[test]
fn headers_clip_by_cells_keep_active_member_without_stubs_or_control_bytes() {
    let group = group("组👩‍💻\x1b[2J\nwide", TabGroupColor::Orange);
    for width in [1, 4, 8, 16, 40, 80, 120] {
        for collapsed in [false, true] {
            for glyphs in [GlyphSet::UTF8, GlyphSet::ASCII] {
                let theme = Theme::terminal_native()
                    .with_glyphs(glyphs)
                    .quantize(ColorLevel::Monochrome);
                let tabs: Vec<_> = (0..5)
                    .map(|index| {
                        let mut member = tab("编译 very long terminal", index == 3, Some(&group));
                        member.collapsed = collapsed;
                        member
                    })
                    .collect();
                let (buffer, hits) = draw(&tabs, &theme, width, rest);
                let part = if collapsed {
                    StripPart::Header
                } else {
                    StripPart::Tab
                };
                // A strip too narrow for the active token's minimum label leaves it to the
                // markers instead of painting a clipped stub.
                let kept = hits
                    .iter()
                    .any(|(index, hit, _)| *index == 3 && *hit == part);
                assert!(
                    kept || (width < 16 && hits.iter().all(|(_, hit, _)| is_page(*hit))),
                    "width={width}, collapsed={collapsed}, glyphs={glyphs:?}, hits={hits:?}"
                );
                assert!(
                    hits.iter()
                        .all(|(_, _, rect)| rect.width > 0 && rect.right() <= width)
                );
                if collapsed {
                    assert_eq!(hits.len(), usize::from(kept));
                }
                let text = text(&buffer);
                assert!(!text.contains('\x1b') && !text.contains('\n'));
                assert!(buffer.content().iter().all(|cell| {
                    cell.fg == ratatui::style::Color::Reset
                        && cell.bg == ratatui::style::Color::Reset
                }));
            }
        }
    }
}

#[test]
fn close_reveals_on_the_active_or_pointed_tab_without_moving_the_strip() {
    let theme = Theme::rgb_test_dark();
    let tabs = [tab("build", true, None), tab("logs", false, None)];
    let (resting, resting_hits) = draw(&tabs, &theme, 60, rest);
    let (hovered, hovered_hits) = draw(&tabs, &theme, 60, |index, part| {
        if index == 1 && part == StripPart::Tab {
            ProjectedPointer::Hovered
        } else {
            ProjectedPointer::Rest
        }
    });
    let rects = |hits: &Hits| hits.iter().map(|(_, _, rect)| *rect).collect::<Vec<_>>();
    assert_eq!(rects(&resting_hits), rects(&hovered_hits));

    let track = |hits: &Hits, index: usize| {
        hits.iter()
            .rfind(|(hit, _, _)| *hit == index)
            .copied()
            .expect("every Tab records its close track")
    };
    let symbols = |buffer: &Buffer, rect: Rect| {
        (rect.x..rect.right())
            .map(|x| buffer[(x, 0)].symbol())
            .collect::<String>()
    };
    let (_, part, rect) = track(&resting_hits, 0);
    assert_eq!(
        (part, symbols(&resting, rect).as_str()),
        (StripPart::Close, "[x]")
    );
    let (_, part, rect) = track(&resting_hits, 1);
    assert_eq!(
        (part, symbols(&resting, rect).as_str()),
        (StripPart::Tab, "   ")
    );
    let (_, part, rect) = track(&hovered_hits, 1);
    assert_eq!(
        (part, symbols(&hovered, rect).as_str()),
        (StripPart::Close, "[x]")
    );
    assert_eq!(hovered[(rect.x, 0)].bg, theme.color(Role::SurfaceHover));

    let (pointed, _) = draw(&tabs, &theme, 60, |index, part| {
        if index == 0 && part == StripPart::Close {
            ProjectedPointer::Hovered
        } else {
            ProjectedPointer::Rest
        }
    });
    let (_, _, close) = track(&resting_hits, 0);
    assert_eq!(
        pointed[(close.x + 1, 0)].fg,
        theme.color(Role::AccentDestructive)
    );
}

#[test]
fn a_pointed_inactive_tab_label_rises_to_primary_text_without_bold() {
    let theme = Theme::rgb_test_dark();
    let tabs = [tab("build", true, None), tab("logs", false, None)];
    for (pointer, role, bold) in [
        (ProjectedPointer::Rest, Role::TextSecondary, false),
        (ProjectedPointer::Hovered, Role::TextPrimary, false),
        (ProjectedPointer::Pressed, Role::TextPrimary, true),
    ] {
        let (buffer, hits) = draw(&tabs, &theme, 60, |index, part| {
            if index == 1 && part == StripPart::Tab {
                pointer
            } else {
                ProjectedPointer::Rest
            }
        });
        let (_, _, rect) = hits.iter().find(|(hit, _, _)| *hit == 1).copied().unwrap();
        let label = &buffer[(rect.x + 1, 0)];
        assert_eq!(label.symbol(), "l");
        assert_eq!(label.fg, theme.color(role), "{pointer:?}");
        assert_eq!(label.modifier.contains(Modifier::BOLD), bold, "{pointer:?}");
    }
}

#[test]
fn terminal_native_working_reads_on_the_current_tab_fill() {
    let theme = Theme::terminal_native();
    let mut working = tab("agent", true, None);
    working.agent = Some(TabAgent {
        state: MetadataState::Working,
        unread: false,
    });
    let (buffer, _) = draw(std::slice::from_ref(&working), &theme, 40, rest);
    let glyph = &buffer[(1, 0)];
    assert_eq!(
        glyph.symbol(),
        state_glyph(MetadataState::Working, &theme, 0)
    );
    let current = theme.style(Role::SurfaceCurrent);
    assert_eq!((Some(glyph.fg), Some(glyph.bg)), (current.fg, current.bg));
}

#[test]
fn terminal_native_pointer_bands_keep_agent_and_unread_hues_off_reverse_video() {
    // The reverse band would show each hue as a block behind a glyph in the band's text color.
    let theme = Theme::terminal_native();
    let mut blocked = tab("agent", false, None);
    blocked.agent = Some(TabAgent {
        state: MetadataState::Blocked,
        unread: true,
    });
    for pointer in [ProjectedPointer::Hovered, ProjectedPointer::Pressed] {
        let (buffer, _) = draw(std::slice::from_ref(&blocked), &theme, 40, |_, _| pointer);
        assert!(text(&buffer).starts_with(&format!(
            " {} agent {}",
            state_glyph(MetadataState::Blocked, &theme, 0),
            theme.glyphs.status_bullet
        )));
        for (x, role) in [(1, Role::AgentAttention), (9, Role::StatusUnread)] {
            let cell = &buffer[(x, 0)];
            assert_eq!(cell.fg, theme.color(role), "{pointer:?} {role:?}");
            assert!(
                matches!(cell.bg, Color::Black | Color::White),
                "{pointer:?} {role:?}"
            );
            assert!(
                !cell.modifier.contains(Modifier::REVERSED),
                "{pointer:?} {role:?}"
            );
        }
        // The label and the gaps keep the band.
        for x in [0, 2, 3, 8, 10] {
            assert!(
                buffer[(x, 0)].modifier.contains(Modifier::REVERSED),
                "{pointer:?} {x}"
            );
        }
    }
}

#[test]
fn active_and_checked_tabs_keep_their_geometry_and_bands() {
    let theme = Theme::rgb_test_dark();
    let mut tabs = [tab("member", true, None), tab("other", false, None)];
    let (_, plain_hits) = draw(&tabs, &theme, 60, rest);
    tabs[1].selected = true;
    let (buffer, checked_hits) = draw(&tabs, &theme, 60, rest);
    assert_eq!(plain_hits, checked_hits);
    let label = |index: usize| {
        let (_, _, rect) = checked_hits
            .iter()
            .find(|(hit, _, _)| *hit == index)
            .copied()
            .unwrap();
        &buffer[(rect.x + 1, 0)]
    };
    assert_eq!(label(0).bg, theme.color(Role::SurfaceCurrent));
    assert!(label(0).modifier.contains(Modifier::BOLD));
    assert_eq!(label(1).bg, theme.color(Role::SurfaceSelection));
    assert!(!text(&buffer).contains(theme.glyphs.active_marker));
}

#[test]
fn agent_cues_lead_tabs_and_a_collapsed_group_carries_its_most_urgent_member() {
    let theme = Theme::rgb_test_dark();
    let agent = |state, unread| Some(TabAgent { state, unread });
    let mut working = tab("agent", true, None);
    working.agent = agent(MetadataState::Working, true);
    let (buffer, _) = draw(std::slice::from_ref(&working), &theme, 40, rest);
    let glyph = state_glyph(MetadataState::Working, &theme, 0);
    assert!(text(&buffer).starts_with(&format!(" {glyph} agent {}", theme.glyphs.status_bullet)));
    assert_eq!(buffer[(1, 0)].fg, theme.color(Role::StatusWorking));
    assert_eq!(buffer[(9, 0)].fg, theme.color(Role::StatusUnread));

    let group = group("Agents", TabGroupColor::Blue);
    let members: Vec<_> = [
        agent(MetadataState::Done, false),
        agent(MetadataState::Blocked, true),
        None,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, agent)| {
        let mut member = tab("member", index == 0, Some(&group));
        member.collapsed = true;
        member.agent = agent;
        member
    })
    .collect();
    let (buffer, hits) = draw(&members, &theme, 40, rest);
    assert_eq!(hits.len(), 1);
    let blocked = state_glyph(MetadataState::Blocked, &theme, 0);
    assert!(
        text(&buffer).starts_with(&format!(" {blocked} Agents {}", theme.glyphs.status_bullet)),
        "{}",
        text(&buffer)
    );
}

#[test]
fn group_header_contrast_pairs_follow_pointer_transitions_and_current_context() {
    let mut group = group("Runtime", TabGroupColor::Blue);
    for base in [
        Theme::one_light(),
        Theme::one_dark(),
        Theme::terminal_native(),
    ] {
        for level in [
            ColorLevel::Truecolor,
            ColorLevel::Ansi16,
            ColorLevel::Monochrome,
        ] {
            let theme = base.quantize(level);
            for color in [
                TabGroupColor::Blue,
                TabGroupColor::Cyan,
                TabGroupColor::Green,
                TabGroupColor::Grey,
                TabGroupColor::Orange,
                TabGroupColor::Pink,
                TabGroupColor::Purple,
                TabGroupColor::Red,
                TabGroupColor::Yellow,
            ] {
                group.color = color;
                for (collapsed, active) in [(false, false), (true, false), (true, true)] {
                    let mut member = tab("Log", active, Some(&group));
                    member.collapsed = collapsed;
                    let tabs = [member];
                    let mut resting = None;
                    for pointer in [
                        ProjectedPointer::Rest,
                        ProjectedPointer::Hovered,
                        ProjectedPointer::Pressed,
                        ProjectedPointer::Rest,
                    ] {
                        let (buffer, hits) = draw(&tabs, &theme, 80, |_, part| {
                            if part == StripPart::Header {
                                pointer
                            } else {
                                ProjectedPointer::Rest
                            }
                        });
                        let (_, _, header) = hits
                            .iter()
                            .copied()
                            .find(|(_, part, _)| *part == StripPart::Header)
                            .expect("the group header remains visible");
                        let surface = match pointer {
                            ProjectedPointer::Rest => {
                                (collapsed && active).then_some(Role::SurfaceCurrent)
                            }
                            ProjectedPointer::Hovered => Some(Role::SurfaceHover),
                            ProjectedPointer::Pressed => Some(Role::SurfacePressed),
                        };
                        let expected = theme.tab_group_header_style(
                            color,
                            surface,
                            None,
                            Style::new()
                                .fg(theme.tab_groups.accent(color))
                                .add_modifier(Modifier::BOLD),
                        );
                        let mut cell = ratatui::buffer::Cell::default();
                        cell.set_style(expected);
                        // The name follows the header's leading pad.
                        let name = &buffer[(header.x + 1, header.y)];
                        assert_eq!(name.symbol(), "R");
                        assert_eq!((name.fg, name.bg), (cell.fg, cell.bg));
                        assert_eq!(name.modifier, cell.modifier);
                        let arrow = &buffer[(header.right() - 2, header.y)];
                        assert_eq!(
                            arrow.symbol(),
                            match (collapsed, pointer) {
                                (true, _) => theme.glyphs.disclosure_closed,
                                (false, ProjectedPointer::Rest) => " ",
                                (false, _) => theme.glyphs.disclosure_open,
                            },
                            "the arrow keeps its cell and appears on hover or collapse"
                        );
                        if pointer == ProjectedPointer::Rest {
                            if let Some(resting) = &resting {
                                assert_eq!(
                                    &buffer, resting,
                                    "leaving the header restores the chip and context"
                                );
                            } else {
                                resting = Some(buffer.clone());
                            }
                        }
                    }
                }
            }
        }
    }
}
