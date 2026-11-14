//! Workbench object options: labels, paths, directories, workspace and operation identities, close
//! fences, terminal geometry, and tab and panel movement.

use super::{
    ExpectedValue, FailureEnvelope, MutationCommand, Options, PanelMoveDestination, ParseFailure,
    Resource, UsageContext, UsageReason, missing_required, required, target, usage_with,
};
use orchbee_workbench_protocol::{
    WorkbenchDirectoryId, WorkbenchDirectoryPath, WorkbenchLabel, WorkbenchPanelSplitDirection,
    WorkbenchRequestId, WorkbenchTabCloseAttemptGeneration, WorkbenchTabInsertIndex,
    WorkbenchTabMoveDirection, WorkbenchTerminalGeometry,
};

/// `--label` carries a wire-ready Workbench label: non-empty and at most the
/// protocol's 512-scalar bound.
pub(super) fn label_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchLabel, ParseFailure> {
    let value = required(&options.label, context, "'--label'", envelope)?;
    WorkbenchLabel::parse(&value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--label",
                value,
                expected: ExpectedValue::Label,
            },
            envelope,
        )
    })
}

/// `--path` carries a wire-ready Workbench directory path: the literal
/// absolute local-platform path, at most the protocol's 4,096-byte bound. The CLI
/// never expands `~`/`$`, never creates the directory, and never probes a
/// filesystem — a malformed value is a usage failure at this boundary.
pub(super) fn path_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchDirectoryPath, ParseFailure> {
    let value = required(&options.path, context, "'--path'", envelope)?;
    WorkbenchDirectoryPath::parse(&value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--path",
                value,
                expected: ExpectedValue::DirectoryPath,
            },
            envelope,
        )
    })
}

/// `--directory` carries a wire-ready Workbench directory ID: the strict
/// nonzero 32-lowercase-hex selector the `worktree list` grammar admits. It
/// is an ID only — never a filesystem path or label — and membership stays
/// the Server's authority; a malformed value is a generic usage failure with
/// no resource details at this boundary.
pub(super) fn directory_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchDirectoryId, ParseFailure> {
    let value = required(&options.directory, context, "'--directory'", envelope)?;
    WorkbenchDirectoryId::parse(&value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--directory",
                value,
                expected: ExpectedValue::DirectoryId,
            },
            envelope,
        )
    })
}

/// `<family> resume <request-id> --generation <n>`: the explicit attempt fence —
/// the protocol's nonzero generation spelling; a stale-but-valid value is
/// the Server's settled `stale` answer, while a malformed value is usage.
pub(super) fn close_generation_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchTabCloseAttemptGeneration, ParseFailure> {
    let value = required(&options.generation, context, "'--generation'", envelope)?;
    WorkbenchTabCloseAttemptGeneration::parse(&value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--generation",
                value,
                expected: ExpectedValue::CloseGeneration,
            },
            envelope,
        )
    })
}

/// The four geometry flags are an all-or-none group. With all four absent the
/// command requests the documented default: an 80x24 logical grid with the
/// protocol's paired-zero (unknown) cell pixel dimensions — a CLI request
/// policy, never a probed display fact. With any flag present all four are
/// required; each parses as a u16 dimension and must satisfy the protocol
/// geometry bounds (columns/rows 1..=4096, cell pixel dimensions 0..=4096 and
/// both zero or both nonzero, grid ≤ 262144 cells, pixel extents ≤ 65535).
pub(super) fn geometry_option(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchTerminalGeometry, ParseFailure> {
    if options.cols.is_none()
        && options.rows.is_none()
        && options.cell_width.is_none()
        && options.cell_height.is_none()
    {
        return WorkbenchTerminalGeometry::new(80, 24, 0, 0).map_err(|_| {
            usage_with(
                UsageReason::InvalidGeometry {
                    context,
                    cols: 80,
                    rows: 24,
                    cell_width: 0,
                    cell_height: 0,
                },
                envelope,
            )
        });
    }
    let dimension = |slot: &Option<String>, flag: &'static str| -> Result<u16, ParseFailure> {
        let value = required(
            slot,
            context,
            match flag {
                "--cols" => "'--cols'",
                "--rows" => "'--rows'",
                "--cell-width" => "'--cell-width'",
                _ => "'--cell-height'",
            },
            envelope,
        )?;
        value.parse::<u16>().map_err(|_| {
            usage_with(
                UsageReason::InvalidValue {
                    context,
                    option: flag,
                    value,
                    expected: ExpectedValue::Dimension,
                },
                envelope,
            )
        })
    };
    let cols = dimension(&options.cols, "--cols")?;
    let rows = dimension(&options.rows, "--rows")?;
    let cell_width = dimension(&options.cell_width, "--cell-width")?;
    let cell_height = dimension(&options.cell_height, "--cell-height")?;
    WorkbenchTerminalGeometry::new(cols, rows, cell_width, cell_height).map_err(|_| {
        usage_with(
            UsageReason::InvalidGeometry {
                context,
                cols,
                rows,
                cell_width,
                cell_height,
            },
            envelope,
        )
    })
}

pub(super) fn manifest_operation_id(
    value: &str,
    context: UsageContext,
    option: &'static str,
    envelope: FailureEnvelope,
) -> Result<WorkbenchRequestId, ParseFailure> {
    WorkbenchRequestId::parse(value).map_err(|_| {
        usage_with(
            UsageReason::InvalidValue {
                context,
                option,
                value: value.into(),
                expected: ExpectedValue::ManifestOperationId,
            },
            envelope,
        )
    })
}

pub(super) fn tab_move_command(
    value: &str,
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<MutationCommand, ParseFailure> {
    let context = UsageContext::Path("tab move");
    if options.order.is_some() && options.insert_index.is_some() {
        return Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: "--order",
                second: "--insert-index",
            },
            envelope,
        ));
    }
    let target = target(value.to_owned(), Resource::Tab, envelope)?;
    if let Some(value) = &options.insert_index {
        let index = (!value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| value.parse::<usize>().ok())
            .flatten()
            .and_then(|value| WorkbenchTabInsertIndex::new(value).ok())
            .ok_or_else(|| {
                usage_with(
                    UsageReason::InvalidValue {
                        context,
                        option: "--insert-index",
                        value: value.clone(),
                        expected: ExpectedValue::TabInsertIndex,
                    },
                    envelope,
                )
            })?;
        return Ok(MutationCommand::TabMoveIndex {
            target,
            insert_index: index,
        });
    }
    if options.order.is_none() {
        return Err(missing_required(
            context,
            "'--order' or '--insert-index'",
            envelope,
        ));
    }
    Ok(MutationCommand::TabMove {
        target,
        direction: tab_direction(options, context, envelope)?,
    })
}

fn tab_direction(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchTabMoveDirection, ParseFailure> {
    let value = required(&options.order, context, "'--order'", envelope)?;
    match value.as_str() {
        "previous" => Ok(WorkbenchTabMoveDirection::Previous),
        "next" => Ok(WorkbenchTabMoveDirection::Next),
        "first" => Ok(WorkbenchTabMoveDirection::First),
        "last" => Ok(WorkbenchTabMoveDirection::Last),
        _ => Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--order",
                value,
                expected: ExpectedValue::TabDirection,
            },
            envelope,
        )),
    }
}

/// Existing-Tab placement always names `--into-tab`; `--target-panel` refines that destination.
pub(super) fn panel_move_destination(
    options: &Options,
    envelope: FailureEnvelope,
) -> Result<PanelMoveDestination, ParseFailure> {
    let context = UsageContext::Path("panel move");
    let declared = [
        options.into_tab.is_some().then_some("--into-tab"),
        options.new_tab.then_some("--new-tab"),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    match declared[..] {
        ["--into-tab"] => {
            if options.label.is_some() {
                return Err(usage_with(
                    UsageReason::ConflictingOptions {
                        context,
                        first: "--into-tab",
                        second: "--label",
                    },
                    envelope,
                ));
            }
            Ok(PanelMoveDestination::Tab {
                tab: target(
                    required(&options.into_tab, context, "'--into-tab'", envelope)?,
                    Resource::Tab,
                    envelope,
                )?,
                panel: options
                    .target_panel
                    .as_ref()
                    .map(|value| target(value.clone(), Resource::Panel, envelope))
                    .transpose()?,
                direction: panel_direction(options, context, envelope)?,
            })
        }
        ["--new-tab"] => {
            if options.target_panel.is_some() {
                return Err(usage_with(
                    UsageReason::ConflictingOptions {
                        context,
                        first: "--new-tab",
                        second: "--target-panel",
                    },
                    envelope,
                ));
            }
            if options.direction.is_some() {
                return Err(usage_with(
                    UsageReason::ConflictingOptions {
                        context,
                        first: "--new-tab",
                        second: "--direction",
                    },
                    envelope,
                ));
            }
            Ok(PanelMoveDestination::NewTab {
                label: label_option(options, context, envelope)?,
            })
        }
        [] => Err(missing_required(
            context,
            "'--into-tab' or '--new-tab'",
            envelope,
        )),
        _ => Err(usage_with(
            UsageReason::ConflictingOptions {
                context,
                first: declared[0],
                second: declared[1],
            },
            envelope,
        )),
    }
}

pub(super) fn panel_direction(
    options: &Options,
    context: UsageContext,
    envelope: FailureEnvelope,
) -> Result<WorkbenchPanelSplitDirection, ParseFailure> {
    let value = required(&options.direction, context, "'--direction'", envelope)?;
    match value.as_str() {
        "up" => Ok(WorkbenchPanelSplitDirection::Above),
        "down" => Ok(WorkbenchPanelSplitDirection::Below),
        "left" => Ok(WorkbenchPanelSplitDirection::Left),
        "right" => Ok(WorkbenchPanelSplitDirection::Right),
        _ => Err(usage_with(
            UsageReason::InvalidValue {
                context,
                option: "--direction",
                value,
                expected: ExpectedValue::PanelDirection,
            },
            envelope,
        )),
    }
}
