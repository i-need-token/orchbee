import {
	ApplyTerminalThemeSchema,
	AttachTerminalSchema,
	ClearTerminalSchema,
	ClaimResizeLeaseSchema,
	CreateTerminalSchema,
	DetachTerminalSchema,
	EmptyControlSchema,
	encodeControlJson,
	FrameAppliedSchema,
	HelloSchema,
	HistoryObservedSchema,
	ListTerminalsSchema,
	NeedSnapshotSchema,
	ReadRowsSchema,
	ResetTerminalSchema,
	ResizeTerminalSchema,
	RowsAppliedSchema,
	SelectionCommandSchema,
	SelectionExportCancelSchema,
	SelectionExportChunkAppliedSchema,
	SelectionExportSchema,
	SelectionGestureSchema,
	SetHistoryDemandSchema,
	SetProjectionDemandSchema,
	SetTerminalDefaultsSchema,
	StartTerminalSchema,
	TerminalTargetSchema,
	UpdateAttachmentInputCapabilitySchema,
	UpdateTerminalConfigurationSchema,
} from './control';
import { isControlRequestKind, MessageKind } from './envelope';
import type { MessageKind as MessageKindValue } from './envelope';
import { ProtocolErrorCode, protocolFailure } from './protocol-result';
import { ReadProcessInfoSchema } from './process-info';
import type { ProtocolResult } from './protocol-result';

/**
 * Encodes client-to-Host control requests only. The message kind chooses the
 * schema, so a producer cannot accidentally pair a valid payload with the
 * wrong command kind.
 */
export function encodeControlRequest(
	kind: MessageKindValue,
	value: unknown,
): ProtocolResult<Uint8Array> {
	if (!isControlRequestKind(kind)) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			`Message kind ${kind} is not a control request`,
		);
	}
	switch (kind) {
		case MessageKind.readProcessInfo:
			return encodeControlJson(value, ReadProcessInfoSchema);
		case MessageKind.applyTerminalTheme:
			return encodeControlJson(value, ApplyTerminalThemeSchema);
		case MessageKind.hello:
			return encodeControlJson(value, HelloSchema);
		case MessageKind.ping:
			return encodeControlJson(value, EmptyControlSchema);
		case MessageKind.listTerminals:
			return encodeControlJson(value, ListTerminalsSchema);
		case MessageKind.createTerminal:
			return encodeControlJson(value, CreateTerminalSchema);
		case MessageKind.startTerminal:
			return encodeControlJson(value, StartTerminalSchema);
		case MessageKind.terminateTerminal:
		case MessageKind.forceTerminateTerminal:
		case MessageKind.disposeTerminal:
			return encodeControlJson(value, TerminalTargetSchema);
		case MessageKind.attachTerminal:
			return encodeControlJson(value, AttachTerminalSchema);
		case MessageKind.detachTerminal:
			return encodeControlJson(value, DetachTerminalSchema);
		case MessageKind.updateAttachmentInputCapability:
			return encodeControlJson(value, UpdateAttachmentInputCapabilitySchema);
		case MessageKind.claimResizeLease:
			return encodeControlJson(value, ClaimResizeLeaseSchema);
		case MessageKind.resizeTerminal:
			return encodeControlJson(value, ResizeTerminalSchema);
		case MessageKind.clearTerminal:
			return encodeControlJson(value, ClearTerminalSchema);
		case MessageKind.resetTerminal:
			return encodeControlJson(value, ResetTerminalSchema);
		case MessageKind.frameApplied:
			return encodeControlJson(value, FrameAppliedSchema);
		case MessageKind.needSnapshot:
			return encodeControlJson(value, NeedSnapshotSchema);
		case MessageKind.readRows:
			return encodeControlJson(value, ReadRowsSchema);
		case MessageKind.setProjectionDemand:
			return encodeControlJson(value, SetProjectionDemandSchema);
		case MessageKind.rowsApplied:
			return encodeControlJson(value, RowsAppliedSchema);
		case MessageKind.setHistoryDemand:
			return encodeControlJson(value, SetHistoryDemandSchema);
		case MessageKind.historyObserved:
			return encodeControlJson(value, HistoryObservedSchema);
		case MessageKind.selectionGesture:
			return encodeControlJson(value, SelectionGestureSchema);
		case MessageKind.selectionCommand:
			return encodeControlJson(value, SelectionCommandSchema);
		case MessageKind.selectionExport:
			return encodeControlJson(value, SelectionExportSchema);
		case MessageKind.selectionExportChunkApplied:
			return encodeControlJson(value, SelectionExportChunkAppliedSchema);
		case MessageKind.selectionExportCancel:
			return encodeControlJson(value, SelectionExportCancelSchema);
		case MessageKind.setTerminalDefaults:
			return encodeControlJson(value, SetTerminalDefaultsSchema);
		case MessageKind.updateTerminalConfiguration:
			return encodeControlJson(value, UpdateTerminalConfigurationSchema);
	}
	return missingControlRequestCodec(kind);
}

function missingControlRequestCodec(kind: never): ProtocolResult<Uint8Array> {
	return protocolFailure(
		ProtocolErrorCode.invalidField,
		`Control request codec is missing for message kind ${String(kind)}`,
	);
}
