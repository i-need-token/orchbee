const common = @import("decode_common.zig");
const host = @import("decode_host.zig");
const interaction = @import("decode_interaction.zig");
const projection = @import("decode_projection.zig");
const selection = @import("decode_selection.zig");
const terminal = @import("decode_terminal.zig");
const theme = @import("decode_theme.zig");

pub const DecodeError = common.Error;
pub const Decoded = common.Decoded;
pub const parseCanonicalU64 = common.parseCanonicalU64;
pub const decodeEmpty = common.decodeEmpty;

pub const ProtocolVersion = host.ProtocolVersion;
pub const Hello = host.Hello;
pub const InventoryCursor = host.InventoryCursor;
pub const ListTerminals = host.ListTerminals;
pub const decodeHello = host.decodeHello;
pub const decodeListTerminals = host.decodeListTerminals;
pub const validHello = host.validHello;

pub const CreateTerminal = terminal.CreateTerminal;
pub const StartTerminal = terminal.StartTerminal;
pub const UpdateTerminalConfiguration = terminal.UpdateTerminalConfiguration;
pub const AttachTerminal = terminal.AttachTerminal;
pub const UpdateAttachmentInputCapability = terminal.UpdateAttachmentInputCapability;
pub const ClaimResizeLease = terminal.ClaimResizeLease;
pub const ResizeTerminal = terminal.ResizeTerminal;
pub const decodeCreateTerminal = terminal.decodeCreateTerminal;
pub const decodeStartTerminal = terminal.decodeStartTerminal;
pub const decodeSetTerminalDefaults = terminal.decodeSetTerminalDefaults;
pub const decodeUpdateTerminalConfiguration = terminal.decodeUpdateTerminalConfiguration;
pub const decodeTerminalTarget = terminal.decodeTerminalTarget;
pub const decodeAttachTerminal = terminal.decodeAttachTerminal;
pub const decodeDetachTerminal = terminal.decodeDetachTerminal;
pub const decodeUpdateAttachmentInputCapability =
    terminal.decodeUpdateAttachmentInputCapability;
pub const decodeClaimResizeLease = terminal.decodeClaimResizeLease;
pub const decodeTerminalControl = terminal.decodeTerminalControl;
pub const decodeResizeTerminal = terminal.decodeResizeTerminal;

pub const ApplyTerminalTheme = theme.ApplyTerminalTheme;
pub const decodeApplyTerminalTheme = theme.decodeApplyTerminalTheme;

pub const FrameApplied = projection.FrameApplied;
pub const SnapshotReason = projection.SnapshotReason;
pub const NeedSnapshot = projection.NeedSnapshot;
pub const ProjectionDemand = projection.ProjectionDemand;
pub const decodeFrameApplied = projection.decodeFrameApplied;
pub const decodeNeedSnapshot = projection.decodeNeedSnapshot;
pub const decodeProjectionDemand = projection.decodeProjectionDemand;

pub const ReadRows = interaction.ReadRows;
pub const RowsApplied = interaction.RowsApplied;
pub const HistoryDemand = interaction.HistoryDemand;
pub const HistoryObserved = interaction.HistoryObserved;
pub const SelectionGesture = selection.SelectionGesture;
pub const SelectionCommand = selection.SelectionCommand;
pub const SelectionExport = selection.SelectionExport;
pub const SelectionExportChunkApplied = selection.SelectionExportChunkApplied;
pub const ExportCancelReason = selection.ExportCancelReason;
pub const SelectionExportCancel = selection.SelectionExportCancel;
pub const decodeReadRows = interaction.decodeReadRows;
pub const decodeRowsApplied = interaction.decodeRowsApplied;
pub const decodeHistoryDemand = interaction.decodeHistoryDemand;
pub const decodeHistoryObserved = interaction.decodeHistoryObserved;
pub const decodeSelectionGesture = selection.decodeSelectionGesture;
pub const decodeSelectionCommand = selection.decodeSelectionCommand;
pub const decodeSelectionExport = selection.decodeSelectionExport;
pub const decodeSelectionExportChunkApplied = selection.decodeSelectionExportChunkApplied;
pub const decodeSelectionExportCancel = selection.decodeSelectionExportCancel;
pub const parseRowOrigin = interaction.parseRowOrigin;
