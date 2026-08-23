import { mkdtemp, readdir, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import {
	ActiveScreen,
	CellWidth,
	CreationIdSchema,
	CursorShape,
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
	type TerminalThemeState,
} from '@orchbee/terminal-protocol';
import {
	terminalPresentationId,
	type TerminalPresentationId,
} from '@orchbee/workbench/terminal-presentation';
import { afterEach, describe, expect, it } from 'vitest';

import { FileTerminalArchiveStore } from '../FileTerminalArchiveStore';
import {
	decodeTerminalArchive,
	encodeTerminalArchive,
	type TerminalArchiveWrite,
} from '../terminal-archive-document';

const attachmentId = '11'.repeat(16);
const identity: TerminalIdentity = {
	creationId: CreationIdSchema.parse('22'.repeat(16)),
	hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
	terminalId: TerminalIdSchema.parse('44'.repeat(16)),
};
const theme: TerminalThemeState = TerminalThemeStateSchema.parse({
	operationId: null,
	revision: '0',
	terminal: {
		colorScheme: 'dark',
		defaultBackgroundRgb: 0x10_10_10,
		defaultCursorRgb: null,
		defaultForegroundRgb: 0xee_ee_ee,
		palette: Array.from({ length: 256 }, (_, index) => index),
	},
});
const style = {
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
} as const;
const interaction = {
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
} as const;

const temporaryDirectories: string[] = [];

afterEach(async () => {
	await Promise.all(
		temporaryDirectories
			.splice(0)
			.map(async (directory) => rm(directory, { force: true, recursive: true })),
	);
});

describe('terminal archive document', () => {
	it('round-trips a bounded final viewport with contiguous retained history', () => {
		const write = createArchiveWrite(terminalPresentationId('presentation-one'));
		const encoded = encodeTerminalArchive(write);
		if (!encoded.ok) throw new Error(encoded.detail);
		const decoded = decodeTerminalArchive(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);

		expect(decoded.value.presentationId).toBe(write.presentationId);
		expect(decoded.value.identity).toEqual(identity);
		expect(decoded.value.retainedRows).toBe(4);
		expect(decoded.value.sourceStartRow).toBe(7n);
		expect(decoded.value.rowWindows).toHaveLength(1);
		expect(decoded.value.frame.scrollbackRows).toBe(9n);
	});

	it('rejects corrupted content and non-contiguous history', () => {
		const write = createArchiveWrite(terminalPresentationId('presentation-one'));
		const encoded = encodeTerminalArchive(write);
		if (!encoded.ok) throw new Error(encoded.detail);
		const corrupted = encoded.value.slice();
		const lastByte = corrupted[corrupted.byteLength - 1];
		if (lastByte === undefined) throw new Error('Archive fixture is empty');
		corrupted[corrupted.byteLength - 1] = lastByte ^ 0xff;
		expect(decodeTerminalArchive(corrupted)).toMatchObject({ ok: false });
		const corruptedMetadata = encoded.value.slice();
		const metadataView = Buffer.from(
			corruptedMetadata.buffer,
			corruptedMetadata.byteOffset,
			corruptedMetadata.byteLength,
		);
		const presentationOffset = metadataView.indexOf('presentation-one', 'utf8');
		if (presentationOffset < 0) throw new Error('Archive header fixture is missing');
		corruptedMetadata.set(new TextEncoder().encode('presentation-two'), presentationOffset);
		expect(decodeTerminalArchive(corruptedMetadata)).toMatchObject({ ok: false });

		const foreignHistory = createRenderWindow(6n, ['old-1', 'old-2']);
		expect(encodeTerminalArchive({ ...write, rowWindows: [foreignHistory] })).toMatchObject({
			ok: false,
		});
	});
});

describe('FileTerminalArchiveStore', () => {
	it('atomically replaces private archives and returns null for corrupt data', async () => {
		const directory = await createTemporaryDirectory();
		const store = new FileTerminalArchiveStore(directory, {
			now: () => new Date('2026-08-23T12:00:00.000Z'),
			randomToken: () => 'fixed-token',
		});
		const presentationId = terminalPresentationId('presentation-one');
		await store.put(createArchiveWrite(presentationId, ['viewport-a', 'viewport-b']));
		await store.put(createArchiveWrite(presentationId, ['replacement-a', 'replacement-b']));

		const loaded = await store.load(presentationId);
		expect(loaded?.frame.title).toBe('replacement-a/ replacement-b');
		const files = await readdir(directory);
		expect(files).toHaveLength(1);
		expect(files[0]).toMatch(/\.obta$/u);
		const directoryMode = (await stat(directory)).mode & 0o777;
		const archiveMode = (await stat(path.join(directory, files[0] ?? ''))).mode & 0o777;
		expect(directoryMode).toBe(process.platform === 'win32' ? directoryMode : 0o700);
		expect(archiveMode).toBe(process.platform === 'win32' ? archiveMode : 0o600);

		await writeFile(path.join(directory, files[0] ?? ''), new Uint8Array([1, 2, 3]));
		expect(await store.load(presentationId)).toBeNull();
		expect(await readdir(directory)).toEqual([]);
	});

	it('evicts the least recently used archive and reconciles orphaned files', async () => {
		const firstId = terminalPresentationId('presentation-one');
		const secondId = terminalPresentationId('presentation-two');
		const firstWrite = createArchiveWrite(firstId);
		const secondWrite = createArchiveWrite(secondId);
		const firstEncoded = encodeTerminalArchive(firstWrite);
		const secondEncoded = encodeTerminalArchive(secondWrite);
		if (!firstEncoded.ok || !secondEncoded.ok) throw new Error('Archive fixture failed');
		let clock = new Date('2026-08-23T12:00:00.000Z');
		const directory = await createTemporaryDirectory();
		const store = new FileTerminalArchiveStore(directory, {
			maximumStoreBytes: firstEncoded.value.byteLength + secondEncoded.value.byteLength - 1,
			now: () => clock,
			randomToken: (() => {
				let sequence = 0;
				return () => `token-${++sequence}`;
			})(),
		});
		await store.put(firstWrite);
		clock = new Date('2026-08-23T12:01:00.000Z');
		await store.put(secondWrite);
		expect(await store.load(firstId)).toBeNull();
		expect(await store.load(secondId)).not.toBeNull();

		const existing = (await readdir(directory))[0];
		if (existing === undefined) throw new Error('Expected retained archive fixture');
		await writeFile(path.join(directory, '.abandoned.tmp'), 'partial');
		const misnamed = `${'f'.repeat(64)}.obta`;
		await writeFile(path.join(directory, misnamed), secondEncoded.value);
		await store.reconcile(new Set([secondId]));
		expect(await readdir(directory)).toEqual([existing]);

		await store.reconcile(new Set<TerminalPresentationId>());
		expect(await readdir(directory)).toEqual([]);
	});
});

async function createTemporaryDirectory(): Promise<string> {
	const directory = await mkdtemp(path.join(tmpdir(), 'orchbee-terminal-archive-'));
	temporaryDirectories.push(directory);
	return directory;
}

function createArchiveWrite(
	presentationId: TerminalPresentationId,
	viewport = ['viewport-a', 'viewport-b'],
): TerminalArchiveWrite {
	const frame = encodeTerminalProjectionBuffer(createFrame(9n, viewport));
	if (!frame.ok) throw new Error(frame.detail);
	return {
		frame: frame.value,
		identity,
		presentationId,
		rowWindows: [createRenderWindow(7n, ['history-a', 'history-b'])],
		theme,
	};
}

function createRenderWindow(startRow: bigint, rows: readonly string[]): Uint8Array {
	const frame = createFrame(startRow, rows);
	const encoded = encodeTerminalRowWindow({
		attachmentId,
		columns: 2,
		detail: TerminalRowWindowDetail.render,
		frame,
		generation: 3n,
		hostEpoch: identity.hostEpoch,
		revision: 5n,
		sequence: 4n,
		startRow,
		totalRows: 11n,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
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
			row: rows.length - 1,
			shape: CursorShape.block,
			visible: true,
			wideTail: false,
		},
		defaultColors: {
			backgroundRgb: 0x10_10_10,
			cursorRgb: null,
			foregroundRgb: 0xee_ee_ee,
		},
		extensions: [],
		generation: 3n,
		hostEpoch: identity.hostEpoch,
		interaction,
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
		rows: rows.length,
		scrollbackRows,
		sequence: 4n,
		styles: [style],
		themeRevision: 0n,
		title: `${rows.join('/ ')}`,
	};
}
