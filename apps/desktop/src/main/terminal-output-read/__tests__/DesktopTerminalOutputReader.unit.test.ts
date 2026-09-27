import { EventEmitter } from 'node:events';

import {
	ActiveScreen,
	AttachmentIdSchema,
	CellWidth,
	CreationIdSchema,
	CursorShape,
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
	type TerminalFrame,
} from '@orchbee/terminal-protocol';
import {
	ProjectedAttachmentBootstrapSchema,
	ProjectedTerminalCommandSchema,
	type ProjectedTerminalCommand,
} from '@orchbee/workbench-client-protocol';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import type { MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import type { ProjectedTerminalBrokerMetrics } from '../../broker';
import type { DesktopTerminalProjectionLease } from '../../terminal-projection-control';
import {
	DesktopTerminalOutputReader,
	type DesktopTerminalOutputProjectionPort,
} from '../DesktopTerminalOutputReader';

const presentationId = terminalPresentationId('presentation-output-reader');
const attachmentId = AttachmentIdSchema.parse('11'.repeat(16));
const creationId = CreationIdSchema.parse('22'.repeat(16));
const hostEpoch = HostEpochSchema.parse('33'.repeat(16));
const terminalId = TerminalIdSchema.parse('44'.repeat(16));
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
const metrics: ProjectedTerminalBrokerMetrics = {
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

describe('DesktopTerminalOutputReader', () => {
	it('reads one exact plain-text tail through a capture-only attachment and disposes before publication', async () => {
		const commands: ProjectedTerminalCommand[] = [];
		const order: string[] = [];
		const port = new ReadPort((command) => {
			commands.push(command);
			if (command.kind === 'set-projection-demand') {
				port.publish({ frame: encodedFrame(), kind: 'frame' });
				return;
			}
			if (command.kind === 'read-rows') {
				port.publish({ kind: 'rows', rows: encodedRows(command.payload.requestSequence) });
			}
		});
		const dispose = vi.fn<() => Promise<void>>(async () => {
			order.push('dispose');
		});
		const projection = projectionPort(createLease(port, dispose), () => {
			order.push('publication-check');
			return true;
		});
		const reader = new DesktopTerminalOutputReader({ projection });

		await expect(reader.read(presentationId, 3, new AbortController().signal)).resolves.toEqual({
			kind: 'completed',
			output: {
				activeScreen: 'primary',
				format: 'plain-text',
				generation: '3',
				presentationId,
				requestedRows: 3,
				returnedRows: 3,
				revision: '5',
				source: 'tail-unwrapped',
				startRow: '0',
				text: 'firstsecond\nthird',
				totalRows: '3',
				truncated: false,
			},
		});
		expect(order).toEqual(['dispose', 'publication-check']);
		expect(commands).toHaveLength(2);
		expect(commands[1]).toMatchObject({
			kind: 'read-rows',
			payload: {
				detail: 'text',
				generation: '3',
				origin: { kind: 'tail', rowsBefore: 0 },
				revision: '5',
				revisionPolicy: 'exact',
				rowCount: 3,
			},
		});
	});

	it('reports a revision change as stale without publishing text', async () => {
		const port = new ReadPort((command) => {
			if (command.kind === 'set-projection-demand') {
				port.publish({ frame: encodedFrame(), kind: 'frame' });
				return;
			}
			if (command.kind === 'read-rows') {
				port.publish({
					failure: {
						code: 'stale-revision',
						message: 'ReadRows used a stale revision.',
						operation: 'history',
						requestSequence: command.payload.requestSequence,
						retryable: true,
					},
					kind: 'failure',
				});
			}
		});
		const dispose = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
		const projection = projectionPort(createLease(port, dispose));
		const reader = new DesktopTerminalOutputReader({ projection });

		await expect(reader.read(presentationId, 3, new AbortController().signal)).resolves.toEqual({
			kind: 'stale',
		});
		expect(dispose).toHaveBeenCalledOnce();
		expect(projection.isPublicationTargetCurrent).not.toHaveBeenCalled();
	});

	it('fences target retirement after row delivery and surfaces cleanup failure first', async () => {
		const successfulPort = respondingPort();
		const successfulDispose = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
		const retired = projectionPort(createLease(successfulPort, successfulDispose), () => false);
		await expect(
			new DesktopTerminalOutputReader({ projection: retired }).read(
				presentationId,
				3,
				new AbortController().signal,
			),
		).resolves.toEqual({ kind: 'target-retired' });
		expect(successfulDispose).toHaveBeenCalledOnce();

		const cleanupPort = respondingPort();
		const cleanupFailure = vi
			.fn<() => Promise<void>>()
			.mockRejectedValue(new Error('detach failed'));
		const cleanup = projectionPort(createLease(cleanupPort, cleanupFailure));
		await expect(
			new DesktopTerminalOutputReader({ projection: cleanup }).read(
				presentationId,
				3,
				new AbortController().signal,
			),
		).resolves.toEqual({ kind: 'cleanup-failed' });
		expect(cleanup.isPublicationTargetCurrent).not.toHaveBeenCalled();
	});

	it('preserves cleanup failure from a router that retires during attachment opening', async () => {
		const projection: DesktopTerminalOutputProjectionPort = {
			isPublicationTargetCurrent: vi.fn<
				DesktopTerminalOutputProjectionPort['isPublicationTargetCurrent']
			>(() => true),
			openReadOnly: vi.fn<DesktopTerminalOutputProjectionPort['openReadOnly']>(async () => {
				throw new Error('Terminal projection cleanup failed.', {
					cause: { code: 'terminal-projection-cleanup-failed', retryable: true },
				});
			}),
		};
		const reader = new DesktopTerminalOutputReader({ projection });

		await expect(reader.read(presentationId, 3, new AbortController().signal)).resolves.toEqual({
			kind: 'cleanup-failed',
		});
	});

	it('cancels while opening and releases a lease that arrives afterward', async () => {
		const opening = deferred<DesktopTerminalProjectionLease>();
		const projection: DesktopTerminalOutputProjectionPort = {
			isPublicationTargetCurrent: vi.fn<
				DesktopTerminalOutputProjectionPort['isPublicationTargetCurrent']
			>(() => true),
			openReadOnly: vi.fn<DesktopTerminalOutputProjectionPort['openReadOnly']>(
				() => opening.promise,
			),
		};
		const reader = new DesktopTerminalOutputReader({ projection });
		const controller = new AbortController();
		const reading = reader.read(presentationId, 3, controller.signal);
		controller.abort();

		await expect(reading).resolves.toEqual({ kind: 'cancelled' });
		const dispose = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
		opening.resolve(createLease(new ReadPort(() => undefined), dispose));
		await Promise.resolve();
		await Promise.resolve();
		expect(dispose).toHaveBeenCalledOnce();
	});

	it('bounds an unresponsive projection and still disposes its lease', async () => {
		const dispose = vi.fn<() => Promise<void>>().mockResolvedValue(undefined);
		const projection = projectionPort(createLease(new ReadPort(() => undefined), dispose));
		const reader = new DesktopTerminalOutputReader({ deadlineMilliseconds: 5, projection });

		await expect(reader.read(presentationId, 3, new AbortController().signal)).resolves.toEqual({
			kind: 'unavailable',
		});
		expect(dispose).toHaveBeenCalledOnce();
	});
});

class ReadPort extends EventEmitter {
	readonly #handleCommand: (command: ProjectedTerminalCommand) => void;

	constructor(handleCommand: (command: ProjectedTerminalCommand) => void) {
		super();
		this.#handleCommand = handleCommand;
	}

	close(): void {}

	postMessage(value: unknown): void {
		const command = ProjectedTerminalCommandSchema.parse(value);
		queueMicrotask(() => this.#handleCommand(command));
	}

	publish(value: unknown): void {
		queueMicrotask(() => this.emit('message', { data: value }));
	}

	start(): void {}
}

function respondingPort(): ReadPort {
	const port = new ReadPort((command) => {
		if (command.kind === 'set-projection-demand') {
			port.publish({ frame: encodedFrame(), kind: 'frame' });
			return;
		}
		if (command.kind === 'read-rows') {
			port.publish({ kind: 'rows', rows: encodedRows(command.payload.requestSequence) });
		}
	});
	return port;
}

function projectionPort(
	lease: DesktopTerminalProjectionLease,
	isCurrent: () => boolean = () => true,
): DesktopTerminalOutputProjectionPort & {
	readonly isPublicationTargetCurrent: ReturnType<
		typeof vi.fn<DesktopTerminalOutputProjectionPort['isPublicationTargetCurrent']>
	>;
} {
	return {
		isPublicationTargetCurrent: vi.fn<
			DesktopTerminalOutputProjectionPort['isPublicationTargetCurrent']
		>(() => isCurrent()),
		openReadOnly: vi.fn<DesktopTerminalOutputProjectionPort['openReadOnly']>(async () => lease),
	};
}

function createLease(port: ReadPort, dispose: () => Promise<void>): DesktopTerminalProjectionLease {
	return {
		bootstrap: ProjectedAttachmentBootstrapSchema.parse({
			attachmentId,
			canInput: false,
			canResize: false,
			capabilityRevision: '1',
			creationId,
			hostEpoch,
			terminalId,
			terminalOutcome: null,
			theme,
		}),
		dispose,
		metrics,
		rendererPort: port as unknown as MessagePortMain,
		setInputCapability: vi.fn<DesktopTerminalProjectionLease['setInputCapability']>(),
	};
}

function encodedFrame(): Uint8Array {
	const encoded = encodeTerminalFrame(frame());
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function frame(): TerminalFrame {
	return {
		activeScreen: ActiveScreen.primary,
		attachmentId,
		baseRevision: null,
		cellPixelHeight: 18,
		cellPixelWidth: 9,
		columns: 1,
		currentDirectory: null,
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
		hostEpoch,
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
		rowData: [
			{
				cells: [
					{
						grapheme: 'T',
						protected: false,
						semanticContent: SemanticContent.output,
						style: 0,
						width: CellWidth.narrow,
					},
				],
				index: 0,
				promptClass: RowPromptClass.none,
				wrap: false,
				wrapContinuation: false,
			},
		],
		rows: 1,
		scrollbackRows: 2n,
		sequence: 1n,
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
		title: null,
	};
}

function encodedRows(sequence: string): Uint8Array {
	const encoded = encodeTerminalRowWindow({
		attachmentId,
		columns: 1,
		detail: TerminalRowWindowDetail.text,
		generation: 3n,
		hostEpoch,
		revision: 5n,
		rows: [
			textRow(0n, 'first', true, false),
			textRow(1n, 'second', false, true),
			textRow(2n, 'third', false, false),
		],
		sequence: BigInt(sequence),
		startRow: 0n,
		totalRows: 3n,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function textRow(index: bigint, text: string, wrap: boolean, wrapContinuation: boolean) {
	return {
		index,
		mappings: [{ byteOffset: 0, column: 0, utf16Offset: 0, width: 1 as const }],
		promptClass: RowPromptClass.none,
		text,
		wrap,
		wrapContinuation,
	};
}

function deferred<Value>(): {
	readonly promise: Promise<Value>;
	readonly resolve: (value: Value) => void;
} {
	let resolvePromise: (value: Value) => void = () => undefined;
	const promise = new Promise<Value>((resolve) => {
		resolvePromise = resolve;
	});
	return { promise, resolve: resolvePromise };
}
