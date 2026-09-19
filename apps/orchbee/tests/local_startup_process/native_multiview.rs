use super::native_context::WorkbenchPeer;
use super::*;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};
use orchbee_workbench_protocol::{
    PanelNode, TerminalId, WorkbenchAction, WorkbenchCommandOutcome, WorkbenchLabel,
    WorkbenchPanelId, WorkbenchPanelSplitDirection, WorkbenchTabId, WorkbenchTerminalContextId,
    WorkbenchTerminalGeometry,
};
use rustix::process::{Pid, PidfdFlags, pidfd_open};
use std::os::fd::AsFd;

// One complete four-leaf observation covers real shell launches, split
// publications, focus routing and seconds of live tick output.
const SETTLE_TIMEOUT: Duration = Duration::from_secs(60);

// Build order fixes the 2x2 quadrants: A is the original leaf, B splits right
// of A, C splits below B and D splits below A.
const TAGS: [char; 4] = ['A', 'B', 'C', 'D'];
const LABELS: [&str; 4] = ["1", "MVB", "MVC", "MVD"];
const TICK_PREFIX: &str = "O4M_";

fn leaf_command(fixture: &Fixture, tag: char) -> String {
    // printf assembles each tick from fragments, so the echoed command line can
    // never satisfy a `O4M_<tag>_<counter>` assertion.
    format!(
        "printf '%s' \"$$\" > {0}; mv4_i=0; while :; do mv4_i=$((mv4_i + 1)); printf 'O4M%s%s%s%s\\n' '_' '{tag}' '_' \"$mv4_i\"; sleep 1; done\r",
        fixture.base.join(format!("mv4-pid-{tag}")).display()
    )
}

fn wait_leaf_pid(
    fixture: &Fixture,
    client: &mut Client,
    tag: char,
    deadline: Instant,
) -> Result<u32, Box<dyn Error>> {
    let value = wait_file(
        &fixture.base.join(format!("mv4-pid-{tag}")),
        &mut [&mut *client],
        deadline,
    );
    Ok(value.trim().parse::<u32>()?)
}

fn split_panel(
    peer: &mut WorkbenchPeer,
    context: &WorkbenchTerminalContextId,
    tab: &WorkbenchTabId,
    panel: &WorkbenchPanelId,
    direction: WorkbenchPanelSplitDirection,
    label: &'static str,
    deadline: Instant,
) -> Result<WorkbenchPanelId, Box<dyn Error>> {
    match peer.prepare(
        WorkbenchAction::PanelSplit {
            terminal_context_id: context.clone(),
            tab_id: tab.clone(),
            panel_id: panel.clone(),
            direction,
            label: WorkbenchLabel::parse(label)?,
            geometry: WorkbenchTerminalGeometry::new(80, 24, 8, 16)?,
        },
        deadline,
    )? {
        WorkbenchCommandOutcome::PanelSplitCreated { new_panel_id, .. } => Ok(new_panel_id),
        outcome => Err(format!("PanelSplit {label} was not created: {outcome:?}").into()),
    }
}

// Focus and input admission are only real once a typed probe echoes inside
// the target leaf's own bounds in a completed draw — the whole path (chord →
// focus → attach → controller → pty echo → render) in one draw fact. The
// chord and probe are each sent once after the leaf is bound and live; `^U`
// clears the probed line so the marker command stays a single clean send.
fn focus_leaf(client: &mut Client, chord: &[u8], label: &str, tag: char, deadline: Instant) {
    let probe = format!("ZZZ{tag}");
    let checkpoint = client.checkpoint();
    client.send(chord);
    client.send(probe.as_bytes());
    client.wait_completed_frame_after(
        checkpoint,
        deadline,
        &format!("input probe {probe} echo inside leaf {label}"),
        |frame| probe_in_leaf(frame.cells(), label, &probe).then_some(()),
    );
    client.send(b"\x15");
}

// The echo is a leaf-region witness: the probe text must appear below the
// leaf's own title band and between its separators.
fn probe_in_leaf(cells: &[Vec<String>], label: &str, probe: &str) -> bool {
    let Some((title_row, left, right)) = leaf_region(cells, label) else {
        return false;
    };
    cells[title_row + 1..]
        .iter()
        .take_while(|row| row.get(left).map(String::as_str) == Some("│"))
        .any(|row| row[left + 1..right.min(row.len())].concat().contains(probe))
}

// The leaf's own geometry: its title row plus the `│` separators bounding it.
// The label is matched out of the bounded title band — a sibling title on the
// same row cannot satisfy it, and a prefix like `· 1` cannot alias `· 10`.
fn leaf_region(cells: &[Vec<String>], label: &str) -> Option<(usize, usize, usize)> {
    positions_of(cells, "Panel · Terminal ")
        .iter()
        .find_map(|&(row, column)| {
            let (left, right) = leaf_bounds(cells, row, column)?;
            let title = cells[row][column..right.min(cells[row].len())].concat();
            (leaf_title_label(&title) == Some(label)).then_some((row, left, right))
        })
}

// The leaf's own label: the whole second `·` section of its title band,
// trimmed — multi-word labels like "Split peer" stay intact.
fn leaf_title_label(title: &str) -> Option<&str> {
    let (_, label) = title.strip_prefix("Panel · Terminal ")?.split_once('·')?;
    let label = label.trim();
    (!label.is_empty()).then_some(label)
}

fn leaf_bounds(
    cells: &[Vec<String>],
    title_row: usize,
    title_col: usize,
) -> Option<(usize, usize)> {
    let row = &cells[title_row];
    let left = row[..title_col].iter().rposition(|cell| cell == "│")?;
    let right = row[title_col..]
        .iter()
        .position(|cell| cell == "│")
        .map(|offset| title_col + offset)?;
    Some((left, right))
}

// A leaf is safe to type into only once its viewer is attached: input sent
// earlier is consumed without replay, so the send gate is the leaf's live
// shell content, not just its projected title.
pub(super) fn wait_leaf_live(client: &mut Client, label: &str, deadline: Instant) {
    let checkpoint = client.checkpoint();
    client.wait_completed_frame_after(
        checkpoint,
        deadline,
        "attached leaf with live shell",
        |frame| {
            let cells = frame.cells();
            leaf_region(cells, label)
                .filter(|&(title_row, left, right)| leaf_has_content(cells, title_row, left, right))
                .map(|_| ())
        },
    );
}

// The title band shares one row with `│` leaf separators; the leaf's interior
// runs below it until its bottom border or the frame edge. A live attached
// shell is witnessed by its prompt (`$` or `#`), not by frame structure.
fn leaf_has_content(cells: &[Vec<String>], title_row: usize, left: usize, right: usize) -> bool {
    for interior in &cells[title_row + 1..] {
        if interior.get(left).map(String::as_str) != Some("│") {
            break;
        }
        if interior[left + 1..right.min(interior.len())]
            .iter()
            .any(|cell| cell == "$" || cell == "#")
        {
            return true;
        }
    }
    false
}

fn cell_bounds(row: &[String]) -> Vec<usize> {
    let mut bounds = Vec::with_capacity(row.len() + 1);
    bounds.push(0);
    for cell in row {
        bounds.push(bounds.last().copied().unwrap_or_default() + cell.len());
    }
    bounds
}

fn positions_of(cells: &[Vec<String>], needle: &str) -> Vec<(usize, usize)> {
    let mut hits = Vec::new();
    for (top, row) in cells.iter().enumerate() {
        let joined = row.concat();
        let bounds = cell_bounds(row);
        let mut from = 0;
        while let Some(offset) = joined[from..].find(needle) {
            let byte = from + offset;
            if let Ok(left) = bounds.binary_search(&byte) {
                hits.push((top, left));
            }
            // Advance past the whole match: `byte + 1` can slice inside a
            // multibyte needle start (e.g. `·`) and panic on the next find.
            from = byte + needle.len();
        }
    }
    hits
}

fn leaf_ticks(cells: &[Vec<String>], tag: char) -> Vec<(usize, usize, u64)> {
    let prefix = format!("{TICK_PREFIX}{tag}_");
    let mut ticks = Vec::new();
    for (top, row) in cells.iter().enumerate() {
        let joined = row.concat();
        let bounds = cell_bounds(row);
        let mut from = 0;
        while let Some(offset) = joined[from..].find(&prefix) {
            let byte = from + offset;
            let digits: String = joined[byte + prefix.len()..]
                .chars()
                .take_while(|character| character.is_ascii_digit())
                .collect();
            if !digits.is_empty()
                && let (Ok(value), Ok(left)) = (digits.parse::<u64>(), bounds.binary_search(&byte))
            {
                ticks.push((top, left, value));
            }
            from = byte + prefix.len();
        }
    }
    ticks
}

// Leaf titles read `Panel · Terminal <n> · <label>`; the outer frame title is
// just `Panel · Terminal`, so only the second `·` section identifies a leaf.
fn leaf_index_of_title(title: &str) -> Option<usize> {
    let rest = title.strip_prefix("Panel · Terminal ")?;
    let (_, label) = rest.split_once('·')?;
    match label.split_whitespace().next()? {
        "1" => Some(0),
        "MVB" => Some(1),
        "MVC" => Some(2),
        "MVD" => Some(3),
        _ => None,
    }
}

// A tick inside a leaf's quadrant is the region witness; a tick anywhere else
// is cross-pane bleed and fails the draw.
fn in_leaf_region(index: usize, row: usize, column: usize, mid_row: usize, mid_col: usize) -> bool {
    match index {
        0 => row < mid_row && column < mid_col,
        1 => row < mid_row && column + 1 >= mid_col,
        2 => row > mid_row && column + 1 >= mid_col,
        3 => row > mid_row && column < mid_col,
        _ => unreachable!(),
    }
}

struct Marked {
    maxima: [u64; 4],
    titles: [(usize, usize); 4],
}

fn marked_frame(cells: &[Vec<String>]) -> Option<Marked> {
    let mut titles = [None; 4];
    for (row, column) in positions_of(cells, "Panel · Terminal ") {
        let Some(index) = leaf_index_of_title(&cells[row][column..].concat()) else {
            continue;
        };
        if titles[index].replace((row, column)).is_some() {
            return None;
        }
    }
    let titles = titles.into_iter().collect::<Option<Vec<_>>>()?;
    let titles: [(usize, usize); 4] = titles.try_into().ok()?;
    // Top titles sit strictly above bottom titles; left titles sit strictly
    // left of right titles. Anything else is not the expected 2x2 projection.
    let sane = titles[0].0 < titles[2].0
        && titles[0].0 < titles[3].0
        && titles[1].0 < titles[2].0
        && titles[1].0 < titles[3].0
        && titles[0].1 < titles[1].1
        && titles[0].1 < titles[2].1
        && titles[3].1 < titles[1].1
        && titles[3].1 < titles[2].1;
    if !sane {
        return None;
    }
    let mid_row = titles[2].0.min(titles[3].0);
    let mid_col = titles[1].1.min(titles[2].1);
    let mut maxima = [0u64; 4];
    for (index, tag) in TAGS.iter().enumerate() {
        for (row, column, value) in leaf_ticks(cells, *tag) {
            if !in_leaf_region(index, row, column, mid_row, mid_col) {
                return None;
            }
            maxima[index] = maxima[index].max(value);
        }
        if maxima[index] == 0 {
            return None;
        }
    }
    Some(Marked { maxima, titles })
}

// After C closes the right split collapses: B takes the full right column while
// A stays over D on the left.
fn surviving_frame(cells: &[Vec<String>]) -> Option<[u64; 3]> {
    if !leaf_ticks(cells, TAGS[2]).is_empty() || !positions_of(cells, "· MVC").is_empty() {
        return None;
    }
    let mut titles = [None; 3];
    for (row, column) in positions_of(cells, "Panel · Terminal ") {
        let Some(index) = leaf_index_of_title(&cells[row][column..].concat()) else {
            continue;
        };
        if index == 2 {
            return None;
        }
        let slot = if index == 3 { 2 } else { index };
        if titles[slot].replace((row, column)).is_some() {
            return None;
        }
    }
    let [a, b, d] = titles;
    let titles = [a?, b?, d?];
    let mid_col = titles[1].1;
    let mid_row = titles[2].0;
    if !(titles[0].0 < mid_row && titles[0].1 < mid_col) {
        return None;
    }
    let mut maxima = [0u64; 3];
    for (slot, index) in [0usize, 1, 3].into_iter().enumerate() {
        for (row, column, value) in leaf_ticks(cells, TAGS[index]) {
            let inside = match index {
                0 => row < mid_row && column < mid_col,
                1 => column + 1 >= mid_col,
                3 => row > mid_row && column < mid_col,
                _ => unreachable!(),
            };
            if !inside {
                return None;
            }
            maxima[slot] = maxima[slot].max(value);
        }
        if maxima[slot] == 0 {
            return None;
        }
    }
    Some(maxima)
}

type BoundLeaf = (String, WorkbenchPanelId, TerminalId);

fn collect_bound_leaves(node: &PanelNode, leaves: &mut Vec<BoundLeaf>) {
    match node {
        PanelNode::Panel {
            panel_id,
            primary_presentation,
            ..
        } => {
            if let Some(binding) = &primary_presentation.binding {
                leaves.push((
                    primary_presentation.label.clone(),
                    panel_id.clone(),
                    binding.terminal_id.clone(),
                ));
            }
        }
        PanelNode::Split { children, .. } => {
            for slot in children.iter() {
                collect_bound_leaves(&slot.node, leaves);
            }
        }
    }
}

fn tab_leaves(
    peer: &mut WorkbenchPeer,
    context: &WorkbenchTerminalContextId,
    deadline: Instant,
) -> Result<Vec<BoundLeaf>, Box<dyn Error>> {
    let snapshot = peer.snapshot(deadline)?;
    let tab = snapshot
        .terminal_contexts
        .iter()
        .find(|candidate| candidate.terminal_context_id == *context)
        .and_then(|candidate| candidate.tabs.first())
        .ok_or("multiview fixture Tab missing")?;
    let mut leaves = Vec::new();
    collect_bound_leaves(&tab.root, &mut leaves);
    Ok(leaves)
}

fn pidfd(pid: u32) -> Result<rustix::fd::OwnedFd, Box<dyn Error>> {
    Ok(pidfd_open(
        Pid::from_raw(i32::try_from(pid)?).ok_or("invalid leaf shell PID")?,
        PidfdFlags::empty(),
    )?)
}

fn shell_alive(descriptor: &rustix::fd::OwnedFd) -> Result<bool, Box<dyn Error>> {
    let mut pollable = [PollFd::new(descriptor.as_fd(), PollFlags::POLLIN)];
    poll(&mut pollable, PollTimeout::ZERO)?;
    Ok(!pollable[0]
        .revents()
        .is_some_and(|flags| flags.contains(PollFlags::POLLIN)))
}

fn two_by_two_witness(zero_pixels: bool) -> TestResult {
    let _process_witness = process_witness_guard();
    let fixture = Fixture::new();
    let mut client = if zero_pixels {
        Client::spawn_zeropixel(&fixture, "en_US.UTF-8")
    } else {
        Client::spawn(&fixture, "en_US.UTF-8")
    };
    client.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::Controlling,
    );
    let deadline = Instant::now() + SETTLE_TIMEOUT;
    let mut peer = WorkbenchPeer::connect(&fixture, deadline)?;
    let snapshot = peer.snapshot(deadline)?;
    let context_id = snapshot.navigation.standalone_terminal_context_id.clone();
    let tab = snapshot
        .terminal_contexts
        .iter()
        .find(|candidate| candidate.terminal_context_id == context_id)
        .and_then(|candidate| candidate.tabs.first())
        .ok_or("multiview fixture Tab missing")?;
    let tab_id = tab.tab_id.clone();
    let PanelNode::Panel {
        panel_id: panel_a, ..
    } = &tab.root
    else {
        return Err("fixture Tab was already split".into());
    };
    let panel_a = panel_a.clone();

    // Leaf A already holds focus; its identity write proves the first binding.
    client.send(leaf_command(&fixture, TAGS[0]).as_bytes());
    let mut pids = [0u32; 4];
    pids[0] = wait_leaf_pid(&fixture, &mut client, TAGS[0], deadline)?;

    let panel_b = split_panel(
        &mut peer,
        &context_id,
        &tab_id,
        &panel_a,
        WorkbenchPanelSplitDirection::Right,
        LABELS[1],
        deadline,
    )?;
    wait_leaf_live(&mut client, LABELS[1], deadline);
    focus_leaf(&mut client, b"\x02l", LABELS[1], TAGS[1], deadline);
    client.send(leaf_command(&fixture, TAGS[1]).as_bytes());
    pids[1] = wait_leaf_pid(&fixture, &mut client, TAGS[1], deadline)?;

    let panel_c = split_panel(
        &mut peer,
        &context_id,
        &tab_id,
        &panel_b,
        WorkbenchPanelSplitDirection::Below,
        LABELS[2],
        deadline,
    )?;
    wait_leaf_live(&mut client, LABELS[2], deadline);
    focus_leaf(&mut client, b"\x02j", LABELS[2], TAGS[2], deadline);
    client.send(leaf_command(&fixture, TAGS[2]).as_bytes());
    pids[2] = wait_leaf_pid(&fixture, &mut client, TAGS[2], deadline)?;

    let panel_d = split_panel(
        &mut peer,
        &context_id,
        &tab_id,
        &panel_a,
        WorkbenchPanelSplitDirection::Below,
        LABELS[3],
        deadline,
    )?;
    wait_leaf_live(&mut client, LABELS[3], deadline);
    focus_leaf(&mut client, b"\x02h", LABELS[3], TAGS[3], deadline);
    client.send(leaf_command(&fixture, TAGS[3]).as_bytes());
    pids[3] = wait_leaf_pid(&fixture, &mut client, TAGS[3], deadline)?;

    assert!(
        pids.iter().enumerate().all(|(index, pid)| {
            *pid != 0 && pids.iter().skip(index + 1).all(|other| other != pid)
        }),
        "leaf shells are not distinct: {pids:?}"
    );
    let pidfds = pids
        .iter()
        .map(|pid| pidfd(*pid))
        .collect::<Result<Vec<_>, _>>()?;

    // The shared snapshot must report four leaves with four distinct terminals —
    // a projection that copied one frame into four slots cannot satisfy this.
    let mut bound = Vec::new();
    while Instant::now() < deadline {
        client.collect();
        bound = tab_leaves(&mut peer, &context_id, deadline)?;
        if bound.len() == 4 {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }
    assert_eq!(bound.len(), 4, "four bound leaves never published");
    for (expected, tag) in LABELS.iter().zip(TAGS) {
        assert!(
            bound.iter().any(|(label, _, _)| label == expected),
            "leaf {tag} label {expected:?} missing from snapshot"
        );
    }
    for (panel, tag) in [
        (&panel_a, 'A'),
        (&panel_b, 'B'),
        (&panel_c, 'C'),
        (&panel_d, 'D'),
    ] {
        assert!(
            bound.iter().any(|(_, candidate, _)| candidate == panel),
            "leaf {tag} panel missing from snapshot"
        );
    }
    assert!(
        bound.iter().enumerate().all(|(index, (_, _, terminal))| {
            bound
                .iter()
                .skip(index + 1)
                .all(|(_, _, other)| other != terminal)
        }),
        "leaf terminals are not distinct: {bound:?}"
    );

    // Two complete draws: every leaf region holds its own marker and every
    // counter strictly advances between them.
    let checkpoint = client.checkpoint();
    let first = client.wait_completed_frame_after(
        checkpoint,
        deadline,
        "four live leaves in one draw",
        |frame| marked_frame(frame.cells()),
    );
    let checkpoint = client.checkpoint();
    let second = client.wait_completed_frame_after(
        checkpoint,
        deadline,
        "all four leaf counters advance",
        |frame| {
            marked_frame(frame.cells()).filter(|marked| {
                marked
                    .maxima
                    .iter()
                    .zip(first.maxima.iter())
                    .all(|(now, before)| now > before)
            })
        },
    );

    // Focus returns to A; the probe echo proves input routing while every
    // leaf keeps ticking across the move.
    focus_leaf(&mut client, b"\x02k", LABELS[0], TAGS[0], deadline);
    let checkpoint = client.checkpoint();
    client.wait_completed_frame_after(
        checkpoint,
        deadline,
        "all four leaves tick across the focus return",
        |frame| {
            marked_frame(frame.cells()).filter(|marked| {
                marked
                    .maxima
                    .iter()
                    .zip(second.maxima.iter())
                    .all(|(now, before)| now > before)
            })
        },
    );

    // A viewport resize must re-bound every leaf correctly.
    let checkpoint = client.checkpoint();
    client.resize_rows(24);
    client.wait_resized_ready_after(checkpoint, deadline, 24, 1);
    let checkpoint = client.checkpoint();
    let resized = client.wait_completed_frame_after(
        checkpoint,
        deadline,
        "resized four-leaf draw",
        |frame| {
            marked_frame(frame.cells()).filter(|marked| {
                marked
                    .maxima
                    .iter()
                    .zip(second.maxima.iter())
                    .all(|(now, before)| now > before)
            })
        },
    );
    assert!(
        resized.titles[2].0 < second.titles[2].0 && resized.titles[3].0 < second.titles[3].0,
        "bottom leaves did not move up after the viewport shrank"
    );

    // Exit and re-entry: the running shells and the 2x2 layout are retained by
    // the site, and a fresh Client reattaches all four leaves.
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    let mut client = if zero_pixels {
        Client::spawn_zeropixel(&fixture, "en_US.UTF-8")
    } else {
        Client::spawn(&fixture, "en_US.UTF-8")
    };
    client.wait_ready_after(
        0,
        Instant::now() + STARTUP_OR_CREATE_TIMEOUT,
        1,
        "1",
        ReadyMode::Controlling,
    );
    let deadline = Instant::now() + SETTLE_TIMEOUT;
    let reentered = client.wait_completed_frame_after(
        0,
        deadline,
        "four leaves preserved across re-entry",
        |frame| {
            marked_frame(frame.cells()).filter(|marked| {
                marked
                    .maxima
                    .iter()
                    .zip(resized.maxima.iter())
                    .all(|(now, before)| now > before)
            })
        },
    );

    // Closing one leaf retires only its shell; the other three keep running.
    match peer.prepare(
        WorkbenchAction::PanelClose {
            terminal_context_id: context_id.clone(),
            tab_id: tab_id.clone(),
            panel_id: panel_c.clone(),
        },
        deadline,
    )? {
        WorkbenchCommandOutcome::PanelCloseAccepted { .. } => {}
        outcome => return Err(format!("PanelClose was not accepted: {outcome:?}").into()),
    }
    let mut collapsed = false;
    while Instant::now() < deadline && !collapsed {
        client.collect();
        let leaves = tab_leaves(&mut peer, &context_id, deadline)?;
        collapsed = leaves.len() == 3 && leaves.iter().all(|(_, panel, _)| *panel != panel_c);
        if !collapsed {
            thread::sleep(Duration::from_millis(50));
        }
    }
    assert!(collapsed, "closed leaf never left the shared snapshot");

    let checkpoint = client.checkpoint();
    client.wait_completed_frame_after(
        checkpoint,
        deadline,
        "three surviving leaves still tick",
        |frame| {
            surviving_frame(frame.cells()).filter(|maxima| {
                let before = [
                    reentered.maxima[0],
                    reentered.maxima[1],
                    reentered.maxima[3],
                ];
                maxima.iter().zip(before.iter()).all(|(now, was)| now > was)
            })
        },
    );
    let mut gone = false;
    while Instant::now() < deadline && !gone {
        let mut pollable = [PollFd::new(pidfds[2].as_fd(), PollFlags::POLLIN)];
        poll(&mut pollable, PollTimeout::from(100u16))?;
        gone = pollable[0]
            .revents()
            .is_some_and(|flags| flags.contains(PollFlags::POLLIN));
        client.collect();
    }
    assert!(gone, "closed leaf shell did not exit");
    for (index, descriptor) in pidfds.iter().enumerate() {
        if index == 2 {
            continue;
        }
        assert!(
            shell_alive(descriptor)?,
            "surviving leaf {index} shell exited"
        );
    }
    client.close_before(Instant::now() + EXIT_TIMEOUT);
    Ok(())
}

#[test]
fn native_multiview_two_by_two_leaves_run_distinct_live_shells() -> TestResult {
    two_by_two_witness(false)
}

#[test]
fn native_multiview_two_by_two_leaves_run_under_zero_pixel_geometry() -> TestResult {
    two_by_two_witness(true)
}
