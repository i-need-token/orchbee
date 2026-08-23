import { EventEmitter } from 'node:events';

import {
	ActiveScreen,
	CanonicalUint64TextSchema,
	CellWidth,
	CreationIdSchema,
	CursorShape,
	decodeTerminalProjectionBuffer,
	encodeTerminalFrame,
	encodeTerminalRowWindow,
	HostEpochSchema,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalFrameKind,
	TerminalIdSchema,
	TerminalRowWindowDetail,
	TerminalThemeStateSchema,
	UnderlineKind,
	type TerminalIdentity,
} from '@orchbee/terminal-protocol';
import { ProjectedAttachmentBootstrapSchema } from '@orchbee/workbench-client-protocol';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import type { MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import type { ProjectedTerminalBrokerMetrics } from '../../broker';
import type { DesktopTerminalProjectionLease } from '../../terminal-projection-control';
import type { FileTerminalArchiveStore } from '../FileTerminalArchiveStore';
import {
	TerminalArchiveCaptureService,
	type TerminalArchiveCaptureServiceDependencies,
} from '../TerminalArchiveCaptureService';
import { encodeTerminalArchive, type TerminalArchiveWrite } from '../terminal-archive-document';

const presentationId = terminalPresentationId('presentation-capture');
const attachmentId = '11'.repeat(16);
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

function exactArrayBuffer(bytes: Uint8Array): ArrayBuffer {
	const copy = new Uint8Array(bytes.byteLength);
	copy.set(bytes);
	return copy.buffer;
}
