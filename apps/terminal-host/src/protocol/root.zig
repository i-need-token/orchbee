const terminal_input = @import("terminal-input");
const contract = @import("contract.zig");
const envelope = @import("envelope.zig");
const frame_codec = @import("frame.zig");
const graphics_extension_codec = @import("graphics_extension.zig");
const input_codec = @import("input.zig");
const projection_buffer_codec = @import("projection_buffer.zig");
const row_window_codec = @import("row_window.zig");
const wire = @import("wire.zig");

pub const FocusInput = terminal_input.FocusInput;
pub const KeyAction = terminal_input.KeyAction;
pub const KeyInput = terminal_input.KeyInput;
pub const Modifiers = terminal_input.Modifiers;
pub const MouseAction = terminal_input.MouseAction;
pub const MouseButton = terminal_input.MouseButton;
pub const MouseButtons = terminal_input.MouseButtons;
pub const MouseInput = terminal_input.MouseInput;
pub const MouseMotion = terminal_input.MouseMotion;
pub const PasteAutoInput = terminal_input.PasteAutoInput;
pub const PromptLineInput = terminal_input.PromptLineInput;
pub const PasteInput = terminal_input.PasteInput;
pub const SemanticInput = terminal_input.SemanticInput;
pub const TextInput = terminal_input.TextInput;

pub const terminal_frame_header_bytes = contract.terminal_frame_header_bytes;
pub const terminal_frame_magic = contract.terminal_frame_magic;
pub const terminal_frame_format_major = contract.terminal_frame_format_major;
pub const terminal_frame_format_minor = contract.terminal_frame_format_minor;
pub const terminal_frame_format_patch = contract.terminal_frame_format_patch;
pub const terminal_frame_extension_kitty_graphics = contract.terminal_frame_extension_kitty_graphics;
pub const terminal_frame_extension_progress = contract.terminal_frame_extension_progress;
pub const terminal_frame_extension_command_completion = contract.terminal_frame_extension_command_completion;
pub const terminal_frame_extension_command_observation = contract.terminal_frame_extension_command_observation;
pub const terminal_frame_extension_command_start = contract.terminal_frame_extension_command_start;
pub const terminal_frame_extension_program_status = contract.terminal_frame_extension_program_status;
pub const terminal_graphics_header_bytes = contract.terminal_graphics_header_bytes;
pub const terminal_graphics_magic = contract.terminal_graphics_magic;
pub const terminal_graphics_format_major = contract.terminal_graphics_format_major;
pub const terminal_graphics_format_minor = contract.terminal_graphics_format_minor;
pub const terminal_graphics_format_patch = contract.terminal_graphics_format_patch;
pub const TerminalProgress = contract.TerminalProgress;
pub const TerminalProgressState = contract.TerminalProgressState;
pub const TerminalCommandCompletion = contract.TerminalCommandCompletion;
pub const TerminalCommandObservation = contract.TerminalCommandObservation;
pub const TerminalProgramStatus = contract.TerminalProgramStatus;
pub const TerminalProgramStatusState = contract.TerminalProgramStatusState;
pub const terminal_projection_buffer_header_bytes = contract.terminal_projection_buffer_header_bytes;
pub const terminal_projection_buffer_magic = contract.terminal_projection_buffer_magic;
pub const terminal_projection_buffer_format_major = contract.terminal_projection_buffer_format_major;
pub const terminal_projection_buffer_format_minor = contract.terminal_projection_buffer_format_minor;
pub const terminal_projection_buffer_format_patch = contract.terminal_projection_buffer_format_patch;
pub const terminal_row_window_header_bytes = contract.terminal_row_window_header_bytes;
pub const terminal_row_window_magic = contract.terminal_row_window_magic;
pub const terminal_row_window_format_major = contract.terminal_row_window_format_major;
pub const terminal_row_window_format_minor = contract.terminal_row_window_format_minor;
pub const terminal_row_window_format_patch = contract.terminal_row_window_format_patch;
pub const transport_envelope_header_bytes = contract.transport_envelope_header_bytes;
pub const current_protocol_major = contract.current_protocol_major;
pub const current_protocol_minor = contract.current_protocol_minor;
pub const current_protocol_patch = contract.current_protocol_patch;
pub const Limits = contract.Limits;
pub const MessageKind = contract.MessageKind;

pub const ProtocolError = contract.ProtocolError;

pub const FrameKind = frame_codec.FrameKind;
pub const CursorShape = frame_codec.CursorShape;
pub const ActiveScreen = frame_codec.ActiveScreen;

pub const RowWindowDetail = row_window_codec.RowWindowDetail;
pub const RowWindowSummary = row_window_codec.RowWindowSummary;

pub const FrameSummary = frame_codec.FrameSummary;
pub const TerminalGraphicsSummary = graphics_extension_codec.Summary;
pub const TerminalGraphicsUpdateKind = graphics_extension_codec.UpdateKind;
pub const ProjectionBufferSummary = projection_buffer_codec.ProjectionBufferSummary;

pub const EnvelopeSummary = envelope.Summary;
pub const PayloadEncoding = contract.PayloadEncoding;
pub const RequestOperation = contract.RequestOperation;
pub const RequestMessageContract = contract.RequestMessageContract;
pub const MessageContract = contract.MessageContract;
pub const messageContract = contract.messageContract;
pub const messagePayloadEncoding = contract.messagePayloadEncoding;

pub const WriteInputSummary = input_codec.WriteInputSummary;
pub const DecodedSemanticInput = input_codec.DecodedSemanticInput;
pub const validateWriteInput = input_codec.validateWriteInput;
pub const validateKeyInput = input_codec.validateKeyInput;
pub const validateCommittedTextInput = input_codec.validateCommittedTextInput;
pub const validatePasteAutoInput = input_codec.validatePasteAutoInput;
pub const validatePromptLineInput = input_codec.validatePromptLineInput;
pub const validatePasteInput = input_codec.validatePasteInput;
pub const validateMouseInput = input_codec.validateMouseInput;
pub const validateFocusInput = input_codec.validateFocusInput;
pub const validateSemanticInput = input_codec.validateSemanticInput;

const Reader = wire.Reader;
const optionalIdentifier = wire.optionalIdentifier;

pub const isKnownMessageKind = contract.isKnownMessageKind;

pub const validateTerminalFrame = frame_codec.validateTerminalFrame;
pub const validateTerminalGraphics = graphics_extension_codec.validate;
pub const validateTerminalProjectionBuffer = projection_buffer_codec.validateTerminalProjectionBuffer;

pub const validateTerminalRowWindow = row_window_codec.validateTerminalRowWindow;

pub const validateTransportEnvelope = envelope.validateTransportEnvelope;
pub const transportRecordLength = envelope.transportRecordLength;
pub const encodeTransportEnvelope = envelope.encodeTransportEnvelope;
pub const transportEnvelopeLength = envelope.transportEnvelopeLength;
pub const transportEnvelopeAllocationLength = envelope.transportEnvelopeAllocationLength;
pub const finalizeTransportEnvelope = envelope.finalizeTransportEnvelope;

pub const testing = if (@import("builtin").is_test) struct {
    pub const optionalIdentifierFn = optionalIdentifier;
    pub const ReaderType = Reader;
} else struct {};

pub const color_default = contract.color_default;
pub const color_none = contract.color_none;
pub const color_palette = contract.color_palette;
pub const color_rgb_mask = contract.color_rgb_mask;
