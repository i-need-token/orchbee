import {
	ActiveScreen,
	CellWidth,
	CursorShape,
	decodeTerminalFrame,
	encodeTerminalGraphics,
	encodeTerminalFrame,
	encodeTerminalProjectionBuffer,
	parseAttachmentId,
	parseHostEpoch,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalCloseActivityCode,
	TerminalFrameExtensionKind,
	TerminalFrameKind,
	TerminalGraphicsImageFormat,
	TerminalGraphicsUpdateKind,
	UnderlineKind,
	viewTerminalProjectionBuffer,
	type TerminalFrame,
	type TerminalGraphicsPlacement,
} from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import { PackedTerminalStore, ProjectedStorePhase } from '../packed-terminal-store';

function identifier<T>(
	result: { readonly ok: true; readonly value: T } | { readonly ok: false },
): T {
	if (!result.ok) throw new Error('test identifier is invalid');
	return result.value;
}

const hostEpoch = identifier(parseHostEpoch('11111111111111111111111111111111'));
const attachmentId = identifier(parseAttachmentId('22222222222222222222222222222222'));

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
};

function frame(
	kind: 'snapshot' | 'delta',
	sequence: bigint,
	revision: bigint,
	text: readonly [string, string],
	baseRevision: bigint | null = null,
	rows: readonly number[] = [0, 1],
): TerminalFrame {
	return {
		activeScreen: ActiveScreen.primary,
		attachmentId,
		baseRevision,
		cellPixelHeight: 18,
		cellPixelWidth: 9,
		columns: 2,
		cursor: {
			blinking: false,
			column: 0,
			row: 0,
			shape: CursorShape.block,
			visible: true,
			wideTail: false,
		},
		currentDirectory: '/tmp',
		defaultColors: { backgroundRgb: 0, cursorRgb: null, foregroundRgb: 0xff_ff_ff },
		extensions: [],
		generation: 1n,
		hostEpoch,
		interaction,
		kind: kind === 'snapshot' ? TerminalFrameKind.snapshot : TerminalFrameKind.delta,
		revision,
		rowData: rows.map((index) => ({
			cells: [
				{
					grapheme: text[index] ?? '',
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
		scrollbackRows: 4n,
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
		title: 'test',
	};
}

function bytes(value: TerminalFrame): Uint8Array {
	const encoded = encodeTerminalFrame(value);
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function decodedFrame(value: TerminalFrame) {
	const decoded = decodeTerminalFrame(bytes(value));
	if (!decoded.ok) throw new Error(decoded.detail);
	return decoded.value;
}

function commandCompletionExtension(): Uint8Array<ArrayBuffer> {
	const data = new Uint8Array(25);
	const view = new DataView(data.buffer);
	view.setBigUint64(0, 3n, true);
	view.setBigUint64(8, 8_000n, true);
	view.setBigUint64(16, 1_786_947_200_000n, true);
	view.setUint8(24, 7);
	return data;
}

const graphicsPlacement: TerminalGraphicsPlacement = {
	column: 0,
	destinationHeight: 18,
	destinationWidth: 9,
	imageId: 7,
	internalId: false,
	offsetX: 0,
	offsetY: 0,
	placementId: 3,
	row: 0,
	sourceHeight: 1,
	sourceWidth: 1,
	sourceX: 0,
	sourceY: 0,
	z: 0,
};

function graphicsExtension(
	kind: 'snapshot' | 'placements',
	resourceRevision: bigint,
	placements: readonly TerminalGraphicsPlacement[],
	includeImage: boolean,
) {
	const encoded = encodeTerminalGraphics({
		animationTick: resourceRevision,
		images: includeImage
			? [
					{
						data: new Uint8Array([1, 2, 3]),
						format: TerminalGraphicsImageFormat.rgb,
						generation: 1n,
						height: 1,
						id: 7,
						width: 1,
					},
				]
			: [],
		kind:
			kind === 'snapshot'
				? TerminalGraphicsUpdateKind.snapshot
				: TerminalGraphicsUpdateKind.placements,
		placements,
		resourceRevision,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	return {
		data: new Uint8Array(encoded.value),
		kind: TerminalFrameExtensionKind.kittyGraphics,
		required: false,
	} as const;
}

describe('PackedTerminalStore', () => {
	it('retains an OBPB 0.0.1 snapshot as direct typed views without per-cell repacking', () => {
		const encoded = encodeTerminalProjectionBuffer(frame('snapshot', 1n, 1n, ['蜂', 'b']));
		if (!encoded.ok) throw new Error(encoded.detail);
		const viewed = viewTerminalProjectionBuffer(encoded.value);
		if (!viewed.ok) throw new Error(viewed.detail);
		const store = new PackedTerminalStore(hostEpoch, attachmentId);

		const result = store.replaceProjectionBufferSnapshot(viewed.value, 1024 * 1024);

		expect(result).toEqual({
			acknowledgement: { attachmentId, generation: '1', revision: '1', sequence: '1' },
			byteLength: encoded.value.byteLength,
			kind: 'committed',
		});
		expect(store.byteLength).toBe(encoded.value.byteLength);
		expect(store.rows[0]?.widths.buffer).toBe(encoded.value.buffer);
		expect(store.rows[0]?.foregroundColors.buffer).toBe(encoded.value.buffer);
		expect(store.rows.map((row) => row.grapheme(0))).toEqual(['蜂', 'b']);
	});

	it('preserves leading U+FEFF in packed OBPB grapheme and hyperlink ranges', () => {
		const grapheme = '\uFEFFA';
		const hyperlink = '\uFEFFx';
		const projectionFrame = frame('snapshot', 1n, 1n, [grapheme, 'b']);
		const projectionStyle = projectionFrame.styles[0];
		if (projectionStyle === undefined) throw new Error('Projection style is missing');
		const encoded = encodeTerminalProjectionBuffer({
			...projectionFrame,
			styles: [{ ...projectionStyle, hyperlink }],
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const viewed = viewTerminalProjectionBuffer(encoded.value);
		if (!viewed.ok) throw new Error(viewed.detail);
		const store = new PackedTerminalStore(hostEpoch, attachmentId);

		expect(store.replaceProjectionBufferSnapshot(viewed.value, 1024 * 1024).kind).toBe('committed');
		const row = store.rows[0];
		expect(row?.grapheme(0)).toBe(grapheme);
		expect(row?.grapheme(0).codePointAt(0)).toBe(0xfeff);
		expect(row?.hyperlink(0)).toBe(hyperlink);
		expect(row?.hyperlink(0)?.codePointAt(0)).toBe(0xfeff);
		expect(row?.widths[0]).toBe(0);
	});

	it('commits an already-decoded immutable history snapshot without source bytes', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		const snapshot = frame('snapshot', 1n, 1n, ['a', 'b']);
		const result = store.replaceDecodedSnapshot(decodedFrame(snapshot), 1024 * 1024);
		expect(result).toEqual({
			acknowledgement: { attachmentId, generation: '1', revision: '1', sequence: '1' },
			byteLength: expect.any(Number),
			kind: 'committed',
		});
		expect(store.rows.map((row) => row.grapheme(0))).toEqual(['a', 'b']);

		const duplicate = store.apply(bytes(snapshot));
		expect(duplicate).toEqual({
			acknowledgement: { attachmentId, generation: '1', revision: '1', sequence: '1' },
			kind: 'duplicate',
		});
	});

	it('rejects an oversized decoded snapshot without exposing staged rows', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		const result = store.replaceDecodedSnapshot(
			decodedFrame(frame('snapshot', 1n, 1n, ['a', 'b'])),
			0,
		);
		expect(result).toEqual({ kind: 'too-large' });
		expect(store.phase).toBe(ProjectedStorePhase.empty);
		expect(store.rows).toEqual([]);
	});

	it('stages a snapshot and atomically exposes packed stable rows', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		const result = store.apply(bytes(frame('snapshot', 1n, 1n, ['a', 'b'])));
		expect(result).toEqual({
			acknowledgement: { attachmentId, generation: '1', revision: '1', sequence: '1' },
			kind: 'committed',
		});
		expect(store.phase).toBe(ProjectedStorePhase.synchronized);
		expect(store.rows.map((row) => row.grapheme(0))).toEqual(['a', 'b']);
		expect(store.rows[0]?.widths).toBeInstanceOf(Uint8Array);
		expect(store.needsFullRedraw).toBe(true);
	});

	it('atomically retains Kitty resources and applies placement-only deltas', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		const snapshotBytes = bytes({
			...frame('snapshot', 1n, 1n, ['a', 'b']),
			extensions: [graphicsExtension('snapshot', 1n, [graphicsPlacement], true)],
		});

		expect(store.apply(snapshotBytes).kind).toBe('committed');
		snapshotBytes.fill(0);
		expect(store.graphics.resourceRevision).toBe(1n);
		expect([...store.graphics.images]).toEqual([
			[7, expect.objectContaining({ data: new Uint8Array([1, 2, 3]), id: 7 })],
		]);
		expect(store.graphics.placements).toEqual([graphicsPlacement]);
		expect(store.byteLength).toBeGreaterThanOrEqual(3);

		const secondPlacement = { ...graphicsPlacement, column: 1, placementId: 4 };
		const images = store.graphics.images;
		expect(
			store.apply(
				bytes({
					...frame('delta', 2n, 2n, ['', ''], 1n, []),
					extensions: [
						graphicsExtension('placements', 1n, [graphicsPlacement, secondPlacement], false),
					],
				}),
			).kind,
		).toBe('committed');
		expect(store.graphics.images).toBe(images);
		expect(store.graphics.placements).toEqual([graphicsPlacement, secondPlacement]);

		const rejected = store.apply(
			bytes({
				...frame('delta', 3n, 3n, ['x', ''], 2n, [0]),
				extensions: [graphicsExtension('placements', 9n, [graphicsPlacement], false)],
			}),
		);
		expect(rejected).toEqual({
			kind: 'snapshot-required',
			request: { attachmentId, reason: 'base-mismatch', sequence: '3' },
		});
		expect(store.rows[0]?.grapheme(0)).toBe('a');
		expect(store.graphics.placements).toEqual([graphicsPlacement, secondPlacement]);

		expect(
			store.apply(
				bytes({
					...frame('snapshot', 4n, 4n, ['c', 'd']),
					extensions: [graphicsExtension('snapshot', 4n, [], false)],
				}),
			).kind,
		).toBe('committed');
		expect(store.graphics.images.size).toBe(0);
		expect(store.graphics.placements).toEqual([]);
		expect(store.graphics.resourceRevision).toBe(4n);
	});

	it.each([
		{
			expected: 'unknown' as const,
			extensions: [],
			label: 'missing',
		},
		{
			expected: 'idle' as const,
			extensions: [
				{
					data: new Uint8Array([TerminalCloseActivityCode.idle]),
					kind: TerminalFrameExtensionKind.closeActivity,
					required: false,
				},
			],
			label: 'idle',
		},
		{
			expected: 'active' as const,
			extensions: [
				{
					data: new Uint8Array([TerminalCloseActivityCode.active]),
					kind: TerminalFrameExtensionKind.closeActivity,
					required: false,
				},
			],
			label: 'active',
		},
		{
			expected: 'unknown' as const,
			extensions: [
				{
					data: new Uint8Array([TerminalCloseActivityCode.idle, 0]),
					kind: TerminalFrameExtensionKind.closeActivity,
					required: false,
				},
			],
			label: 'malformed',
		},
	])('maps a $label close-activity extension to $expected', ({ expected, extensions }) => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		const snapshot = { ...frame('snapshot', 1n, 1n, ['a', 'b']), extensions };

		expect(store.apply(bytes(snapshot)).kind).toBe('committed');
		expect(store.metadata?.closeActivity).toBe(expected);
	});

	it('maps the current terminal progress extension into projection metadata', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		const active = {
			...frame('snapshot', 1n, 1n, ['a', 'b']),
			extensions: [
				{
					data: new Uint8Array([1, 64]),
					kind: TerminalFrameExtensionKind.progress,
					required: false,
				},
			],
		};
		expect(store.apply(bytes(active)).kind).toBe('committed');
		expect(store.metadata?.progress).toEqual({ state: 'set', value: 64 });

		const removed = {
			...frame('delta', 2n, 2n, ['a', 'b'], 1n),
			extensions: [
				{
					data: new Uint8Array([0, 0xff]),
					kind: TerminalFrameExtensionKind.progress,
					required: false,
				},
			],
		};
		expect(store.apply(bytes(removed)).kind).toBe('committed');
		expect(store.metadata?.progress).toEqual({ state: 'remove', value: null });
	});

	it('maps the latest command completion extension into projection metadata', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		const snapshot: TerminalFrame = {
			...frame('snapshot', 1n, 1n, ['a', 'b']),
			extensions: [
				{
					data: commandCompletionExtension(),
					kind: TerminalFrameExtensionKind.commandCompletion,
					required: false,
				},
			],
		};
		expect(store.apply(bytes(snapshot)).kind).toBe('committed');
		expect(store.metadata?.commandCompletion).toEqual({
			completedAtMilliseconds: 1_786_947_200_000n,
			durationMilliseconds: 8_000n,
			exitCode: 7,
			sequence: 3n,
		});
	});

	it('replaces only declared row backings while preserving row-view identity', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		store.apply(bytes(frame('snapshot', 1n, 1n, ['a', 'b'])));
		store.markRendered();
		const first = store.rows[0];
		const second = store.rows[1];
		const result = store.apply(bytes(frame('delta', 2n, 2n, ['x', ''], 1n, [0])));
		expect(result.kind).toBe('committed');
		expect(store.rows[0]).toBe(first);
		expect(store.rows[1]).toBe(second);
		expect(first?.grapheme(0)).toBe('x');
		expect(second?.grapheme(0)).toBe('b');
		expect(first?.dirty).toBe(true);
		expect(second?.dirty).toBe(false);
	});

	it('decodes a row grapheme once per backing and invalidates the cache on replacement', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		store.apply(bytes(frame('snapshot', 1n, 1n, ['a', 'b'])));
		const row = store.rows[0];
		if (row === undefined) throw new Error('Missing packed row fixture');
		const decode = vi.spyOn(TextDecoder.prototype, 'decode');
		decode.mockClear();

		expect(row.grapheme(0)).toBe('a');
		expect(row.grapheme(0)).toBe('a');
		expect(decode).toHaveBeenCalledOnce();
		decode.mockClear();
		expect(row.grapheme(1)).toBe('');
		expect(row.grapheme(1)).toBe('');
		expect(decode).not.toHaveBeenCalled();

		store.apply(bytes(frame('delta', 2n, 2n, ['x', ''], 1n, [0])));
		decode.mockClear();
		expect(row.grapheme(0)).toBe('x');
		expect(row.grapheme(0)).toBe('x');
		expect(decode).toHaveBeenCalledOnce();
		decode.mockRestore();
	});

	it('retains only the bounded current grid across a long delta stream', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		store.apply(bytes(frame('snapshot', 1n, 1n, ['0', 'b'])));
		store.markRendered();
		const rowViews = [...store.rows];

		for (let value = 2n; value <= 2_001n; value += 1n) {
			const payload = bytes(
				frame('delta', value, value, [(value % 10n).toString(), ''], value - 1n, [0]),
			);
			expect(store.apply(payload).kind).toBe('committed');
			payload.fill(0);
			store.markRendered();
		}

		expect(store.rows).toHaveLength(2);
		expect(store.rows[0]).toBe(rowViews[0]);
		expect(store.rows[1]).toBe(rowViews[1]);
		expect(store.rows[0]?.grapheme(0)).toBe('1');
		expect(store.rows[1]?.grapheme(0)).toBe('b');
		expect(store.rows.every((row) => row.widths.length === 2 && !row.dirty)).toBe(true);
		expect(store.metadata?.revision).toBe(2_001n);
		expect(store.needsFullRedraw).toBe(false);
	});

	it('captures an immutable display-only view that survives later row replacement', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		expect(store.capturePresentationSnapshot()).toBeNull();
		store.apply(bytes(frame('snapshot', 1n, 1n, ['a', 'b'])));
		const snapshot = store.capturePresentationSnapshot();
		if (snapshot === null) throw new Error('Expected a packed presentation snapshot.');
		const capturedByteLength = store.byteLength;

		store.apply(bytes(frame('delta', 2n, 2n, ['x', ''], 1n, [0])));

		expect(snapshot.metadata.revision).toBe(1n);
		expect(snapshot.rows.map((row) => row.grapheme(0))).toEqual(['a', 'b']);
		expect(store.rows.map((row) => row.grapheme(0))).toEqual(['x', 'b']);
		expect(snapshot.byteLength).toBe(capturedByteLength);
		expect(snapshot.needsFullRedraw).toBe(true);
		expect(Object.isFrozen(snapshot)).toBe(true);
		expect(Object.isFrozen(snapshot.rows)).toBe(true);
		snapshot.markRendered();
		expect(snapshot.needsFullRedraw).toBe(true);
	});

	it('re-acknowledges an exact duplicate without mutating the packed grid', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		const snapshot = bytes(frame('snapshot', 1n, 1n, ['a', 'b']));
		store.apply(snapshot);
		store.markRendered();
		const row = store.rows[0];
		const duplicate = store.apply(snapshot);
		expect(duplicate.kind).toBe('duplicate');
		expect(store.rows[0]).toBe(row);
		expect(row?.dirty).toBe(false);
	});

	it('recognizes immutable duplicate identity without revisiting payload bytes', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		store.apply(bytes(frame('snapshot', 1n, 1n, ['a', 'b'])));
		const duplicateBytes = bytes(frame('snapshot', 1n, 1n, ['x', 'b']));
		const lastByte = duplicateBytes.length - 1;
		duplicateBytes[lastByte] = 1;
		expect(decodeTerminalFrame(duplicateBytes).ok).toBe(false);

		expect(store.apply(duplicateBytes)).toEqual({
			acknowledgement: { attachmentId, generation: '1', revision: '1', sequence: '1' },
			kind: 'duplicate',
		});
		expect(store.rows.map((row) => row.grapheme(0))).toEqual(['a', 'b']);
	});

	it('strictly decodes a new sequence after fixed-header inspection', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		store.apply(bytes(frame('snapshot', 1n, 1n, ['a', 'b'])));
		const nextBytes = bytes(frame('delta', 2n, 2n, ['x', ''], 1n, [0]));
		const lastByte = nextBytes.length - 1;
		nextBytes[lastByte] = 1;

		expect(store.apply(nextBytes)).toEqual({
			kind: 'snapshot-required',
			request: { attachmentId, reason: 'decoder-rejected', sequence: '2' },
		});
		expect(store.rows.map((row) => row.grapheme(0))).toEqual(['a', 'b']);
	});

	it('requests a snapshot on a bad base without exposing staged rows', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		store.apply(bytes(frame('snapshot', 1n, 1n, ['a', 'b'])));
		const row = store.rows[0];
		const result = store.apply(bytes(frame('delta', 2n, 3n, ['x', ''], 2n, [0])));
		expect(result).toEqual({
			kind: 'snapshot-required',
			request: { attachmentId, reason: 'base-mismatch', sequence: '2' },
		});
		expect(store.phase).toBe(ProjectedStorePhase.snapshotRequired);
		expect(store.rows[0]).toBe(row);
		expect(row?.grapheme(0)).toBe('a');
	});

	it('accepts a newer complete snapshot across abandoned frame sequences', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		store.apply(bytes(frame('snapshot', 1n, 1n, ['a', 'b'])));
		const rejected = store.apply(bytes(frame('delta', 2n, 3n, ['x', ''], 2n, [0])));
		expect(rejected.kind).toBe('snapshot-required');

		const recovered = store.apply(bytes(frame('snapshot', 3n, 3n, ['x', 'y'])));
		expect(recovered).toEqual({
			acknowledgement: { attachmentId, generation: '1', revision: '3', sequence: '3' },
			kind: 'committed',
		});
		expect(store.phase).toBe(ProjectedStorePhase.synchronized);
		expect(store.rows.map((row) => row.grapheme(0))).toEqual(['x', 'y']);
	});

	it('accepts the first complete snapshot after its initial sequence was abandoned', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);

		expect(store.apply(bytes(frame('snapshot', 2n, 2n, ['x', 'y'])))).toEqual({
			acknowledgement: { attachmentId, generation: '1', revision: '2', sequence: '2' },
			kind: 'committed',
		});
		expect(store.rows.map((row) => row.grapheme(0))).toEqual(['x', 'y']);
	});

	it('still rejects a delta that crosses an abandoned frame sequence', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		store.apply(bytes(frame('snapshot', 1n, 1n, ['a', 'b'])));

		expect(store.apply(bytes(frame('delta', 3n, 2n, ['x', ''], 1n, [0])))).toEqual({
			kind: 'snapshot-required',
			request: { attachmentId, reason: 'sequence-gap', sequence: '3' },
		});
		expect(store.rows.map((row) => row.grapheme(0))).toEqual(['a', 'b']);
	});

	it('retains no grid after disposal', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		store.apply(bytes(frame('snapshot', 1n, 1n, ['a', 'b'])));
		store.dispose();
		expect(store.phase).toBe(ProjectedStorePhase.disposed);
		expect(store.rows).toEqual([]);
		expect(store.apply(bytes(frame('snapshot', 2n, 2n, ['c', 'd'])))).toEqual({
			kind: 'disposed',
		});
	});
});
