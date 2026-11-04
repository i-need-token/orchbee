export {
	ApplyTerminalThemeSchema,
	AttachmentAcknowledgedSchema,
	AttachmentCapabilitiesSchema,
	CanonicalUint64TextSchema,
	ClearTerminalSchema,
	ClaimResizeLeaseSchema,
	decodeControlJson,
	encodeControlJson,
	EmptyControlSchema,
	FrameAppliedSchema,
	HelloAcceptedSchema,
	HistoryChangedSchema,
	HistoryDemandAcceptedSchema,
	HistoryObservedSchema,
	InputAcceptedSchema,
	ListTerminalsSchema,
	NeedSnapshotSchema,
	ProjectionBlockedSchema,
	ProjectionDemandAcceptedSchema,
	ProtocolRejectedSchema,
	ReadRowsSchema,
	ResizeLeaseChangedSchema,
	ResizeTerminalSchema,
	ResizeAcceptedSchema,
	ResetTerminalSchema,
	RowsAppliedSchema,
	SelectionChangedSchema,
	SelectionCommandSchema,
	SelectionExportSchema,
	SelectionExportFinishedSchema,
	SelectionExportStartedSchema,
	SelectionGestureSchema,
	SelectionPointSchema,
	SetHistoryDemandSchema,
	SetProjectionDemandSchema,
	SetTerminalDefaultsSchema,
	TerminalAcknowledgedSchema,
	TerminalApplicationSignalSchema,
	TerminalBellSchema,
	TerminalClipboardWriteRequestSchema,
	TerminalExitedSchema,
	TerminalFailedSchema,
	TerminalGeometrySchema,
	TerminalIdentitySchema,
	TerminalConfigurationAcceptedSchema,
	TerminalControlAcceptedSchema,
	TerminalDefaultsAcceptedSchema,
	TerminalListSchema,
	TerminalStartAcceptedSchema,
	TerminalStartedSchema,
	TerminalTargetSchema,
	TerminalThemeResultSchema,
	TerminalThemeSchema,
	TerminalThemeStateSchema,
	TypedFailureSchema,
	UpdateAttachmentInputCapabilitySchema,
} from './control';
export type {
	CanonicalUint64Text,
	ClearTerminal,
	FrameApplied,
	HistoryChanged,
	HistoryObserved,
	NeedSnapshot,
	ProjectionDemand,
	ReadRows,
	ResizeTerminal,
	ResetTerminal,
	RowsApplied,
	RowRevisionPolicy,
	SelectionChanged,
	SelectionCommand,
	SelectionExportCancel,
	SelectionGesture,
	SelectionPoint,
	SelectionViewportPoint,
	SetHistoryDemand,
	SetProjectionDemand,
	TerminalBell,
	TerminalClipboardWriteRequest,
	TerminalExited,
	TerminalFailed,
	TerminalGeometry,
	TerminalConfiguration,
	TerminalThemeState,
} from './control';
export { encodeControlRequest } from './encodeControlRequest';
export {
	decodeProtocolEnvelope,
	encodeProtocolEnvelope,
	MessageKind,
	PayloadEncoding,
} from './envelope';
export type { DecodedProtocolEnvelope } from './envelope';
export {
	ActiveScreen,
	CellWidth,
	CursorShape,
	decodeTerminalFrame,
	encodeTerminalFrame,
	inspectTerminalFrameHeader,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalColorEncoding,
	TerminalCellWidthCode,
	TerminalFrameKind,
	TerminalRowFlag,
	TerminalRowPromptClassCode,
	TerminalSemanticContentCode,
	TerminalStyleFlag,
	TerminalUnderlineKindCode,
	UnderlineKind,
} from './frame';
export type {
	ActiveScreen as ActiveScreenValue,
	CursorShape as CursorShapeValue,
	DecodedTerminalFrame,
	InspectedTerminalFrameHeader,
	TerminalFrame,
	TerminalInteraction,
	TerminalProgress,
	TerminalCommandCompletion,
} from './frame';
export {
	AttachmentIdSchema,
	CorrelationIdSchema,
	CreationIdSchema,
	HostEpochSchema,
	TerminalIdSchema,
	TerminalThemeOperationIdSchema,
} from './identifiers';
export type { AttachmentId, CorrelationId, CreationId, HostEpoch, TerminalId } from './identifiers';
export {
	decodeKeyInput,
	decodePasteInput,
	decodeTextInput,
	encodeInputRequest,
	FocusInputSchema,
	KeyAction,
	KeyInputSchema,
	KEY_INPUT_HEADER_BYTES,
	MouseAction,
	MouseButton,
	MouseInputSchema,
	PasteInputSchema,
	TerminalKeyCode,
	TextInputSchema,
} from './input';
export type {
	FocusInput,
	KeyInput,
	MouseInput,
	PasteInput,
	TerminalKeyCode as TerminalKeyCodeValue,
	TerminalModifiers,
	TextInput,
} from './input';
export {
	decodeTerminalRowWindow,
	encodeTerminalRowWindow,
	TERMINAL_ROW_WINDOW_FORMAT,
	TERMINAL_ROW_WINDOW_HEADER_BYTES,
	TerminalRowWindowDetail,
	viewTerminalRowWindow,
} from './row-window';
export type { DecodedTerminalRowWindow } from './row-window';
export { decodeSelectionExportChunk, encodeSelectionExportChunk } from './selection';
export type { SelectionExportChunk } from './selection';

export { PROTOCOL_LIMITS } from './limits';
export { ProtocolErrorCode } from './protocol-result';
export type { ProtocolFailure } from './protocol-result';
export { TerminalGraphicsUpdateKind } from './terminal-graphics';
/** @internal Fixture encoder for the `@orchbee/terminal` packed-store tests; production encodes graphics only inside this package. */
export { encodeTerminalGraphics } from './terminal-graphics';
export type {
	TerminalGraphics,
	TerminalGraphicsImage,
	TerminalGraphicsPlacement,
} from './terminal-graphics';
export { encodeTerminalProjectionBuffer, TerminalProjectionSection } from './projection-buffer';
/** @internal Fixture decoder for the `@orchbee/terminal` packed-store tests; production decodes a standalone buffer only inside this package. */
export { decodeTerminalProjectionBuffer } from './projection-buffer';
export type { DecodedTerminalProjectionBuffer } from './projection-buffer';
export { CURRENT_PROTOCOL_VERSION } from './version';
export { TerminalProcessInfoSchema } from './process-info';
