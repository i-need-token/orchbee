import { EventEmitter } from 'node:events';

import {
	ActiveScreen,
	CellWidth,
	CreationIdSchema,
	CursorShape,
	decodeTerminalFrame,
	decodeTerminalRowWindow,
	encodeTerminalProjectionBuffer,
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
import {
	ProjectedTerminalEventSchema,
	type ProjectedTerminalEvent,
} from '@orchbee/workbench-client-protocol';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import {
	ArchivedTerminalProjectionService,
	type ArchivedTerminalProjectionServiceDependencies,
} from '../ArchivedTerminalProjectionService';
import type { FileTerminalArchiveStore } from '../FileTerminalArchiveStore';
import { decodeTerminalArchive, encodeTerminalArchive } from '../terminal-archive-document';

const presentationId = terminalPresentationId('presentation-archived');
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

describe('ArchivedTerminalProjectionService', () => {
	it('serves history, search, selection, and copy without mutable capabilities', async () => {
		const archive = createArchive();
		const channels: TestChannel[] = [];
		const writeClipboardText = vi.fn<(text: string) => Promise<void>>().mockResolvedValue();
		const service = new ArchivedTerminalProjectionService({
			createMessageChannel: () => {
				const channel = createChannel();
				channels.push(channel);
				return channel;
			},
			randomAttachmentId: () => '55'.repeat(16),
			store: {
				load: vi.fn<FileTerminalArchiveStore['load']>().mockResolvedValue(archive),
			} as unknown as FileTerminalArchiveStore,
			writeClipboardText,
		});

		const lease = await service.open(presentationId);
		if (lease === null) throw new Error('Expected an archived projection');
		expect(lease.bootstrap).toMatchObject({ canInput: false, canResize: false });
		await expect(lease.setInputCapability(true)).rejects.toThrow('permanently read-only');
		const port = channels[0]?.port1;
		if (port === undefined) throw new Error('Archive channel was not created');
		port.start();

		const initialFrame = nextEvent(port, (event) => (event.kind === 'frame' ? event : null));
		port.postMessage({
			kind: 'set-projection-demand',
			payload: { attachmentId: lease.bootstrap.attachmentId, demand: 'active' },
		});
		const frameEvent = await initialFrame;
		const frame = decodeTerminalFrame(new Uint8Array(frameEvent.frame));
		if (!frame.ok) throw new Error(frame.detail);
		expect(frame.value.scrollbackRows).toBe(2n);
		expect(frame.value.rowData.indices).toHaveLength(2);

		const renderRows = nextEvent(port, (event) => (event.kind === 'rows' ? event : null));
		port.postMessage({
			kind: 'read-rows',
			payload: {
				attachmentId: lease.bootstrap.attachmentId,
				detail: 'render',
				generation: frame.value.generation.toString(),
				maximumBytes: 2 * 1024 * 1024,
				origin: { kind: 'absolute', row: '0' },
				purpose: 'viewport',
				requestSequence: '7',
				revision: frame.value.revision.toString(),
				rowCount: 2,
			},
		});
		const rendered = decodeTerminalRowWindow(new Uint8Array((await renderRows).rows));
		if (!rendered.ok || rendered.value.detail !== TerminalRowWindowDetail.render) {
			throw new Error('Expected archived render rows');
		}
		expect(rendered.value.startRow).toBe(0n);

		const searchRows = nextEvent(port, (event) => (event.kind === 'rows' ? event : null));
		port.postMessage({
			kind: 'read-rows',
			payload: {
				attachmentId: lease.bootstrap.attachmentId,
				detail: 'text',
				generation: frame.value.generation.toString(),
				maximumBytes: 2 * 1024 * 1024,
				origin: { kind: 'absolute', row: '0' },
				purpose: 'search',
				requestSequence: '8',
				revision: frame.value.revision.toString(),
				rowCount: 4,
			},
		});
		const searched = decodeTerminalRowWindow(new Uint8Array((await searchRows).rows));
		if (!searched.ok || searched.value.detail !== TerminalRowWindowDetail.text) {
			throw new Error('Expected archived text rows');
		}
		expect(searched.value.rows.map((row) => row.text)).toEqual([
			'history-a',
			'history-b',
			'viewport-a',
			'viewport-b',
		]);

		const selectionChanged = nextEvent(port, (event) =>
			event.kind === 'selection-changed' ? event : null,
		);
		port.postMessage({
			kind: 'selection-command',
			payload: {
				attachmentId: lease.bootstrap.attachmentId,
				command: 'select-all',
				generation: frame.value.generation.toString(),
				revision: frame.value.revision.toString(),
			},
		});
		expect((await selectionChanged).event.state).toBe('active');

		const copied = nextEvent(port, (event) => (event.kind === 'selection-copied' ? event : null));
		port.postMessage({
			kind: 'selection-copy',
			payload: { attachmentId: lease.bootstrap.attachmentId, requestSequence: '9' },
		});
		expect((await copied).byteLength).toBeGreaterThan(0);
		expect(writeClipboardText).toHaveBeenCalledWith('history-a\nhistory-b\nviewport-a\nviewport-b');

		const rejectedInput = nextEvent(port, (event) => (event.kind === 'failure' ? event : null));
		port.postMessage({
			kind: 'focus-input',
			payload: {
				attachmentId: lease.bootstrap.attachmentId,
				capabilityRevision: '1',
				focused: true,
			},
		});
		expect((await rejectedInput).failure).toMatchObject({
			code: 'terminal-archive-read-only',
			operation: 'input',
			retryable: false,
		});

		await lease.dispose();
		await service.dispose();
	});

	it('falls back to no projection when the local archive is missing', async () => {
		const service = new ArchivedTerminalProjectionService({
			createMessageChannel: () => createChannel(),
			randomAttachmentId: () => '55'.repeat(16),
			store: {
				load: vi.fn<FileTerminalArchiveStore['load']>().mockResolvedValue(null),
			} as unknown as FileTerminalArchiveStore,
			writeClipboardText:
				vi.fn<ArchivedTerminalProjectionServiceDependencies['writeClipboardText']>(),
		});
		expect(await service.open(presentationId)).toBeNull();
	});
});

interface TestChannel {
	readonly port1: TestPort;
	readonly port2: TestPort;
}

class TestPort extends EventEmitter {
	peer: TestPort | undefined;
	closed = false;

	close(): void {
		if (this.closed) return;
		this.closed = true;
		this.emit('close');
	}

	postMessage(value: unknown): void {
		if (this.closed) throw new Error('Port is closed');
		queueMicrotask(() => this.peer?.emit('message', { data: value }));
	}

	start(): void {}
}

function createChannel(): TestChannel {
	const port1 = new TestPort();
	const port2 = new TestPort();
	port1.peer = port2;
	port2.peer = port1;
	return { port1, port2 };
}

function nextEvent<Event extends ProjectedTerminalEvent>(
	port: TestPort,
	select: (event: ProjectedTerminalEvent) => Event | null,
): Promise<Event> {
	return new Promise((resolve) => {
		const listener = (message: { readonly data: unknown }): void => {
			const parsed = ProjectedTerminalEventSchema.safeParse(message.data);
			if (!parsed.success) return;
			const selected = select(parsed.data);
			if (selected === null) return;
			port.off('message', listener);
			resolve(selected);
		};
		port.on('message', listener);
	});
}

function createArchive() {
	const frame = encodeTerminalProjectionBuffer(createFrame(2n, ['viewport-a', 'viewport-b']));
	const historyFrame = createFrame(0n, ['history-a', 'history-b']);
	const history = encodeTerminalRowWindow({
		attachmentId,
		columns: 2,
		detail: TerminalRowWindowDetail.render,
		frame: historyFrame,
		generation: 3n,
		hostEpoch: identity.hostEpoch,
		revision: 5n,
		sequence: 1n,
		startRow: 0n,
		totalRows: 4n,
	});
	if (!frame.ok || !history.ok) throw new Error('Archive fixture failed to encode');
	const encoded = encodeTerminalArchive({
		frame: frame.value,
		identity,
		presentationId,
		rowWindows: [history.value],
		theme,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	const archive = decodeTerminalArchive(encoded.value);
	if (!archive.ok) throw new Error(archive.detail);
	return archive.value;
}

function createFrame(scrollbackRows: bigint, rows: readonly string[]) {
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
			row: 1,
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
		rowData: rows.map((text, index) => ({
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
		rows: 2,
		scrollbackRows,
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
		title: 'archived terminal',
	};
}
