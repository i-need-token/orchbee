import { EventEmitter } from 'node:events';

import {
	ActiveScreen,
	AttachmentIdSchema,
	CanonicalUint64TextSchema,
	CellWidth,
	CreationIdSchema,
	CursorShape,
	decodeTerminalProjectionBuffer,
	decodeTerminalRowWindow,
	encodeTerminalFrame,
	encodeTerminalGraphics,
	encodeTerminalRowWindow,
	HostEpochSchema,
	PROTOCOL_LIMITS,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalFrameKind,
	TerminalGraphicsUpdateKind,
	TerminalIdSchema,
	TerminalRowWindowDetail,
	TerminalThemeStateSchema,
	UnderlineKind,
	type TerminalFrame,
	type TerminalIdentity,
} from '@orchbee/terminal-protocol';
import {
	ProjectedAttachmentBootstrapSchema,
	terminalPresentationId,
} from '@orchbee/workbench-client-protocol';
import type { MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import type { ProjectedTerminalBrokerMetrics } from '../../broker';
import type { DesktopTerminalProjectionLease } from '../../terminal-projection-control';
import type { FileTerminalArchiveStore } from '../FileTerminalArchiveStore';
import {
	TerminalArchiveCaptureService,
	type TerminalArchiveCaptureServiceDependencies,
} from '../TerminalArchiveCaptureService';
import {
	encodeTerminalArchive,
	encodeTerminalArchiveForCaptureSizing,
	type TerminalArchiveWrite,
} from '../terminal-archive-document';

const terminalProtocolOperations = vi.hoisted(() => ({ decodedRowWindows: 0 }));

vi.mock(import('@orchbee/terminal-protocol'), async (importOriginal) => {
	const actual = await importOriginal();
	return {
		...actual,
		decodeTerminalRowWindow(bytes: Uint8Array) {
			terminalProtocolOperations.decodedRowWindows += 1;
			return actual.decodeTerminalRowWindow(bytes);
		},
	};
});

const presentationId = terminalPresentationId('presentation-capture');
const emptyGraphics = {
	animationTick: 0n,
	images: [],
	kind: TerminalGraphicsUpdateKind.snapshot,
	placements: [],
	resourceRevision: 0n,
} as const;
const encodedEmptyGraphics = encodeTerminalGraphics(emptyGraphics);
if (!encodedEmptyGraphics.ok) throw new Error(encodedEmptyGraphics.detail);
const emptyGraphicsBytes = encodedEmptyGraphics.value;
const attachmentId = AttachmentIdSchema.parse('11'.repeat(16));
const identity: TerminalIdentity = {
	creationId: CreationIdSchema.parse('22'.repeat(16)),
	hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
	terminalId: TerminalIdSchema.parse('44'.repeat(16)),
};
const theme = TerminalThemeStateSchema.parse({
	operationId: null,
	revision: '0',
	terminal: {
		colorScheme: 'dark',
		defaultBackgroundRgb: 0,
		defaultCursorRgb: null,
		defaultForegroundRgb: 0xff_ff_ff,
		palette: Array.from({ length: 256 }, (_, index) => index),
	},
});
const emptyMetrics: ProjectedTerminalBrokerMetrics = {
	clipboardCommits: 0,
	commandsAccepted: 0,
	commandsRejected: 0,
	externalTargetsOpened: 0,
	frameBytesForwarded: 0,
	framesForwarded: 0,
	lastSelectionCopyMilliseconds: null,
	lastSelectionExportStartMilliseconds: null,
	rowWindowBytesForwarded: 0,
	rowWindowsForwarded: 0,
	selectionExportBytesForwarded: 0,
	selectionExportChunksForwarded: 0,
};
const captureStyle: TerminalFrame['styles'][number] = {
	background: { kind: TerminalColorKind.default },
	blink: false,
	bold: false,
	faint: false,
	foreground: { kind: TerminalColorKind.default },
	hyperlink: null,
	inverse: false,
	invisible: false,
	italic: false,
	overline: false,
	strikethrough: false,
	underline: { kind: TerminalColorKind.none },
	underlineKind: UnderlineKind.none,
};

describe('TerminalArchiveCaptureService', () => {
	it('captures final history before releasing the retired read-only projection', async () => {
		const finalFrame = encodedLiveFrame(3n, ['viewport-a', 'viewport-b']);
		const port = new CapturePort((command) => {
			if (command.kind === 'set-projection-demand') {
				port.publish({ frame: exactArrayBuffer(finalFrame), kind: 'frame' });
				return;
			}
			if (command.kind !== 'read-rows') return;
			const requestSequence = command.payload.requestSequence;
			if (requestSequence === undefined) throw new Error('Missing row request sequence');
			const viewport = requestSequence === '1';
			const rowTexts = viewport
				? ['viewport-a', 'viewport-b']
				: ['history-a', 'history-b', 'history-c'];
			const startRow = viewport ? 3n : 0n;
			const history = encodeTerminalRowWindow({
				attachmentId,
				columns: 2,
				detail: TerminalRowWindowDetail.render,
				frame: createFrame(startRow, rowTexts, BigInt(requestSequence)),
				graphics: emptyGraphics,
				generation: 3n,
				hostEpoch: identity.hostEpoch,
				revision: 5n,
				sequence: BigInt(requestSequence),
				startRow,
				totalRows: 5n,
			});
			if (!history.ok) throw new Error(history.detail);
			port.publish({ kind: 'rows', rows: exactArrayBuffer(history.value) });
		});
		const dispose = vi.fn<() => Promise<void>>().mockResolvedValue();
		const put = vi.fn<(archive: TerminalArchiveWrite) => Promise<void>>().mockResolvedValue();
		const capture = new TerminalArchiveCaptureService({
			openCaptureReadOnly: vi
				.fn<TerminalArchiveCaptureServiceDependencies['openCaptureReadOnly']>()
				.mockResolvedValue(createLease(port, dispose)),
			store: { put } as unknown as FileTerminalArchiveStore,
		});

		await capture.capture(presentationId, identity, CanonicalUint64TextSchema.parse('5'));

		expect(dispose).toHaveBeenCalledOnce();
		expect(put).toHaveBeenCalledOnce();
		const stored = put.mock.calls[0]?.[0];
		if (stored === undefined) throw new Error('Capture did not store an archive');
		expect(stored.rowWindows).toHaveLength(1);
		const viewport = decodeTerminalProjectionBuffer(stored.frame);
		if (!viewport.ok) throw new Error(viewport.detail);
		expect(viewport.value.rowData.indices).toHaveLength(2);
		const encoded = encodeTerminalArchive(stored);
		if (!encoded.ok) throw new Error(encoded.detail);
		expect(encoded.value.byteLength).toBeGreaterThan(finalFrame.byteLength);
	});

	it.each([20, 40, 80])(
		'decodes $historyRows one-row history sources with linear work while retaining every row',
		async (historyRows) => {
			const totalRows = BigInt(historyRows + 1);
			const finalFrame = encodedLiveFrame(BigInt(historyRows), ['viewport']);
			const port = new CapturePort((command) => {
				if (command.kind === 'set-projection-demand') {
					port.publish({ frame: exactArrayBuffer(finalFrame), kind: 'frame' });
					return;
				}
				if (command.kind !== 'read-rows') return;
				const requestSequence = command.payload.requestSequence;
				if (requestSequence === undefined) throw new Error('Missing row request sequence');
				const sequence = BigInt(requestSequence);
				const isViewport = sequence === 1n;
				const startRow = isViewport ? BigInt(historyRows) : BigInt(historyRows) - (sequence - 1n);
				const encoded = encodeTerminalRowWindow({
					attachmentId,
					columns: 2,
					detail: TerminalRowWindowDetail.render,
					frame: createFrame(startRow, [isViewport ? 'viewport' : `history-${startRow}`], sequence),
					graphics: emptyGraphics,
					generation: 3n,
					hostEpoch: identity.hostEpoch,
					revision: 5n,
					sequence,
					startRow,
					totalRows,
				});
				if (!encoded.ok) throw new Error(encoded.detail);
				port.publish({ kind: 'rows', rows: exactArrayBuffer(encoded.value) });
			});
			const dispose = vi.fn<() => Promise<void>>().mockResolvedValue();
			const put = vi.fn<(archive: TerminalArchiveWrite) => Promise<void>>().mockResolvedValue();
			const capture = new TerminalArchiveCaptureService({
				openCaptureReadOnly: vi
					.fn<TerminalArchiveCaptureServiceDependencies['openCaptureReadOnly']>()
					.mockResolvedValue(createLease(port, dispose)),
				store: { put } as unknown as FileTerminalArchiveStore,
			});
			terminalProtocolOperations.decodedRowWindows = 0;

			await capture.capture(presentationId, identity, CanonicalUint64TextSchema.parse('5'));

			const captureDecodeCount = terminalProtocolOperations.decodedRowWindows;
			const stored = put.mock.calls[0]?.[0];
			if (stored === undefined) throw new Error('Capture did not store an archive');
			expect(stored.rowWindows).toHaveLength(historyRows);
			expect(
				stored.rowWindows.map((bytes) => {
					const decoded = decodeTerminalRowWindow(bytes);
					if (!decoded.ok) throw new Error(decoded.detail);
					return decoded.value.startRow;
				}),
			).toEqual(Array.from({ length: historyRows }, (_, index) => BigInt(index)));
			const admitted = encodeTerminalArchive(stored);
			expect(admitted.ok).toBe(true);
			expect(captureDecodeCount).toBe(2 * historyRows + 1);
			expect(dispose).toHaveBeenCalledOnce();
		},
	);

	it('retains exactly the largest complete history prefix admitted by the archive byte limit', async () => {
		const scenario = createByteBoundedCaptureScenario();
		const port = new CapturePort((command) => {
			if (command.kind === 'set-projection-demand') {
				port.publish({ frame: exactArrayBuffer(scenario.finalFrame), kind: 'frame' });
				return;
			}
			if (command.kind !== 'read-rows') return;
			const requestSequence = command.payload.requestSequence;
			if (requestSequence === undefined) throw new Error('Missing row request sequence');
			const response = scenario.rowWindows.get(requestSequence);
			if (response === undefined) throw new Error('Unexpected archive row request');
			port.publish({ kind: 'rows', rows: exactArrayBuffer(response) });
		});
		const dispose = vi.fn<() => Promise<void>>().mockResolvedValue();
		const put = vi.fn<(archive: TerminalArchiveWrite) => Promise<void>>().mockResolvedValue();
		const capture = new TerminalArchiveCaptureService({
			openCaptureReadOnly: vi
				.fn<TerminalArchiveCaptureServiceDependencies['openCaptureReadOnly']>()
				.mockResolvedValue(createLease(port, dispose)),
			store: { put } as unknown as FileTerminalArchiveStore,
		});

		await capture.capture(presentationId, identity, CanonicalUint64TextSchema.parse('5'));

		const stored = put.mock.calls[0]?.[0];
		if (stored === undefined) throw new Error('Capture did not store an archive');
		expect(scenario.expectedRetainedWindows).toHaveLength(5);
		expect(stored.rowWindows.map((window) => window.byteLength)).toEqual(
			scenario.expectedRetainedWindows.map((window) => window.byteLength),
		);
		expect(stored.rowWindows).toHaveLength(scenario.expectedRetainedWindows.length);
		expect(encodeTerminalArchive(stored).ok).toBe(true);
		expect(dispose).toHaveBeenCalledOnce();
	});

	it('times out without blocking projection release or writing a partial archive', async () => {
		const port = new CapturePort(() => undefined);
		const dispose = vi.fn<() => Promise<void>>().mockResolvedValue();
		const put = vi.fn<(archive: TerminalArchiveWrite) => Promise<void>>().mockResolvedValue();
		const capture = new TerminalArchiveCaptureService({
			captureDeadlineMilliseconds: 10,
			openCaptureReadOnly: vi
				.fn<TerminalArchiveCaptureServiceDependencies['openCaptureReadOnly']>()
				.mockResolvedValue(createLease(port, dispose)),
			store: { put } as unknown as FileTerminalArchiveStore,
		});

		await expect(
			capture.capture(presentationId, identity, CanonicalUint64TextSchema.parse('5')),
		).rejects.toThrow('timed out');
		expect(dispose).toHaveBeenCalledOnce();
		expect(put).not.toHaveBeenCalled();
	});

	it('bounds attachment opening and releases a lease that arrives after the deadline', async () => {
		const port = new CapturePort(() => undefined);
		const dispose = vi.fn<() => Promise<void>>().mockResolvedValue();
		const put = vi.fn<(archive: TerminalArchiveWrite) => Promise<void>>().mockResolvedValue();
		const opening = createDeferred<DesktopTerminalProjectionLease>();
		const capture = new TerminalArchiveCaptureService({
			captureDeadlineMilliseconds: 10,
			openCaptureReadOnly: vi
				.fn<TerminalArchiveCaptureServiceDependencies['openCaptureReadOnly']>()
				.mockReturnValue(opening.promise),
			store: { put } as unknown as FileTerminalArchiveStore,
		});

		await expect(
			capture.capture(presentationId, identity, CanonicalUint64TextSchema.parse('5')),
		).rejects.toThrow('timed out');
		expect(put).not.toHaveBeenCalled();

		opening.resolve(createLease(port, dispose));
		await Promise.resolve();
		await Promise.resolve();
		expect(dispose).toHaveBeenCalledOnce();
	});

	it.each([
		{ createScenario: createStyleCaptureScenario, kind: 'style' },
		{ createScenario: createStringCaptureScenario, kind: 'string' },
	])(
		'rejects an aggregate multi-window $kind overflow before archive store exposure',
		async ({ createScenario }) => {
			const scenario = createScenario();
			const port = new CapturePort((command) => {
				if (command.kind === 'set-projection-demand') {
					port.publish({ frame: exactArrayBuffer(scenario.finalFrame), kind: 'frame' });
					return;
				}
				if (command.kind !== 'read-rows') return;
				const requestSequence = command.payload.requestSequence;
				if (requestSequence === undefined) throw new Error('Missing row request sequence');
				const response = scenario.rowWindows.get(requestSequence);
				if (response === undefined) throw new Error('Unexpected archive row request');
				port.publish({ kind: 'rows', rows: exactArrayBuffer(response) });
			});
			const dispose = vi.fn<() => Promise<void>>().mockResolvedValue();
			const put = vi.fn<(archive: TerminalArchiveWrite) => Promise<void>>().mockResolvedValue();
			const capture = new TerminalArchiveCaptureService({
				openCaptureReadOnly: vi
					.fn<TerminalArchiveCaptureServiceDependencies['openCaptureReadOnly']>()
					.mockResolvedValue(createLease(port, dispose)),
				store: { put } as unknown as FileTerminalArchiveStore,
			});

			await expect(
				capture.capture(presentationId, identity, CanonicalUint64TextSchema.parse('5')),
			).rejects.toThrow('representability');
			expect(put).not.toHaveBeenCalled();
			expect(dispose).toHaveBeenCalledOnce();
		},
	);
});

class CapturePort extends EventEmitter {
	readonly #handleCommand: (command: {
		readonly kind: string;
		readonly payload: Record<string, unknown> & { readonly requestSequence?: string };
	}) => void;

	constructor(
		handleCommand: (command: {
			readonly kind: string;
			readonly payload: Record<string, unknown> & { readonly requestSequence?: string };
		}) => void,
	) {
		super();
		this.#handleCommand = handleCommand;
	}

	close(): void {}

	postMessage(value: unknown): void {
		queueMicrotask(() => this.#handleCommand(value as never));
	}

	publish(value: unknown): void {
		queueMicrotask(() => this.emit('message', { data: value }));
	}

	start(): void {}
}

function createLease(
	port: CapturePort,
	dispose: () => Promise<void>,
): DesktopTerminalProjectionLease {
	return {
		bootstrap: ProjectedAttachmentBootstrapSchema.parse({
			attachmentId,
			canInput: false,
			canResize: false,
			capabilityRevision: '1',
			creationId: identity.creationId,
			hostEpoch: identity.hostEpoch,
			terminalId: identity.terminalId,
			terminalOutcome: null,
			theme,
		}),
		dispose,
		metrics: emptyMetrics,
		rendererPort: port as unknown as MessagePortMain,
		setInputCapability: vi.fn<DesktopTerminalProjectionLease['setInputCapability']>(),
	};
}

function createDeferred<Value>(): {
	readonly promise: Promise<Value>;
	readonly resolve: (value: Value) => void;
} {
	let resolvePromise: (value: Value) => void = () => undefined;
	const promise = new Promise<Value>((resolve) => {
		resolvePromise = resolve;
	});
	return { promise, resolve: resolvePromise };
}

function encodedLiveFrame(scrollbackRows: bigint, rows: readonly string[]): Uint8Array {
	const encoded = encodeTerminalFrame(createFrame(scrollbackRows, rows));
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function createFrame(scrollbackRows: bigint, rowTexts: readonly string[], sequence = 1n) {
	return {
		activeScreen: ActiveScreen.primary,
		attachmentId,
		baseRevision: null,
		cellPixelHeight: 18,
		cellPixelWidth: 9,
		columns: 2,
		currentDirectory: '/tmp',
		cursor: {
			blinking: false,
			column: 0,
			row: rowTexts.length - 1,
			shape: CursorShape.block,
			visible: false,
			wideTail: false,
		},
		defaultColors: { backgroundRgb: 0, cursorRgb: null, foregroundRgb: 0xff_ff_ff },
		extensions: [],
		generation: 3n,
		hostEpoch: identity.hostEpoch,
		interaction: {
			alternateScroll: false,
			applicationCursorKeys: false,
			applicationKeypad: false,
			bracketedPaste: false,
			focusReporting: false,
			kittyDisambiguateEscapeCodes: false,
			kittyReportAllKeysAsEscapeCodes: false,
			kittyReportAlternateKeys: false,
			kittyReportAssociatedText: false,
			kittyReportEventTypes: false,
			modifyOtherKeys2: false,
			mouseAny: false,
			mouseButton: false,
			mouseNormal: false,
			mouseSgr: false,
			mouseSgrPixels: false,
			mouseUtf8: false,
			mouseUrxvt: false,
			mouseX10: false,
		},
		kind: TerminalFrameKind.snapshot,
		revision: 5n,
		rowData: rowTexts.map((text, index) => ({
			cells: [
				{
					grapheme: text,
					protected: false,
					semanticContent: SemanticContent.output,
					style: 0,
					width: CellWidth.narrow,
				},
				{
					grapheme: '',
					protected: false,
					semanticContent: SemanticContent.output,
					style: 0,
					width: CellWidth.narrow,
				},
			],
			index,
			promptClass: RowPromptClass.none,
			wrap: false,
			wrapContinuation: false,
		})),
		rows: rowTexts.length,
		scrollbackRows,
		sequence,
		styles: [
			{
				background: { kind: TerminalColorKind.default },
				blink: false,
				bold: false,
				faint: false,
				foreground: { kind: TerminalColorKind.default },
				hyperlink: null,
				inverse: false,
				invisible: false,
				italic: false,
				overline: false,
				strikethrough: false,
				underline: { kind: TerminalColorKind.none },
				underlineKind: UnderlineKind.none,
			},
		],
		themeRevision: 0n,
		title: 'captured terminal',
	};
}

interface CaptureCardinalityCell {
	readonly grapheme: string;
	readonly style: number;
}

interface CaptureCardinalityScenario {
	readonly finalFrame: Uint8Array;
	readonly rowWindows: ReadonlyMap<string, Uint8Array>;
}

interface ByteBoundedCaptureScenario extends CaptureCardinalityScenario {
	readonly expectedRetainedWindows: readonly Uint8Array[];
}

function createByteBoundedCaptureScenario(): ByteBoundedCaptureScenario {
	const columns = 96;
	const historyRows = 7;
	const totalRows = BigInt(historyRows + 1);
	const cells = Array.from({ length: columns }, () => ({
		grapheme: 'x'.repeat(PROTOCOL_LIMITS.maxStringBytes),
		style: 0,
	}));
	const finalFrame = encodeTerminalFrame(
		createCaptureCardinalityFrame({
			columns,
			currentDirectory: '/byte-bounded-archive',
			rows: [cells],
			scrollbackRows: BigInt(historyRows),
			styles: [captureStyle],
			title: 'byte-bounded-archive',
		}),
	);
	if (!finalFrame.ok) throw new Error(finalFrame.detail);
	const viewport = encodeCaptureWindow({
		columns,
		currentDirectory: '/byte-bounded-archive',
		rows: [cells],
		sequence: 1n,
		startRow: BigInt(historyRows),
		styles: [captureStyle],
		title: 'byte-bounded-archive',
		totalRows,
	});
	const newestFirst = Array.from({ length: historyRows }, (_value, captureIndex) => {
		const startRow = BigInt(historyRows - captureIndex - 1);
		return encodeCaptureWindow({
			columns,
			currentDirectory: '/byte-bounded-archive',
			rows: [cells],
			sequence: BigInt(captureIndex + 2),
			startRow,
			styles: [captureStyle],
			title: 'byte-bounded-archive',
			totalRows,
		});
	});
	const decodedViewport = decodeTerminalRowWindow(viewport);
	if (!decodedViewport.ok || decodedViewport.value.detail !== TerminalRowWindowDetail.render) {
		throw new Error('Byte-bounded viewport failed to decode');
	}
	let expectedRetainedWindows: Uint8Array[] = [];
	let rejectionDetail: string | null = null;
	for (const window of newestFirst) {
		const candidateWindows = [window, ...expectedRetainedWindows];
		const candidate = encodeTerminalArchiveForCaptureSizing({
			frame: decodedViewport.value.frame.bytes,
			graphics: emptyGraphicsBytes,
			identity,
			presentationId,
			rowWindows: candidateWindows,
			theme,
		});
		if (!candidate.ok) {
			rejectionDetail = candidate.detail;
			break;
		}
		expectedRetainedWindows = candidateWindows;
	}
	if (rejectionDetail === null)
		throw new Error('Byte-bounded fixture did not reach the archive limit');
	expect(rejectionDetail).toContain('byte limit');
	return {
		expectedRetainedWindows,
		finalFrame: finalFrame.value,
		rowWindows: new Map([
			['1', viewport],
			...newestFirst.map((window, index) => [(index + 2).toString(), window] as const),
		]),
	};
}

function createStyleCaptureScenario(): CaptureCardinalityScenario {
	const columns = 41;
	const historyRows = 25;
	const firstWindowRows = 12;
	const totalRows = BigInt(historyRows * 2);
	const later = createStyleCaptureWindow(
		BigInt(firstWindowRows),
		historyRows - firstWindowRows,
		firstWindowRows * columns,
		columns,
		totalRows,
		2n,
	);
	const earlier = createStyleCaptureWindow(0n, firstWindowRows, 0, columns, totalRows, 3n);
	return createCaptureScenario(
		columns,
		historyRows,
		[trueColorCaptureStyle(0)],
		Array.from({ length: columns }, () => ({ grapheme: 's', style: 0 })),
		later,
		earlier,
		'style-archive',
		'/style-archive',
	);
}

function createStyleCaptureWindow(
	startRow: bigint,
	rowCount: number,
	firstStyle: number,
	columns: number,
	totalRows: bigint,
	sequence: bigint,
): Uint8Array {
	const styles = Array.from({ length: rowCount * columns }, (_value, index) =>
		trueColorCaptureStyle(firstStyle + index),
	);
	const rows = Array.from({ length: rowCount }, (_value, row) =>
		Array.from({ length: columns }, (_cell, column) => ({
			grapheme: 's',
			style: row * columns + column,
		})),
	);
	return encodeCaptureWindow({
		columns,
		currentDirectory: '/style-archive',
		rows,
		sequence,
		startRow,
		styles,
		title: 'style-archive',
		totalRows,
	});
}

function createStringCaptureScenario(): CaptureCardinalityScenario {
	const columns = 63;
	const historyRows = 65;
	const firstWindowRows = 32;
	const totalRows = BigInt(historyRows * 2);
	const styles = hyperlinkCaptureStyles();
	const later = createStringCaptureWindow(
		BigInt(firstWindowRows),
		historyRows - firstWindowRows,
		firstWindowRows * columns,
		columns,
		totalRows,
		2n,
		styles,
	);
	const earlier = createStringCaptureWindow(0n, firstWindowRows, 0, columns, totalRows, 3n, styles);
	return createCaptureScenario(
		columns,
		historyRows,
		styles,
		Array.from({ length: columns }, (_, index) => ({
			grapheme: captureCardinalityGrapheme(0),
			style: index % styles.length,
		})),
		later,
		earlier,
		'archive-title',
		'/archive-directory',
	);
}

function createStringCaptureWindow(
	startRow: bigint,
	rowCount: number,
	firstCell: number,
	columns: number,
	totalRows: bigint,
	sequence: bigint,
	styles: readonly TerminalFrame['styles'][number][],
): Uint8Array {
	const distinctGraphemeCount = PROTOCOL_LIMITS.maxFrameStrings - 3;
	const rows = Array.from({ length: rowCount }, (_value, row) =>
		Array.from({ length: columns }, (_cell, column) => {
			const cell = firstCell + row * columns + column;
			return {
				grapheme: captureCardinalityGrapheme(cell < distinctGraphemeCount ? cell : 0),
				style: cell % styles.length,
			};
		}),
	);
	return encodeCaptureWindow({
		columns,
		currentDirectory: '/archive-directory',
		rows,
		sequence,
		startRow,
		styles,
		title: 'archive-title',
		totalRows,
	});
}

function createCaptureScenario(
	columns: number,
	historyRows: number,
	styles: readonly TerminalFrame['styles'][number][],
	viewportCells: readonly CaptureCardinalityCell[],
	later: Uint8Array,
	earlier: Uint8Array,
	title: string,
	currentDirectory: string,
): CaptureCardinalityScenario {
	const viewportFrame = createCaptureCardinalityFrame({
		columns,
		currentDirectory,
		rows: Array.from({ length: historyRows }, () => viewportCells),
		scrollbackRows: BigInt(historyRows),
		styles,
		title,
	});
	const finalFrame = encodeTerminalFrame(viewportFrame);
	if (!finalFrame.ok) throw new Error(finalFrame.detail);
	const viewport = encodeCaptureWindow({
		columns,
		currentDirectory,
		rows: Array.from({ length: historyRows }, () => viewportCells),
		sequence: 1n,
		startRow: BigInt(historyRows),
		styles,
		title,
		totalRows: BigInt(historyRows * 2),
	});
	return {
		finalFrame: finalFrame.value,
		rowWindows: new Map([
			['1', viewport],
			['2', later],
			['3', earlier],
		]),
	};
}

function encodeCaptureWindow(value: {
	readonly columns: number;
	readonly currentDirectory: string;
	readonly rows: readonly (readonly CaptureCardinalityCell[])[];
	readonly sequence: bigint;
	readonly startRow: bigint;
	readonly styles: readonly TerminalFrame['styles'][number][];
	readonly title: string;
	readonly totalRows: bigint;
}): Uint8Array {
	const encoded = encodeTerminalRowWindow({
		attachmentId,
		columns: value.columns,
		detail: TerminalRowWindowDetail.render,
		frame: createCaptureCardinalityFrame({
			columns: value.columns,
			currentDirectory: value.currentDirectory,
			rows: value.rows,
			scrollbackRows: value.startRow,
			sequence: value.sequence,
			styles: value.styles,
			title: value.title,
		}),
		graphics: emptyGraphics,
		generation: 3n,
		hostEpoch: identity.hostEpoch,
		revision: 5n,
		sequence: value.sequence,
		startRow: value.startRow,
		totalRows: value.totalRows,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function createCaptureCardinalityFrame(value: {
	readonly columns: number;
	readonly currentDirectory: string;
	readonly rows: readonly (readonly CaptureCardinalityCell[])[];
	readonly scrollbackRows: bigint;
	readonly sequence?: bigint;
	readonly styles: readonly TerminalFrame['styles'][number][];
	readonly title: string;
}): TerminalFrame {
	return {
		activeScreen: ActiveScreen.primary,
		attachmentId,
		baseRevision: null,
		cellPixelHeight: 18,
		cellPixelWidth: 9,
		columns: value.columns,
		currentDirectory: value.currentDirectory,
		cursor: {
			blinking: false,
			column: 0,
			row: 0,
			shape: CursorShape.block,
			visible: false,
			wideTail: false,
		},
		defaultColors: { backgroundRgb: 0, cursorRgb: null, foregroundRgb: 0xff_ff_ff },
		extensions: [],
		generation: 3n,
		hostEpoch: identity.hostEpoch,
		interaction: createFrame(0n, ['x']).interaction,
		kind: TerminalFrameKind.snapshot,
		revision: 5n,
		rowData: value.rows.map((cells, index) => ({
			cells: cells.map((cell) => ({
				grapheme: cell.grapheme,
				protected: false,
				semanticContent: SemanticContent.output,
				style: cell.style,
				width: CellWidth.narrow,
			})),
			index,
			promptClass: RowPromptClass.none,
			wrap: false,
			wrapContinuation: false,
		})),
		rows: value.rows.length,
		scrollbackRows: value.scrollbackRows,
		sequence: value.sequence ?? 1n,
		styles: [...value.styles],
		themeRevision: 0n,
		title: value.title,
	};
}

function trueColorCaptureStyle(rgb: number): TerminalFrame['styles'][number] {
	return { ...captureStyle, foreground: { kind: TerminalColorKind.rgb, rgb } };
}

function hyperlinkCaptureStyles(): readonly TerminalFrame['styles'][number][] {
	return [
		{ ...captureStyle, hyperlink: 'archive-link-a' },
		{ ...captureStyle, hyperlink: 'archive-link-b' },
	];
}

function captureCardinalityGrapheme(index: number): string {
	return String.fromCodePoint(0xe000 + index);
}

function exactArrayBuffer(bytes: Uint8Array): ArrayBuffer {
	const copy = new Uint8Array(bytes.byteLength);
	copy.set(bytes);
	return copy.buffer;
}
