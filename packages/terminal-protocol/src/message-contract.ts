export const MessageKind = {
	decodeGraphicsPng: 80,
	graphicsPngDecoded: 81,
	stageTerminalOutput: 82,
	terminalOutputStaged: 83,
	promptLineInput: 84,
	screenGatedKeyInput: 85,
	attachKeeper: 90,
	detachKeeper: 91,
	keeperLost: 92,
	keeperUnresponsive: 93,
	terminalUnheld: 94,
	quiesceTerminal: 95,
	readHandoverRecord: 96,
	handoverRecordChunk: 97,
	resumeTerminal: 98,
	dropTerminal: 99,
	importTerminal: 100,
	writeHandoverRecord: 101,
	handoverRecordWritten: 102,
	restoreTerminal: 103,
	claimTerminal: 104,
	discardImport: 105,
	recoverTerminal: 106,
	startProgram: 110,
	programExited: 111,
	cancelProgram: 112,
	readProgramOutput: 113,
	disposeProgram: 114,
	listPrograms: 115,
	recoverProgram: 116,
	readProcessInfo: 79,
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
	graphicsImage: 'graphics-image',
	terminalFrame: 'terminal-frame',
	handoverRecord: 'handover-record',
} as const;

export type PayloadEncoding = (typeof PayloadEncoding)[keyof typeof PayloadEncoding];

export type MessageRole = 'event' | 'request' | 'response';

interface RequestMessageContract {
	readonly payloadEncoding: PayloadEncoding;
	readonly successKind: MessageKind;
}

interface MessageContract {
	readonly eventEncoding: PayloadEncoding | null;
	readonly name: string;
	readonly request: RequestMessageContract | null;
	readonly responseEncoding: PayloadEncoding | null;
}

const control = PayloadEncoding.controlJson;
const raw = PayloadEncoding.rawBytes;
const record = PayloadEncoding.handoverRecord;

/**
 * The complete current-version message registry. A request entry also owns
 * its successful response kind, so producers do not repeat that association.
 */
export const MESSAGE_CONTRACTS = {
	[MessageKind.decodeGraphicsPng]: {
		eventEncoding: null,
		name: 'DecodeGraphicsPng',
		request: {
			payloadEncoding: PayloadEncoding.graphicsImage,
			successKind: MessageKind.graphicsPngDecoded,
		},
		responseEncoding: null,
	},
	[MessageKind.graphicsPngDecoded]: {
		eventEncoding: null,
		name: 'GraphicsPngDecoded',
		request: null,
		responseEncoding: PayloadEncoding.graphicsImage,
	},
	[MessageKind.stageTerminalOutput]: {
		eventEncoding: null,
		name: 'StageTerminalOutput',
		request: {
			payloadEncoding: raw,
			successKind: MessageKind.terminalOutputStaged,
		},
		responseEncoding: null,
	},
	[MessageKind.terminalOutputStaged]: {
		eventEncoding: null,
		name: 'TerminalOutputStaged',
		request: null,
		responseEncoding: raw,
	},
	[MessageKind.promptLineInput]: {
		eventEncoding: null,
		name: 'PromptLineInput',
		request: { payloadEncoding: raw, successKind: MessageKind.inputAccepted },
		responseEncoding: null,
	},
	[MessageKind.screenGatedKeyInput]: {
		eventEncoding: null,
		name: 'ScreenGatedKeyInput',
		request: { payloadEncoding: raw, successKind: MessageKind.inputAccepted },
		responseEncoding: null,
	},
	[MessageKind.attachKeeper]: {
		eventEncoding: null,
		name: 'AttachKeeper',
		request: { payloadEncoding: control, successKind: MessageKind.attachKeeper },
		responseEncoding: control,
	},
	[MessageKind.detachKeeper]: {
		eventEncoding: null,
		name: 'DetachKeeper',
		request: { payloadEncoding: control, successKind: MessageKind.detachKeeper },
		responseEncoding: control,
	},
	[MessageKind.keeperLost]: {
		eventEncoding: control,
		name: 'KeeperLost',
		request: null,
		responseEncoding: null,
	},
	[MessageKind.keeperUnresponsive]: {
		eventEncoding: control,
		name: 'KeeperUnresponsive',
		request: null,
		responseEncoding: null,
	},
	[MessageKind.terminalUnheld]: {
		eventEncoding: control,
		name: 'TerminalUnheld',
		request: null,
		responseEncoding: null,
	},
	[MessageKind.quiesceTerminal]: {
		eventEncoding: null,
		name: 'QuiesceTerminal',
		request: { payloadEncoding: control, successKind: MessageKind.quiesceTerminal },
		responseEncoding: control,
	},
	[MessageKind.readHandoverRecord]: {
		eventEncoding: null,
		name: 'ReadHandoverRecord',
		request: { payloadEncoding: control, successKind: MessageKind.handoverRecordChunk },
		responseEncoding: null,
	},
	[MessageKind.handoverRecordChunk]: {
		eventEncoding: null,
		name: 'HandoverRecordChunk',
		request: null,
		responseEncoding: record,
	},
	[MessageKind.resumeTerminal]: {
		eventEncoding: null,
		name: 'ResumeTerminal',
		request: { payloadEncoding: control, successKind: MessageKind.resumeTerminal },
		responseEncoding: control,
	},
	[MessageKind.dropTerminal]: {
		eventEncoding: null,
		name: 'DropTerminal',
		request: { payloadEncoding: control, successKind: MessageKind.dropTerminal },
		responseEncoding: control,
	},
	[MessageKind.importTerminal]: {
		eventEncoding: null,
		name: 'ImportTerminal',
		request: { payloadEncoding: control, successKind: MessageKind.importTerminal },
		responseEncoding: control,
	},
	[MessageKind.writeHandoverRecord]: {
		eventEncoding: null,
		name: 'WriteHandoverRecord',
		request: { payloadEncoding: record, successKind: MessageKind.handoverRecordWritten },
		responseEncoding: null,
	},
	[MessageKind.handoverRecordWritten]: {
		eventEncoding: null,
		name: 'HandoverRecordWritten',
		request: null,
		responseEncoding: control,
	},
	[MessageKind.restoreTerminal]: {
		eventEncoding: null,
		name: 'RestoreTerminal',
		request: { payloadEncoding: control, successKind: MessageKind.restoreTerminal },
		responseEncoding: control,
	},
	[MessageKind.claimTerminal]: {
		eventEncoding: null,
		name: 'ClaimTerminal',
		request: { payloadEncoding: control, successKind: MessageKind.claimTerminal },
		responseEncoding: control,
	},
	[MessageKind.discardImport]: {
		eventEncoding: null,
		name: 'DiscardImport',
		request: { payloadEncoding: control, successKind: MessageKind.discardImport },
		responseEncoding: control,
	},
	[MessageKind.recoverTerminal]: {
		eventEncoding: null,
		name: 'RecoverTerminal',
		request: { payloadEncoding: control, successKind: MessageKind.recoverTerminal },
		responseEncoding: control,
	},
	[MessageKind.startProgram]: {
		eventEncoding: null,
		name: 'StartProgram',
		request: { payloadEncoding: control, successKind: MessageKind.startProgram },
		responseEncoding: control,
	},
	[MessageKind.programExited]: {
		eventEncoding: control,
		name: 'ProgramExited',
		request: null,
		responseEncoding: null,
	},
	[MessageKind.cancelProgram]: {
		eventEncoding: null,
		name: 'CancelProgram',
		request: { payloadEncoding: control, successKind: MessageKind.cancelProgram },
		responseEncoding: control,
	},
	[MessageKind.readProgramOutput]: {
		eventEncoding: null,
		name: 'ReadProgramOutput',
		request: { payloadEncoding: control, successKind: MessageKind.readProgramOutput },
		responseEncoding: control,
	},
	[MessageKind.disposeProgram]: {
		eventEncoding: null,
		name: 'DisposeProgram',
		request: { payloadEncoding: control, successKind: MessageKind.disposeProgram },
		responseEncoding: control,
	},
	[MessageKind.listPrograms]: {
		eventEncoding: null,
		name: 'ListPrograms',
		request: { payloadEncoding: control, successKind: MessageKind.listPrograms },
		responseEncoding: control,
	},
	[MessageKind.recoverProgram]: {
		eventEncoding: null,
		name: 'RecoverProgram',
		request: { payloadEncoding: control, successKind: MessageKind.recoverProgram },
		responseEncoding: control,
	},
	[MessageKind.readProcessInfo]: {
		eventEncoding: null,
		name: 'ReadProcessInfo',
		request: { payloadEncoding: control, successKind: MessageKind.readProcessInfo },
		responseEncoding: control,
	},
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

// Only the Worker sends the Host handover requests, under the `keeper`, `handover`, and
// `recovery` capabilities; no TypeScript producer encodes them, so none is a client control request here.
const WORKER_ONLY_CONTROL_REQUEST_KINDS = [
	MessageKind.attachKeeper,
	MessageKind.detachKeeper,
	MessageKind.quiesceTerminal,
	MessageKind.readHandoverRecord,
	MessageKind.resumeTerminal,
	MessageKind.dropTerminal,
	MessageKind.importTerminal,
	MessageKind.restoreTerminal,
	MessageKind.claimTerminal,
	MessageKind.discardImport,
	MessageKind.recoverTerminal,
	MessageKind.startProgram,
	MessageKind.cancelProgram,
	MessageKind.readProgramOutput,
	MessageKind.disposeProgram,
	MessageKind.listPrograms,
	MessageKind.recoverProgram,
] as const;
const workerOnlyControlRequestKinds: ReadonlySet<MessageKind> = new Set(
	WORKER_ONLY_CONTROL_REQUEST_KINDS,
);
type ControlRequestKind = Exclude<
	RequestKindForEncoding<typeof PayloadEncoding.controlJson>,
	(typeof WORKER_ONLY_CONTROL_REQUEST_KINDS)[number]
>;
// Staged output, prompt lines and screen-gated keys are raw bytes too, but only the Worker sends
// them; no TypeScript producer encodes any, so none is a client input request here.
type InputRequestKind = Exclude<
	RequestKindForEncoding<typeof PayloadEncoding.rawBytes>,
	| typeof MessageKind.stageTerminalOutput
	| typeof MessageKind.promptLineInput
	| typeof MessageKind.screenGatedKeyInput
>;

export function isKnownMessageKind(value: number): value is MessageKind {
	return Number.isInteger(value) && Object.hasOwn(MESSAGE_CONTRACTS, value);
}

export function isControlRequestKind(kind: MessageKind): kind is ControlRequestKind {
	return (
		!workerOnlyControlRequestKinds.has(kind) &&
		MESSAGE_CONTRACTS[kind].request?.payloadEncoding === PayloadEncoding.controlJson
	);
}

export function isInputRequestKind(kind: MessageKind): kind is InputRequestKind {
	return (
		kind !== MessageKind.stageTerminalOutput &&
		kind !== MessageKind.promptLineInput &&
		kind !== MessageKind.screenGatedKeyInput &&
		MESSAGE_CONTRACTS[kind].request?.payloadEncoding === PayloadEncoding.rawBytes
	);
}
