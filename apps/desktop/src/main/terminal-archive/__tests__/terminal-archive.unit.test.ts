import { createHash } from 'node:crypto';
import { mkdtemp, readdir, rm, stat, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';

import {
	ActiveScreen,
	AttachmentIdSchema,
	CellWidth,
	CreationIdSchema,
	CursorShape,
	decodeTerminalProjectionBuffer,
	decodeTerminalRowWindow,
	encodeTerminalFrame,
	encodeTerminalGraphics,
	encodeTerminalProjectionBuffer,
	encodeTerminalRowWindow,
	HostEpochSchema,
	materializeTerminalProjectionFrame,
	PROTOCOL_LIMITS,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalFrameKind,
	TerminalGraphicsUpdateKind,
	TerminalIdSchema,
	TERMINAL_ROW_WINDOW_HEADER_BYTES,
	TerminalRowWindowDetail,
	TerminalThemeStateSchema,
	UnderlineKind,
	type TerminalFrame,
	type TerminalIdentity,
	type TerminalThemeState,
} from '@orchbee/terminal-protocol';
import {
	terminalPresentationId,
	type TerminalPresentationId,
} from '@orchbee/workbench/terminal-presentation';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { ArchivedTerminalProjectionService } from '../ArchivedTerminalProjectionService';
import { FileTerminalArchiveStore } from '../FileTerminalArchiveStore';
import {
	decodeTerminalArchive,
	encodeTerminalArchive,
	terminalArchiveGraphicsForRows,
	type TerminalArchiveWrite,
} from '../terminal-archive-document';

const attachmentId = AttachmentIdSchema.parse('11'.repeat(16));
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
} as const;

interface CellPixelGeometry {
	readonly cellPixelHeight: number;
	readonly cellPixelWidth: number;
}

const matchingCellPixelGeometry: CellPixelGeometry = {
	cellPixelHeight: 18,
	cellPixelWidth: 9,
};
const mismatchedCellPixelGeometries = [
	{ cellPixelHeight: 18, cellPixelWidth: 10, dimension: 'cellPixelWidth' },
	{ cellPixelHeight: 19, cellPixelWidth: 9, dimension: 'cellPixelHeight' },
] as const;

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

	it('round-trips graphics pixels and re-roots one placement spanning the history seam', () => {
		const write = createGraphicsArchiveWrite(
			terminalPresentationId('presentation-graphics-round-trip'),
		);
		const encoded = encodeTerminalArchive(write);
		if (!encoded.ok) throw new Error(encoded.detail);
		const decoded = decodeTerminalArchive(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);

		expect(Array.from(decoded.value.graphics.images[0]?.data ?? [])).toEqual(
			Array.from(graphicsPixels),
		);
		expect(decoded.value.graphics.placements).toMatchObject([{ row: -1 }]);
		expect(decoded.value.rowWindows[0]?.graphics.placements).toMatchObject([{ row: 1 }]);

		const materialized = terminalArchiveGraphicsForRows(decoded.value, 1n, 2);
		if (materialized === null) throw new Error('Expected archived graphics to materialize');
		expect(materialized.animationTick).toBe(17n);
		expect(materialized.resourceRevision).toBe(9n);
		expect(materialized.images).toHaveLength(1);
		expect(Array.from(materialized.images[0]?.data ?? [])).toEqual(Array.from(graphicsPixels));
		expect(materialized.placements).toHaveLength(1);
		expect(materialized.placements[0]).toMatchObject({
			offsetY: 9,
			row: 0,
			z: -0x4000_0000,
		});
	});

	it.each(mismatchedCellPixelGeometries)(
		'rejects a render row window mismatched only in $dimension at encode admission',
		(geometry) => {
			const write = createArchiveWrite(terminalPresentationId('presentation-one'));
			const mismatchedWindow = createRenderWindow(7n, ['history-a', 'history-b'], geometry);
			expectDecodedRenderWindowGeometry(mismatchedWindow, geometry);

			expect(encodeTerminalArchive({ ...write, rowWindows: [mismatchedWindow] })).toEqual({
				detail: 'Terminal archive row-window metadata is inconsistent',
				ok: false,
			});
		},
	);

	it.each(mismatchedCellPixelGeometries)(
		'rejects a content-digest-valid $dimension mismatch at decode and load admission',
		async (geometry) => {
			const presentationId = terminalPresentationId(`presentation-${geometry.dimension}`);
			const write = createArchiveWrite(presentationId);
			const mismatchedWindow = createRenderWindow(7n, ['history-a', 'history-b'], geometry);
			expectDecodedRenderWindowGeometry(mismatchedWindow, geometry);
			const encoded = archiveWithReplacedRowWindow(write, mismatchedWindow);

			expect(decodeTerminalArchive(encoded)).toEqual({
				detail: 'Terminal archive row-window metadata is inconsistent',
				ok: false,
			});

			const directory = await createTemporaryDirectory();
			const store = new FileTerminalArchiveStore(directory);
			await store.put(write);
			const archiveName = (await readdir(directory))[0];
			if (archiveName === undefined) throw new Error('Expected a stored archive fixture');
			await writeFile(path.join(directory, archiveName), encoded);
			expect(await store.load(presentationId)).toBeNull();
			expect(await readdir(directory)).toEqual([]);
		},
	);

	it('rejects 1,025 archive-wide styles while admitting the exact 1,024-style boundary', () => {
		const boundary = createStyleCardinalityArchive(
			terminalPresentationId('presentation-style-boundary'),
			PROTOCOL_LIMITS.maxFrameStyles,
		);
		const counterexample = createStyleCardinalityArchive(
			terminalPresentationId('presentation-style-counterexample'),
			PROTOCOL_LIMITS.maxFrameStyles + 1,
		);
		expectStrictAndIndividuallyRepresentableParts(counterexample);

		const admitted = encodeTerminalArchive(boundary);
		expect(admitted).toMatchObject({ ok: true });
		if (!admitted.ok) throw new Error(admitted.detail);
		expect(decodeTerminalArchive(admitted.value)).toMatchObject({ ok: true });
		expect(encodeTerminalArchive(counterexample)).toEqual({
			detail: 'Terminal archive exceeds archived projection representability limits',
			ok: false,
		});
	});

	it('accounts for metadata, graphemes, and hyperlinks at the 4,096-string boundary', () => {
		const boundary = createStringCardinalityArchive(
			terminalPresentationId('presentation-string-boundary'),
			PROTOCOL_LIMITS.maxFrameStrings - 4,
		);
		const counterexample = createStringCardinalityArchive(
			terminalPresentationId('presentation-string-counterexample'),
			PROTOCOL_LIMITS.maxFrameStrings - 3,
		);
		expectStrictAndIndividuallyRepresentableParts(counterexample);

		const admitted = encodeTerminalArchive(boundary);
		expect(admitted).toMatchObject({ ok: true });
		if (!admitted.ok) throw new Error(admitted.detail);
		expect(decodeTerminalArchive(admitted.value)).toMatchObject({ ok: true });
		expect(encodeTerminalArchive(counterexample)).toEqual({
			detail: 'Terminal archive exceeds archived projection representability limits',
			ok: false,
		});
	});

	it('selects the deduplicated hyperlink-byte maximum under the expanded render-window budget', () => {
		const boundary = createRenderWindowMaximumArchive(
			terminalPresentationId('presentation-render-window-boundary'),
			0,
		);
		const counterexample = createRenderWindowMaximumArchive(
			terminalPresentationId('presentation-render-window-counterexample'),
			1,
		);
		expectStrictAndIndividuallyRepresentableParts(counterexample.write, 1);
		expect(RENDER_ESTIMATOR_COLUMNS * RENDER_ESTIMATOR_SHARED_HYPERLINK_BYTES).toBeGreaterThan(
			RENDER_ESTIMATOR_SAFE_DISTINCT_HYPERLINK_BYTES,
		);
		expect(RENDER_ESTIMATOR_SHARED_HYPERLINK_BYTES).toBeLessThan(
			RENDER_ESTIMATOR_SAFE_DISTINCT_HYPERLINK_BYTES,
		);

		const boundaryWindow = encodeRenderWindowMaximumCandidate(boundary.finalFrame);
		const counterexampleWindow = encodeRenderWindowMaximumCandidate(counterexample.finalFrame);
		expect(boundaryWindow).toMatchObject({ ok: true });
		expect(counterexampleWindow).toMatchObject({ ok: true });
		if (!boundaryWindow.ok || !counterexampleWindow.ok) {
			throw new Error('Expected both expanded-budget render windows to encode');
		}
		expect(counterexampleWindow.value.byteLength).toBe(boundaryWindow.value.byteLength + 1);
		expect(counterexampleWindow.value.byteLength).toBeLessThan(PROTOCOL_LIMITS.maxRowWindowBytes);
		expect(encodeTerminalFrame(counterexample.finalFrame)).toMatchObject({ ok: true });

		const admitted = encodeTerminalArchive(boundary.write);
		expect(admitted).toMatchObject({ ok: true });
		if (!admitted.ok) throw new Error(admitted.detail);
		expect(decodeTerminalArchive(admitted.value)).toMatchObject({ ok: true });
		expect(encodeTerminalArchive(counterexample.write)).toMatchObject({ ok: true });
	}, 10_000);

	it.each([
		{
			kind: 'wrap/wrap-continuation',
			seam: 'wrap' as const,
			startRow: 4n,
			violation: 'Adjacent row wrap flags must agree',
		},
		{
			kind: 'spacer-head/following-wide-row',
			seam: 'spacer-head' as const,
			startRow: 5n,
			violation: 'A row after spacer-head must begin with the wrapped wide cell',
		},
	])(
		'rejects a later strict-valid $kind seam while admitting its legal boundary',
		({ seam, startRow, violation }) => {
			const presentationId = terminalPresentationId(`presentation-${seam}-seam`);
			const legal = createStructuralSeamArchive(presentationId, seam, true);
			const counterexample = createStructuralSeamArchive(presentationId, seam, false);
			expectStrictAndIndividuallyRepresentableParts(counterexample, 6);
			expect(materializeStructuralSeam(counterexample, startRow)).toMatchObject({
				detail: expect.stringContaining(violation),
				ok: false,
			});

			expect(encodeTerminalArchive(counterexample)).toEqual({
				detail: 'Terminal archive exceeds archived projection representability limits',
				ok: false,
			});
			const encoded = encodeTerminalArchive(legal);
			expect(encoded).toMatchObject({ ok: true });
			if (!encoded.ok) throw new Error(encoded.detail);
			expect(decodeTerminalArchive(encoded.value)).toMatchObject({ ok: true });
			expect(materializeStructuralSeam(legal, startRow)).toMatchObject({ ok: true });
		},
	);

	it('rejects and unlinks a digest-valid legacy structural seam before projection exposure', async () => {
		const presentationId = terminalPresentationId('presentation-legacy-structural-seam');
		const legal = createStructuralSeamArchive(presentationId, 'wrap', true);
		const counterexample = createStructuralSeamArchive(presentationId, 'wrap', false);
		const legacy = archiveWithReplacedRowWindows(legal, counterexample.rowWindows);

		expect(decodeTerminalArchive(legacy)).toEqual({
			detail: 'Terminal archive exceeds archived projection representability limits',
			ok: false,
		});

		const directory = await createTemporaryDirectory();
		const store = new FileTerminalArchiveStore(directory);
		await writeStoredArchive(store, legal, directory, legacy);
		expect(await store.load(presentationId)).toBeNull();
		expect(await readdir(directory)).toEqual([]);

		await writeStoredArchive(store, legal, directory, legacy);
		const createMessageChannel = vi.fn<() => never>(() => {
			throw new Error('Invalid archive must not allocate a projection channel');
		});
		const service = new ArchivedTerminalProjectionService({
			createMessageChannel,
			randomAttachmentId: () => '55'.repeat(16),
			store,
			writeClipboardText: vi.fn<(text: string) => Promise<void>>().mockResolvedValue(),
		});
		expect(await service.open(presentationId)).toBeNull();
		expect(createMessageChannel).not.toHaveBeenCalled();
		expect(await readdir(directory)).toEqual([]);
	});

	it.each([
		{
			createBaseline: (presentationId: TerminalPresentationId) =>
				createStyleCardinalityArchive(presentationId, PROTOCOL_LIMITS.maxFrameStyles + 1, false),
			createCounterexample: (presentationId: TerminalPresentationId) =>
				createStyleCardinalityArchive(presentationId, PROTOCOL_LIMITS.maxFrameStyles + 1),
			kind: 'style',
		},
		{
			createBaseline: (presentationId: TerminalPresentationId) =>
				createStringCardinalityArchive(presentationId, PROTOCOL_LIMITS.maxFrameStrings - 4),
			createCounterexample: (presentationId: TerminalPresentationId) =>
				createStringCardinalityArchive(presentationId, PROTOCOL_LIMITS.maxFrameStrings - 3),
			kind: 'string',
		},
	])(
		'rejects and unlinks a digest-valid legacy $kind archive before projection exposure',
		async ({ createBaseline, createCounterexample, kind }) => {
			const presentationId = terminalPresentationId(`presentation-legacy-${kind}`);
			const baseline = createBaseline(presentationId);
			const counterexample = createCounterexample(presentationId);
			const legacy = archiveWithReplacedRowWindows(baseline, counterexample.rowWindows);

			expect(decodeTerminalArchive(legacy)).toEqual({
				detail: 'Terminal archive exceeds archived projection representability limits',
				ok: false,
			});

			const directory = await createTemporaryDirectory();
			const store = new FileTerminalArchiveStore(directory);
			await writeStoredArchive(store, baseline, directory, legacy);
			expect(await store.load(presentationId)).toBeNull();
			expect(await readdir(directory)).toEqual([]);

			await writeStoredArchive(store, baseline, directory, legacy);
			const createMessageChannel = vi.fn<() => never>(() => {
				throw new Error('Invalid archive must not allocate a projection channel');
			});
			const service = new ArchivedTerminalProjectionService({
				createMessageChannel,
				randomAttachmentId: () => '55'.repeat(16),
				store,
				writeClipboardText: vi.fn<(text: string) => Promise<void>>().mockResolvedValue(),
			});
			expect(await service.open(presentationId)).toBeNull();
			expect(createMessageChannel).not.toHaveBeenCalled();
			expect(await readdir(directory)).toEqual([]);
		},
	);

	it('rejects truncated archives, corrupted content, and non-contiguous history', () => {
		expect(decodeTerminalArchive(new Uint8Array([0x4f, 0x42, 0x54, 0x41]))).toMatchObject({
			ok: false,
		});
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
		graphics: emptyGraphicsBytes,
		identity,
		presentationId,
		rowWindows: [createRenderWindow(7n, ['history-a', 'history-b'])],
		theme,
	};
}

const graphicsPixels = new Uint8Array([
	0xff, 0x00, 0x00, 0xff, 0x00, 0xff, 0x00, 0xff, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0x80,
]);

function createGraphicsSnapshot(row: number) {
	return {
		animationTick: 17n,
		images: [
			{
				data: graphicsPixels,
				format: 'rgba',
				generation: 3n,
				height: 2,
				id: 7,
				width: 2,
			},
		],
		kind: TerminalGraphicsUpdateKind.snapshot,
		placements: [
			{
				column: 1,
				destinationHeight: 27,
				destinationWidth: 18,
				imageId: 7,
				internalId: true,
				offsetX: 2,
				offsetY: 9,
				placementId: 13,
				row,
				sourceHeight: 2,
				sourceWidth: 2,
				sourceX: 0,
				sourceY: 0,
				z: -0x4000_0000,
			},
		],
		resourceRevision: 9n,
	} as const;
}

function createGraphicsArchiveWrite(presentationId: TerminalPresentationId): TerminalArchiveWrite {
	const frame = encodeTerminalProjectionBuffer(createFrame(9n, ['viewport-a', 'viewport-b']));
	const viewportGraphics = encodeTerminalGraphics(createGraphicsSnapshot(-1));
	const historyFrame = createFrame(7n, ['history-a', 'history-b']);
	const historyWindow = encodeTerminalRowWindow({
		attachmentId,
		columns: 2,
		detail: TerminalRowWindowDetail.render,
		frame: historyFrame,
		graphics: createGraphicsSnapshot(1),
		generation: 3n,
		hostEpoch: identity.hostEpoch,
		revision: 5n,
		sequence: 4n,
		startRow: 7n,
		totalRows: 11n,
	});
	if (!frame.ok || !viewportGraphics.ok || !historyWindow.ok) {
		throw new Error('Graphics archive fixture failed to encode');
	}
	return {
		frame: frame.value,
		graphics: viewportGraphics.value,
		identity,
		presentationId,
		rowWindows: [historyWindow.value],
		theme,
	};
}

function createRenderWindow(
	startRow: bigint,
	rows: readonly string[],
	geometry: CellPixelGeometry = matchingCellPixelGeometry,
): Uint8Array {
	const frame = createFrame(startRow, rows, geometry);
	const encoded = encodeTerminalRowWindow({
		attachmentId,
		columns: 2,
		detail: TerminalRowWindowDetail.render,
		frame,
		graphics: emptyGraphics,
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

function createFrame(
	scrollbackRows: bigint,
	rows: readonly string[],
	geometry: CellPixelGeometry = matchingCellPixelGeometry,
) {
	return {
		activeScreen: ActiveScreen.primary,
		attachmentId,
		baseRevision: null,
		cellPixelHeight: geometry.cellPixelHeight,
		cellPixelWidth: geometry.cellPixelWidth,
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

type StructuralSeamKind = 'spacer-head' | 'wrap';

interface StructuralArchiveRow {
	readonly cells: TerminalFrame['rowData'][number]['cells'];
	readonly promptClass: RowPromptClass;
	readonly wrap: boolean;
	readonly wrapContinuation: boolean;
}

function createStructuralSeamArchive(
	presentationId: TerminalPresentationId,
	seam: StructuralSeamKind,
	legal: boolean,
): TerminalArchiveWrite {
	const rows = Array.from({ length: 8 }, () => structuralPlainRow());
	if (seam === 'wrap') {
		rows[5] = structuralPlainRow({ wrapContinuation: legal ? false : true });
	} else {
		rows[5] = structuralSpacerHeadRow();
		rows[6] = legal
			? structuralWideContinuationRow()
			: structuralPlainRow({ wrapContinuation: true });
	}
	const rowWindows = rows.slice(0, 6).map((row, index) => {
		const sequence = BigInt(index + 1);
		const frame = createStructuralFrame(BigInt(index), [row], sequence);
		const encoded = encodeTerminalRowWindow({
			attachmentId,
			columns: 4,
			detail: TerminalRowWindowDetail.render,
			frame,
			graphics: emptyGraphics,
			generation: 3n,
			hostEpoch: identity.hostEpoch,
			revision: 5n,
			sequence,
			startRow: BigInt(index),
			totalRows: 8n,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		return encoded.value;
	});
	const finalFrame = encodeTerminalProjectionBuffer(createStructuralFrame(6n, rows.slice(6), 7n));
	if (!finalFrame.ok) throw new Error(finalFrame.detail);
	return {
		frame: finalFrame.value,
		graphics: emptyGraphicsBytes,
		identity,
		presentationId,
		rowWindows,
		theme,
	};
}

function createStructuralFrame(
	scrollbackRows: bigint,
	rows: readonly StructuralArchiveRow[],
	sequence: bigint,
): TerminalFrame {
	return {
		activeScreen: ActiveScreen.primary,
		attachmentId,
		baseRevision: null,
		cellPixelHeight: matchingCellPixelGeometry.cellPixelHeight,
		cellPixelWidth: matchingCellPixelGeometry.cellPixelWidth,
		columns: 4,
		currentDirectory: '/structural-archive',
		cursor: {
			blinking: false,
			column: 0,
			row: 0,
			shape: CursorShape.block,
			visible: false,
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
		rowData: rows.map((row, index) => ({ ...row, index })),
		rows: rows.length,
		scrollbackRows,
		sequence,
		styles: [style],
		themeRevision: 0n,
		title: 'structural archive',
	};
}

function structuralPlainRow(
	overrides: Partial<Pick<StructuralArchiveRow, 'wrap' | 'wrapContinuation'>> = {},
): StructuralArchiveRow {
	return {
		cells: [structuralCell('x'), structuralCell(''), structuralCell(''), structuralCell('')],
		promptClass: RowPromptClass.none,
		wrap: overrides.wrap ?? false,
		wrapContinuation: overrides.wrapContinuation ?? false,
	};
}

function structuralSpacerHeadRow(): StructuralArchiveRow {
	return {
		cells: [
			structuralCell('x'),
			structuralCell(''),
			structuralCell(''),
			structuralCell('', CellWidth.spacerHead),
		],
		promptClass: RowPromptClass.none,
		wrap: true,
		wrapContinuation: false,
	};
}

function structuralWideContinuationRow(): StructuralArchiveRow {
	return {
		cells: [
			structuralCell('x', CellWidth.wide),
			structuralCell('', CellWidth.spacerTail),
			structuralCell(''),
			structuralCell(''),
		],
		promptClass: RowPromptClass.none,
		wrap: false,
		wrapContinuation: true,
	};
}

function structuralCell(
	grapheme: string,
	width: CellWidth = CellWidth.narrow,
): TerminalFrame['rowData'][number]['cells'][number] {
	return {
		grapheme,
		protected: false,
		semanticContent: SemanticContent.output,
		style: 0,
		width,
	};
}

function materializeStructuralSeam(write: TerminalArchiveWrite, startRow: bigint) {
	const metadata = decodeTerminalProjectionBuffer(write.frame);
	if (!metadata.ok) throw new Error(metadata.detail);
	const sources = write.rowWindows.map((encoded) => {
		const window = decodeTerminalRowWindow(encoded);
		if (!window.ok || window.value.detail !== TerminalRowWindowDetail.render) {
			throw new Error('Expected a strict-valid structural archive window');
		}
		return window.value.frame;
	});
	return materializeTerminalProjectionFrame(metadata.value, [...sources, metadata.value], {
		attachmentId,
		revealCursor: false,
		rowCount: 2,
		scrollbackRows: startRow,
		sequence: 1n,
		sourceStartRow: startRow,
	});
}

interface CardinalityCell {
	readonly grapheme: string;
	readonly style: number;
}

const RENDER_ESTIMATOR_COLUMNS = 1_024;
const RENDER_ESTIMATOR_SHARED_HYPERLINK_BYTES = 4_096;
const RENDER_ESTIMATOR_DISTINCT_HYPERLINK_BYTES = 2_014;
const RENDER_ESTIMATOR_LAST_HYPERLINK_BYTES = 2_568;
const RENDER_ESTIMATOR_SAFE_DISTINCT_HYPERLINK_BYTES =
	RENDER_ESTIMATOR_SHARED_HYPERLINK_BYTES +
	(RENDER_ESTIMATOR_COLUMNS - 2) * RENDER_ESTIMATOR_DISTINCT_HYPERLINK_BYTES +
	RENDER_ESTIMATOR_LAST_HYPERLINK_BYTES;

function createRenderWindowMaximumArchive(
	presentationId: TerminalPresentationId,
	lastHyperlinkByteAdjustment: number,
): { readonly finalFrame: TerminalFrame; readonly write: TerminalArchiveWrite } {
	const sharedHyperlink = renderEstimatorHyperlink(0, RENDER_ESTIMATOR_SHARED_HYPERLINK_BYTES);
	const finalHyperlinks = Array.from({ length: RENDER_ESTIMATOR_COLUMNS }, (_value, index) => {
		if (index === 0) return sharedHyperlink;
		const byteLength =
			index === RENDER_ESTIMATOR_COLUMNS - 1
				? RENDER_ESTIMATOR_LAST_HYPERLINK_BYTES + lastHyperlinkByteAdjustment
				: RENDER_ESTIMATOR_DISTINCT_HYPERLINK_BYTES;
		return renderEstimatorHyperlink(index, byteLength);
	});
	const historyFrame = createRenderWindowMaximumFrame(0n, 1n, [sharedHyperlink]);
	const finalFrame = createRenderWindowMaximumFrame(1n, 2n, finalHyperlinks);
	const historyWindow = encodeTerminalRowWindow({
		attachmentId,
		columns: RENDER_ESTIMATOR_COLUMNS,
		detail: TerminalRowWindowDetail.render,
		frame: historyFrame,
		graphics: emptyGraphics,
		generation: historyFrame.generation,
		hostEpoch: identity.hostEpoch,
		revision: historyFrame.revision,
		sequence: historyFrame.sequence,
		startRow: 0n,
		totalRows: 2n,
	});
	const encodedFinalFrame = encodeTerminalProjectionBuffer(finalFrame);
	if (!historyWindow.ok || !encodedFinalFrame.ok) {
		throw new Error('Render-window maximum fixture failed to encode');
	}
	return {
		finalFrame,
		write: {
			frame: encodedFinalFrame.value,
			graphics: emptyGraphicsBytes,
			identity,
			presentationId,
			rowWindows: [historyWindow.value],
			theme,
		},
	};
}

function createRenderWindowMaximumFrame(
	scrollbackRows: bigint,
	sequence: bigint,
	hyperlinks: readonly string[],
): TerminalFrame {
	if (hyperlinks.length !== 1 && hyperlinks.length !== RENDER_ESTIMATOR_COLUMNS) {
		throw new Error('Render-window maximum fixture has invalid hyperlink cardinality');
	}
	return {
		...createCardinalityFrame({
			columns: RENDER_ESTIMATOR_COLUMNS,
			currentDirectory: '',
			rows: [
				Array.from({ length: RENDER_ESTIMATOR_COLUMNS }, (_value, column) => ({
					grapheme: 'X',
					style: hyperlinks.length === 1 ? 0 : column,
				})),
			],
			scrollbackRows,
			sequence,
			styles: hyperlinks.map((hyperlink) => ({ ...style, hyperlink })),
			title: '',
		}),
		currentDirectory: null,
		title: null,
	};
}

function encodeRenderWindowMaximumCandidate(frame: TerminalFrame) {
	return encodeTerminalRowWindow({
		attachmentId,
		columns: frame.columns,
		detail: TerminalRowWindowDetail.render,
		frame,
		graphics: emptyGraphics,
		generation: frame.generation,
		hostEpoch: frame.hostEpoch,
		revision: frame.revision,
		sequence: frame.sequence,
		startRow: frame.scrollbackRows,
		totalRows: 2n,
	});
}

function renderEstimatorHyperlink(index: number, byteLength: number): string {
	const prefix = `https://example.test/render/${index.toString().padStart(4, '0')}/`;
	if (prefix.length > byteLength) {
		throw new Error('Render-window maximum hyperlink length is too small');
	}
	return prefix + 'h'.repeat(byteLength - prefix.length);
}

function createStyleCardinalityArchive(
	presentationId: TerminalPresentationId,
	styleCount: number,
	distinct = true,
): TerminalArchiveWrite {
	const columns = styleCount === PROTOCOL_LIMITS.maxFrameStyles ? 32 : 41;
	const historyRows = styleCount / columns;
	if (!Number.isInteger(historyRows)) throw new Error('Style fixture must fill complete rows');
	const firstWindowRows = Math.floor(historyRows / 2);
	const totalRows = BigInt(historyRows * 2);
	const rowWindows = [
		createStyleWindow(0n, firstWindowRows, 0, columns, totalRows, 1n, distinct),
		createStyleWindow(
			BigInt(firstWindowRows),
			historyRows - firstWindowRows,
			firstWindowRows * columns,
			columns,
			totalRows,
			2n,
			distinct,
		),
	];
	const frame = encodeTerminalProjectionBuffer(
		createCardinalityFrame({
			columns,
			currentDirectory: '/style-archive',
			rows: Array.from({ length: historyRows }, () =>
				Array.from({ length: columns }, () => ({ grapheme: 's', style: 0 })),
			),
			scrollbackRows: BigInt(historyRows),
			styles: [trueColorStyle(0)],
			title: 'style-archive',
		}),
	);
	if (!frame.ok) throw new Error(frame.detail);
	return {
		frame: frame.value,
		graphics: emptyGraphicsBytes,
		identity,
		presentationId,
		rowWindows,
		theme,
	};
}

function createStyleWindow(
	startRow: bigint,
	rowCount: number,
	firstStyle: number,
	columns: number,
	totalRows: bigint,
	sequence: bigint,
	distinct: boolean,
): Uint8Array {
	const cellCount = rowCount * columns;
	const styles = distinct
		? Array.from({ length: cellCount }, (_value, index) => trueColorStyle(firstStyle + index))
		: [trueColorStyle(0)];
	const rows = Array.from({ length: rowCount }, (_value, row) =>
		Array.from({ length: columns }, (_cell, column) => ({
			grapheme: 's',
			style: distinct ? row * columns + column : 0,
		})),
	);
	return encodeCardinalityWindow({
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

function createStringCardinalityArchive(
	presentationId: TerminalPresentationId,
	distinctGraphemeCount: number,
): TerminalArchiveWrite {
	const columns = 63;
	const historyRows = 65;
	const firstWindowRows = 32;
	const totalRows = BigInt(historyRows * 2);
	const stringStyles = hyperlinkStyles();
	const rowWindows = [
		createStringWindow(
			0n,
			firstWindowRows,
			0,
			columns,
			totalRows,
			1n,
			distinctGraphemeCount,
			stringStyles,
		),
		createStringWindow(
			BigInt(firstWindowRows),
			historyRows - firstWindowRows,
			firstWindowRows * columns,
			columns,
			totalRows,
			2n,
			distinctGraphemeCount,
			stringStyles,
		),
	];
	const frame = encodeTerminalProjectionBuffer(
		createCardinalityFrame({
			columns,
			currentDirectory: '/archive-directory',
			rows: Array.from({ length: historyRows }, () =>
				Array.from({ length: columns }, (_, index) => ({
					grapheme: cardinalityGrapheme(0),
					style: index % stringStyles.length,
				})),
			),
			scrollbackRows: BigInt(historyRows),
			styles: stringStyles,
			title: 'archive-title',
		}),
	);
	if (!frame.ok) throw new Error(frame.detail);
	return {
		frame: frame.value,
		graphics: emptyGraphicsBytes,
		identity,
		presentationId,
		rowWindows,
		theme,
	};
}

function createStringWindow(
	startRow: bigint,
	rowCount: number,
	firstCell: number,
	columns: number,
	totalRows: bigint,
	sequence: bigint,
	distinctGraphemeCount: number,
	styles: readonly TerminalFrame['styles'][number][],
): Uint8Array {
	const rows = Array.from({ length: rowCount }, (_value, row) =>
		Array.from({ length: columns }, (_cell, column) => {
			const cell = firstCell + row * columns + column;
			return {
				grapheme: cardinalityGrapheme(cell < distinctGraphemeCount ? cell : 0),
				style: cell % styles.length,
			};
		}),
	);
	return encodeCardinalityWindow({
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

function encodeCardinalityWindow(value: {
	readonly columns: number;
	readonly currentDirectory: string;
	readonly rows: readonly (readonly CardinalityCell[])[];
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
		frame: createCardinalityFrame({
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

function createCardinalityFrame(value: {
	readonly columns: number;
	readonly currentDirectory: string;
	readonly rows: readonly (readonly CardinalityCell[])[];
	readonly scrollbackRows: bigint;
	readonly sequence?: bigint;
	readonly styles: readonly TerminalFrame['styles'][number][];
	readonly title: string;
}): TerminalFrame {
	return {
		activeScreen: ActiveScreen.primary,
		attachmentId,
		baseRevision: null,
		cellPixelHeight: matchingCellPixelGeometry.cellPixelHeight,
		cellPixelWidth: matchingCellPixelGeometry.cellPixelWidth,
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

function trueColorStyle(rgb: number): TerminalFrame['styles'][number] {
	return { ...style, foreground: { kind: TerminalColorKind.rgb, rgb } };
}

function hyperlinkStyles(): readonly TerminalFrame['styles'][number][] {
	return [
		{ ...style, hyperlink: 'archive-link-a' },
		{ ...style, hyperlink: 'archive-link-b' },
	];
}

function cardinalityGrapheme(index: number): string {
	return String.fromCodePoint(0xe000 + index);
}

function expectStrictAndIndividuallyRepresentableParts(
	write: TerminalArchiveWrite,
	expectedWindowCount = 2,
): void {
	const metadata = decodeTerminalProjectionBuffer(write.frame);
	expect(metadata).toMatchObject({ ok: true });
	if (!metadata.ok) throw new Error(metadata.detail);
	expect(write.rowWindows).toHaveLength(expectedWindowCount);
	for (const encodedWindow of write.rowWindows) {
		const projection = decodeTerminalProjectionBuffer(renderWindowProjectionBytes(encodedWindow));
		const window = decodeTerminalRowWindow(encodedWindow);
		expect(projection).toMatchObject({ ok: true });
		expect(window).toMatchObject({ ok: true });
		if (!projection.ok || !window.ok || window.value.detail !== TerminalRowWindowDetail.render) {
			throw new Error('Expected strict-valid render archive fixture');
		}
		const materialized = materializeTerminalProjectionFrame(metadata.value, [window.value.frame], {
			attachmentId,
			revealCursor: false,
			rowCount: window.value.frame.rows,
			scrollbackRows: window.value.startRow,
			sequence: window.value.sequence,
			sourceStartRow: window.value.startRow,
		});
		expect(materialized).toMatchObject({ ok: true });
		if (!materialized.ok) throw new Error(materialized.detail);
		expect(encodeTerminalFrame(materialized.value)).toMatchObject({ ok: true });
	}
}

function expectDecodedRenderWindowGeometry(
	encodedWindow: Uint8Array,
	geometry: CellPixelGeometry,
): void {
	const inner = decodeTerminalProjectionBuffer(renderWindowProjectionBytes(encodedWindow));
	const outer = decodeTerminalRowWindow(encodedWindow);
	expect(inner).toMatchObject({ ok: true });
	expect(outer).toMatchObject({ ok: true });
	if (!inner.ok || !outer.ok || outer.value.detail !== TerminalRowWindowDetail.render) {
		throw new Error('Expected a valid decoded render row window');
	}
	const expectedGeometry = {
		cellPixelHeight: geometry.cellPixelHeight,
		cellPixelWidth: geometry.cellPixelWidth,
	};
	expect(inner.value).toMatchObject(expectedGeometry);
	expect(outer.value.frame).toMatchObject(expectedGeometry);
}

function renderWindowProjectionBytes(encodedWindow: Uint8Array): Uint8Array {
	const view = new DataView(
		encodedWindow.buffer,
		encodedWindow.byteOffset,
		encodedWindow.byteLength,
	);
	const projectionLength = view.getUint32(101, true);
	return encodedWindow.subarray(
		TERMINAL_ROW_WINDOW_HEADER_BYTES,
		TERMINAL_ROW_WINDOW_HEADER_BYTES + projectionLength,
	);
}

function archiveWithReplacedRowWindow(
	write: TerminalArchiveWrite,
	replacement: Uint8Array,
): Uint8Array {
	return archiveWithReplacedRowWindows(write, [replacement]);
}

function archiveWithReplacedRowWindows(
	write: TerminalArchiveWrite,
	replacements: readonly Uint8Array[],
): Uint8Array {
	const admitted = encodeTerminalArchive(write);
	if (!admitted.ok) throw new Error(admitted.detail);
	const encoded = admitted.value.slice();
	const view = new DataView(encoded.buffer, encoded.byteOffset, encoded.byteLength);
	const headerLength = view.getUint32(8, true);
	const headerBytes = encoded.subarray(12, 12 + headerLength);
	const headerText = new TextDecoder().decode(headerBytes);
	const headerValue: unknown = JSON.parse(headerText);
	if (!isRecord(headerValue)) throw new Error('Archive fixture header is invalid');
	const chunkLengths = headerValue['chunkLengths'];
	const contentDigest = headerValue['contentDigest'];
	const frameLength = headerValue['frameLength'];
	const graphicsLength = headerValue['graphicsLength'];
	if (
		typeof contentDigest !== 'string' ||
		typeof frameLength !== 'number' ||
		typeof graphicsLength !== 'number' ||
		!Array.isArray(chunkLengths)
	) {
		throw new Error('Archive fixture header is invalid');
	}
	if (
		chunkLengths.length !== replacements.length ||
		!chunkLengths.every(
			(length, index) => typeof length === 'number' && length === replacements[index]?.byteLength,
		) ||
		frameLength !== write.frame.byteLength ||
		graphicsLength !== write.graphics.byteLength
	) {
		throw new Error('Archive replacement must preserve the fixture layout');
	}
	const digestMetadata = { ...headerValue };
	delete digestMetadata['contentDigest'];
	const replacementDigest = createHash('sha256')
		.update(JSON.stringify(digestMetadata), 'utf8')
		.update(write.frame)
		.update(write.graphics);
	for (const replacement of replacements) replacementDigest.update(replacement);
	const digest = replacementDigest.digest('hex');
	const digestMarker = `"contentDigest":"${contentDigest}"`;
	const markerOffset = headerText.indexOf(digestMarker);
	if (markerOffset < 0) throw new Error('Archive fixture digest is missing');
	const digestTextOffset = markerOffset + '"contentDigest":"'.length;
	const digestByteOffset = new TextEncoder().encode(
		headerText.slice(0, digestTextOffset),
	).byteLength;
	encoded.set(new TextEncoder().encode(digest), 12 + digestByteOffset);
	let payloadOffset = 12 + headerLength + write.frame.byteLength + write.graphics.byteLength;
	for (const replacement of replacements) {
		encoded.set(replacement, payloadOffset);
		payloadOffset += replacement.byteLength;
	}
	return encoded;
}

async function writeStoredArchive(
	store: FileTerminalArchiveStore,
	baseline: TerminalArchiveWrite,
	directory: string,
	encoded: Uint8Array,
): Promise<void> {
	await store.put(baseline);
	const archiveName = (await readdir(directory))[0];
	if (archiveName === undefined) throw new Error('Expected a stored archive fixture');
	await writeFile(path.join(directory, archiveName), encoded);
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}
