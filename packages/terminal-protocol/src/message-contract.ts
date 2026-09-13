export const MessageKind = {
	applyTerminalTheme: 77,
	attachTerminal: 20,
	attachmentCapabilities: 22,
	resizeLeaseChanged: 23,
	updateAttachmentInputCapability: 24,
	claimResizeLease: 25,
	clearTerminal: 74,
	createTerminal: 11,
	deltaFrame: 42,
	detachTerminal: 21,
	disposeTerminal: 13,
	forceTerminateTerminal: 18,
	frameApplied: 43,
	focusInput: 37,
	hello: 1,
	helloAccepted: 2,
	inputAccepted: 31,
	listTerminals: 10,
	needSnapshot: 44,
	keyInput: 34,
	mouseInput: 36,
	pasteInput: 38,
	pasteAutoInput: 39,
	ping: 4,
	projectionDemandAccepted: 48,
	projectionBlocked: 49,
	rowsApplied: 50,
	setHistoryDemand: 51,
	historyDemandAccepted: 52,
	historyChanged: 53,
	historyObserved: 54,
	terminalBell: 55,
	terminalClipboardWriteRequest: 56,
	terminalMetadataChanged: 57,
	terminalApplicationSignal: 58,
	protocolRejected: 3,
	readRows: 45,
	resizeAccepted: 33,
	resizeTerminal: 32,
	resetTerminal: 75,
	rowsSnapshot: 46,
	selectionGesture: 60,
	selectionCommand: 61,
	selectionChanged: 62,
	selectionExport: 63,
	selectionExportStarted: 64,
	selectionExportChunk: 65,
	selectionExportChunkApplied: 66,
	selectionExportFinished: 67,
	selectionExportCancel: 68,
	setTerminalDefaults: 70,
	terminalDefaultsAccepted: 71,
	updateTerminalConfiguration: 72,
	terminalConfigurationAccepted: 73,
	terminalControlAccepted: 76,
	terminalThemeResult: 78,
	setProjectionDemand: 47,
	snapshotFrame: 41,
	terminalExited: 15,
	startTerminal: 16,
	terminalStartAccepted: 17,
	terminalFailed: 19,
	terminalStarted: 14,
	terminateTerminal: 12,
	textInput: 35,
	typedFailure: 5,
	writeInput: 30,
} as const;

export type MessageKind = (typeof MessageKind)[keyof typeof MessageKind];

export const PayloadEncoding = {
	controlJson: 'control-json',
	rawBytes: 'raw-bytes',
	rowWindow: 'row-window',
	terminalFrame: 'terminal-frame',
} as const;

export type PayloadEncoding = (typeof PayloadEncoding)[keyof typeof PayloadEncoding];

export type MessageRole = 'event' | 'request' | 'response';

export interface RequestMessageContract {
	readonly payloadEncoding: PayloadEncoding;
	readonly successKind: MessageKind;
}

export interface MessageContract {
	readonly eventEncoding: PayloadEncoding | null;
	readonly name: string;
	readonly request: RequestMessageContract | null;
	readonly responseEncoding: PayloadEncoding | null;
}

const control = PayloadEncoding.controlJson;
const raw = PayloadEncoding.rawBytes;

/**
 * The complete current-version message registry. A request entry also owns
 * its successful response kind, so producers do not repeat that association.
 */
export const MESSAGE_CONTRACTS = {
	[MessageKind.hello]: {
		eventEncoding: null,
		name: 'Hello',
		request: { payloadEncoding: control, successKind: MessageKind.helloAccepted },
		responseEncoding: null,
	},
	[MessageKind.helloAccepted]: {
		eventEncoding: null,
		name: 'HelloAccepted',
		request: null,
		responseEncoding: control,
	},
	[MessageKind.protocolRejected]: {
		eventEncoding: null,
		name: 'ProtocolRejected',
		request: null,
		responseEncoding: control,
	},
	[MessageKind.ping]: {
		eventEncoding: null,
		name: 'Ping',
		request: { payloadEncoding: control, successKind: MessageKind.ping },
		responseEncoding: control,
	},
	[MessageKind.typedFailure]: {
		eventEncoding: null,
		name: 'TypedFailure',
		request: null,
		responseEncoding: control,
	},
	[MessageKind.listTerminals]: {
		eventEncoding: null,
		name: 'ListTerminals',
		request: { payloadEncoding: control, successKind: MessageKind.listTerminals },
		responseEncoding: control,
	},
	[MessageKind.createTerminal]: {
		eventEncoding: null,
		name: 'CreateTerminal',
		request: { payloadEncoding: control, successKind: MessageKind.createTerminal },
		responseEncoding: control,
	},
	[MessageKind.terminateTerminal]: {
		eventEncoding: null,
		name: 'TerminateTerminal',
		request: { payloadEncoding: control, successKind: MessageKind.terminateTerminal },
		responseEncoding: control,
	},
	[MessageKind.disposeTerminal]: {
		eventEncoding: null,
		name: 'DisposeTerminal',
		request: { payloadEncoding: control, successKind: MessageKind.disposeTerminal },
		responseEncoding: control,
	},
	[MessageKind.terminalStarted]: {
		eventEncoding: control,
		name: 'TerminalStarted',
		request: null,
		responseEncoding: null,
	},
	[MessageKind.terminalExited]: {
		eventEncoding: control,
		name: 'TerminalExited',
		request: null,
		responseEncoding: null,
	},
	[MessageKind.startTerminal]: {
		eventEncoding: null,
		name: 'StartTerminal',
		request: { payloadEncoding: control, successKind: MessageKind.terminalStartAccepted },
		responseEncoding: null,
	},
	[MessageKind.terminalStartAccepted]: {
		eventEncoding: null,
		name: 'TerminalStartAccepted',
		request: null,
		responseEncoding: control,
	},
	[MessageKind.forceTerminateTerminal]: {
		eventEncoding: null,
		name: 'ForceTerminateTerminal',
		request: { payloadEncoding: control, successKind: MessageKind.forceTerminateTerminal },
		responseEncoding: control,
	},
	[MessageKind.terminalFailed]: {
		eventEncoding: control,
		name: 'TerminalFailed',
		request: null,
		responseEncoding: null,
	},
	[MessageKind.attachTerminal]: {
		eventEncoding: null,
		name: 'AttachTerminal',
		request: { payloadEncoding: control, successKind: MessageKind.attachmentCapabilities },
		responseEncoding: null,
	},
	[MessageKind.detachTerminal]: {
		eventEncoding: null,
		name: 'DetachTerminal',
		request: { payloadEncoding: control, successKind: MessageKind.detachTerminal },
		responseEncoding: control,
	},
	[MessageKind.attachmentCapabilities]: {
		eventEncoding: null,
		name: 'AttachmentCapabilities',
		request: null,
		responseEncoding: control,
	},
	[MessageKind.resizeLeaseChanged]: {
		eventEncoding: control,
		name: 'ResizeLeaseChanged',
		request: null,
		responseEncoding: null,
	},
	[MessageKind.updateAttachmentInputCapability]: {
		eventEncoding: null,
		name: 'UpdateAttachmentInputCapability',
		request: { payloadEncoding: control, successKind: MessageKind.attachmentCapabilities },
		responseEncoding: null,
	},
	[MessageKind.claimResizeLease]: {
		eventEncoding: null,
		name: 'ClaimResizeLease',
		request: { payloadEncoding: control, successKind: MessageKind.attachmentCapabilities },
		responseEncoding: null,
	},
	[MessageKind.writeInput]: {
		eventEncoding: null,
		name: 'WriteInput',
		request: { payloadEncoding: raw, successKind: MessageKind.inputAccepted },
		responseEncoding: null,
	},
	[MessageKind.inputAccepted]: {
		eventEncoding: null,
		name: 'InputAccepted',
		request: null,
		responseEncoding: control,
	},
	[MessageKind.resizeTerminal]: {
		eventEncoding: null,
		name: 'ResizeTerminal',
		request: { payloadEncoding: control, successKind: MessageKind.resizeAccepted },
		responseEncoding: null,
	},
	[MessageKind.resizeAccepted]: {
		eventEncoding: null,
		name: 'ResizeAccepted',
		request: null,
		responseEncoding: control,
	},
	[MessageKind.clearTerminal]: {
		eventEncoding: null,
		name: 'ClearTerminal',
		request: { payloadEncoding: control, successKind: MessageKind.terminalControlAccepted },
		responseEncoding: null,
	},
	[MessageKind.resetTerminal]: {
		eventEncoding: null,
		name: 'ResetTerminal',
		request: { payloadEncoding: control, successKind: MessageKind.terminalControlAccepted },
		responseEncoding: null,
	},
	[MessageKind.terminalControlAccepted]: {
		eventEncoding: null,
		name: 'TerminalControlAccepted',
		request: null,
		responseEncoding: control,
	},
	[MessageKind.keyInput]: {
		eventEncoding: null,
		name: 'KeyInput',
		request: { payloadEncoding: raw, successKind: MessageKind.inputAccepted },
		responseEncoding: null,
	},
	[MessageKind.textInput]: {
		eventEncoding: null,
		name: 'TextInput',
		request: { payloadEncoding: raw, successKind: MessageKind.inputAccepted },
		responseEncoding: null,
	},
	[MessageKind.mouseInput]: {
		eventEncoding: null,
		name: 'MouseInput',
		request: { payloadEncoding: raw, successKind: MessageKind.inputAccepted },
		responseEncoding: null,
	},
	[MessageKind.focusInput]: {
		eventEncoding: null,
		name: 'FocusInput',
		request: { payloadEncoding: raw, successKind: MessageKind.inputAccepted },
		responseEncoding: null,
	},
	[MessageKind.pasteInput]: {
		eventEncoding: null,
		name: 'PasteInput',
		request: { payloadEncoding: raw, successKind: MessageKind.inputAccepted },
		responseEncoding: null,
	},
	[MessageKind.pasteAutoInput]: {
		eventEncoding: null,
		name: 'PasteAutoInput',
		request: { payloadEncoding: raw, successKind: MessageKind.inputAccepted },
		responseEncoding: null,
	},
	[MessageKind.snapshotFrame]: {
		eventEncoding: PayloadEncoding.terminalFrame,
		name: 'SnapshotFrame',
		request: null,
		responseEncoding: null,
	},
	[MessageKind.deltaFrame]: {
		eventEncoding: PayloadEncoding.terminalFrame,
		name: 'DeltaFrame',
		request: null,
		responseEncoding: null,
	},
	[MessageKind.frameApplied]: {
		eventEncoding: null,
		name: 'FrameApplied',
		request: { payloadEncoding: control, successKind: MessageKind.frameApplied },
		responseEncoding: control,
	},
	[MessageKind.needSnapshot]: {
		eventEncoding: null,
		name: 'NeedSnapshot',
		request: { payloadEncoding: control, successKind: MessageKind.needSnapshot },
		responseEncoding: control,
	},
	[MessageKind.readRows]: {
		eventEncoding: null,
		name: 'ReadRows',
		request: { payloadEncoding: control, successKind: MessageKind.rowsSnapshot },
		responseEncoding: null,
	},
	[MessageKind.rowsSnapshot]: {
		eventEncoding: null,
		name: 'RowsSnapshot',
		request: null,
		responseEncoding: PayloadEncoding.rowWindow,
	},
	[MessageKind.setProjectionDemand]: {
		eventEncoding: null,
		name: 'SetProjectionDemand',
		request: {
			payloadEncoding: control,
			successKind: MessageKind.projectionDemandAccepted,
		},
		responseEncoding: null,
	},
	[MessageKind.projectionDemandAccepted]: {
		eventEncoding: null,
		name: 'ProjectionDemandAccepted',
		request: null,
		responseEncoding: control,
	},
	[MessageKind.projectionBlocked]: {
		eventEncoding: control,
		name: 'ProjectionBlocked',
		request: null,
		responseEncoding: null,
	},
	[MessageKind.rowsApplied]: {
		eventEncoding: null,
		name: 'RowsApplied',
		request: { payloadEncoding: control, successKind: MessageKind.rowsApplied },
		responseEncoding: control,
	},
	[MessageKind.setHistoryDemand]: {
		eventEncoding: null,
		name: 'SetHistoryDemand',
		request: { payloadEncoding: control, successKind: MessageKind.historyDemandAccepted },
		responseEncoding: null,
	},
	[MessageKind.historyDemandAccepted]: {
		eventEncoding: null,
		name: 'HistoryDemandAccepted',
		request: null,
		responseEncoding: control,
	},
	[MessageKind.historyChanged]: {
		eventEncoding: control,
		name: 'HistoryChanged',
		request: null,
		responseEncoding: null,
	},
	[MessageKind.historyObserved]: {
		eventEncoding: null,
		name: 'HistoryObserved',
		request: { payloadEncoding: control, successKind: MessageKind.historyObserved },
		responseEncoding: control,
	},
	[MessageKind.terminalBell]: {
		eventEncoding: control,
		name: 'TerminalBell',
		request: null,
		responseEncoding: null,
	},
	[MessageKind.terminalClipboardWriteRequest]: {
		eventEncoding: control,
		name: 'TerminalClipboardWriteRequest',
		request: null,
		responseEncoding: null,
	},
	[MessageKind.terminalMetadataChanged]: {
		eventEncoding: control,
		name: 'TerminalMetadataChanged',
		request: null,
		responseEncoding: null,
	},
	[MessageKind.terminalApplicationSignal]: {
		eventEncoding: control,
		name: 'TerminalApplicationSignal',
		request: null,
		responseEncoding: null,
	},
	[MessageKind.selectionGesture]: {
		eventEncoding: null,
		name: 'SelectionGesture',
		request: { payloadEncoding: control, successKind: MessageKind.selectionChanged },
		responseEncoding: null,
	},
	[MessageKind.selectionCommand]: {
		eventEncoding: null,
		name: 'SelectionCommand',
		request: { payloadEncoding: control, successKind: MessageKind.selectionChanged },
		responseEncoding: null,
	},
	[MessageKind.selectionChanged]: {
		eventEncoding: control,
		name: 'SelectionChanged',
		request: null,
		responseEncoding: control,
	},
	[MessageKind.selectionExport]: {
		eventEncoding: null,
		name: 'SelectionExport',
		request: { payloadEncoding: control, successKind: MessageKind.selectionExportStarted },
		responseEncoding: null,
	},
	[MessageKind.selectionExportStarted]: {
		eventEncoding: null,
		name: 'SelectionExportStarted',
		request: null,
		responseEncoding: control,
	},
	[MessageKind.selectionExportChunk]: {
		eventEncoding: raw,
		name: 'SelectionExportChunk',
		request: null,
		responseEncoding: null,
	},
	[MessageKind.selectionExportChunkApplied]: {
		eventEncoding: null,
		name: 'SelectionExportChunkApplied',
		request: {
			payloadEncoding: control,
			successKind: MessageKind.selectionExportChunkApplied,
		},
		responseEncoding: control,
	},
	[MessageKind.selectionExportFinished]: {
		eventEncoding: control,
		name: 'SelectionExportFinished',
		request: null,
		responseEncoding: control,
	},
	[MessageKind.selectionExportCancel]: {
		eventEncoding: null,
		name: 'SelectionExportCancel',
		request: { payloadEncoding: control, successKind: MessageKind.selectionExportFinished },
		responseEncoding: null,
	},
	[MessageKind.setTerminalDefaults]: {
		eventEncoding: null,
		name: 'SetTerminalDefaults',
		request: { payloadEncoding: control, successKind: MessageKind.terminalDefaultsAccepted },
		responseEncoding: null,
	},
	[MessageKind.terminalDefaultsAccepted]: {
		eventEncoding: null,
		name: 'TerminalDefaultsAccepted',
		request: null,
		responseEncoding: control,
	},
	[MessageKind.updateTerminalConfiguration]: {
		eventEncoding: null,
		name: 'UpdateTerminalConfiguration',
		request: {
			payloadEncoding: control,
			successKind: MessageKind.terminalConfigurationAccepted,
		},
		responseEncoding: null,
	},
	[MessageKind.terminalConfigurationAccepted]: {
		eventEncoding: null,
		name: 'TerminalConfigurationAccepted',
		request: null,
		responseEncoding: control,
	},
	[MessageKind.applyTerminalTheme]: {
		eventEncoding: null,
		name: 'ApplyTerminalTheme',
		request: { payloadEncoding: control, successKind: MessageKind.terminalThemeResult },
		responseEncoding: null,
	},
	[MessageKind.terminalThemeResult]: {
		eventEncoding: null,
		name: 'TerminalThemeResult',
		request: null,
		responseEncoding: control,
	},
} as const satisfies Record<MessageKind, MessageContract>;

type RequestKindForEncoding<Encoding extends PayloadEncoding> = {
	[Kind in MessageKind]: (typeof MESSAGE_CONTRACTS)[Kind]['request'] extends {
		readonly payloadEncoding: Encoding;
	}
		? Kind
		: never;
}[MessageKind];

export type ControlRequestKind = RequestKindForEncoding<typeof PayloadEncoding.controlJson>;
export type InputRequestKind = RequestKindForEncoding<typeof PayloadEncoding.rawBytes>;

export function isKnownMessageKind(value: number): value is MessageKind {
	return Number.isInteger(value) && Object.hasOwn(MESSAGE_CONTRACTS, value);
}

export function messageContract(kind: number): MessageContract | null {
	return isKnownMessageKind(kind) ? MESSAGE_CONTRACTS[kind] : null;
}

export function requestMessageContract(kind: MessageKind): RequestMessageContract | null {
	return MESSAGE_CONTRACTS[kind].request;
}

export function isControlRequestKind(kind: MessageKind): kind is ControlRequestKind {
	return MESSAGE_CONTRACTS[kind].request?.payloadEncoding === PayloadEncoding.controlJson;
}

export function isInputRequestKind(kind: MessageKind): kind is InputRequestKind {
	return MESSAGE_CONTRACTS[kind].request?.payloadEncoding === PayloadEncoding.rawBytes;
}

export function messagePayloadEncoding(kind: number, role: MessageRole): PayloadEncoding | null {
	const contract = messageContract(kind);
	if (contract === null) return null;
	switch (role) {
		case 'event':
			return contract.eventEncoding;
		case 'request':
			return contract.request?.payloadEncoding ?? null;
		case 'response':
			return contract.responseEncoding;
	}
}
