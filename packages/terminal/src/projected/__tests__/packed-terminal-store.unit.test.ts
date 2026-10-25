import {
	PROTOCOL_LIMITS,
	AttachmentIdSchema,
	HostEpochSchema,
	decodeTerminalProjectionBuffer,
	ActiveScreen,
	CellWidth,
	CursorShape,
	decodeTerminalFrame,
	encodeTerminalGraphics,
	encodeTerminalFrame,
	encodeTerminalProjectionBuffer,
	ProtocolErrorCode,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalCloseActivityCode,
	TerminalCellWidthCode,
	TerminalFrameExtensionKind,
	TerminalFrameKind,
	TerminalGraphicsUpdateKind,
	UnderlineKind,
	type TerminalFrame,
	type TerminalGraphicsPlacement,
} from '@orchbee/terminal-protocol';
import { readFileSync } from 'node:fs';
import * as fc from 'fast-check';
import { describe, expect, it, vi } from 'vitest';

import { PackedTerminalStore, ProjectedStorePhase } from '../packed-terminal-store';

const hostEpoch = HostEpochSchema.parse('11111111111111111111111111111111');
const attachmentId = AttachmentIdSchema.parse('22222222222222222222222222222222');

interface PresentationStoreTraceCase {
	readonly expectedKind: 'connection-terminal' | 'snapshot-required';
	readonly expectedReason: string;
	readonly expectedSequence?: string;
	readonly initialRevision: string;
	readonly initialSequence: string;
	readonly inputRevision: string;
	readonly inputSequence: string;
	readonly name: string;
	readonly operation:
		| 'increment-cell-pixel-height'
		| 'increment-cell-pixel-width'
		| 'malformed-snapshot-body'
		| 'truncate-fixed-header';
}

interface PresentationStoreTraceCorpus {
	readonly cases: readonly PresentationStoreTraceCase[];
	readonly schemaVersion: number;
	readonly terminalFrameFormat: string;
}

const presentationStoreTraceCorpus: PresentationStoreTraceCorpus = JSON.parse(
	readFileSync(new URL('./fixtures/presentation-store-transition.json', import.meta.url), 'utf8'),
);

const presentationStoreActions = [
	'bad-base-delta',
	'conflicting-duplicate',
	'exact-duplicate',
	'gapped-delta',
	'pixel-mismatch-delta',
	'recovery-snapshot',
	'valid-delta',
] as const;

function presentationDecisionKey(result: ReturnType<PackedTerminalStore['apply']>): string {
	switch (result.kind) {
		case 'snapshot-required':
			return `${result.kind}:${result.request.reason}:${result.request.sequence}`;
		case 'connection-terminal':
			return `${result.kind}:${result.reason.kind}`;
		default:
			return result.kind;
	}
}

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
};

function testCell(
	grapheme: string,
	width: TerminalFrame['rowData'][number]['cells'][number]['width'] = CellWidth.narrow,
): TerminalFrame['rowData'][number]['cells'][number] {
	return {
		grapheme,
		protected: false,
		semanticContent: SemanticContent.output,
		style: 0,
		width,
	};
}

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

function singleColumnFrame(
	kind: 'snapshot' | 'delta',
	sequence: bigint,
	revision: bigint,
	gridRows: number,
	rows: readonly { readonly index: number; readonly text: string }[],
	baseRevision: bigint | null = null,
	hyperlink: string | null = null,
): TerminalFrame {
	const template = frame(kind, sequence, revision, ['', ''], baseRevision, []);
	const style = template.styles[0];
	if (style === undefined) throw new Error('Single-column frame style is missing');
	return {
		...template,
		columns: 1,
		rowData: rows.map((row) => ({
			cells: [
				{
					grapheme: row.text,
					protected: false,
					semanticContent: SemanticContent.output,
					style: 0,
					width: CellWidth.narrow,
				},
			],
			index: row.index,
			promptClass: RowPromptClass.none,
			wrap: false,
			wrapContinuation: false,
		})),
		rows: gridRows,
		styles: [{ ...style, hyperlink }],
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
	animationTick: bigint = resourceRevision,
) {
	const encoded = encodeTerminalGraphics({
		animationTick,
		images: includeImage
			? [
					{
						data: new Uint8Array([1, 2, 3]),
						format: 'rgb',
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
	it('matches the shared OBTF presentation transition trace corpus', () => {
		expect(presentationStoreTraceCorpus).toMatchObject({
			schemaVersion: 1,
			terminalFrameFormat: '0.1.0',
		});
		expect(presentationStoreTraceCorpus.cases).toHaveLength(5);

		for (const trace of presentationStoreTraceCorpus.cases) {
			const store = new PackedTerminalStore(hostEpoch, attachmentId);
			const initialSequence = BigInt(trace.initialSequence);
			const initialRevision = BigInt(trace.initialRevision);
			const inputSequence = BigInt(trace.inputSequence);
			const inputRevision = BigInt(trace.inputRevision);
			expect(
				store.apply(bytes(frame('snapshot', initialSequence, initialRevision, ['a', 'b']))).kind,
			).toBe('committed');
			let input: Uint8Array;
			switch (trace.operation) {
				case 'malformed-snapshot-body': {
					input = bytes(frame('snapshot', inputSequence, inputRevision, ['x', 'y']));
					input[input.length - 1] = 1;
					break;
				}
				case 'truncate-fixed-header': {
					input = bytes(frame('snapshot', inputSequence, inputRevision, ['x', 'y'])).subarray(
						0,
						80,
					);
					break;
				}
				case 'increment-cell-pixel-width': {
					input = bytes({
						...frame('delta', inputSequence, inputRevision, ['x', ''], initialRevision, [0]),
						cellPixelWidth: 10,
					});
					break;
				}
				case 'increment-cell-pixel-height': {
					input = bytes({
						...frame('delta', inputSequence, inputRevision, ['x', ''], initialRevision, [0]),
						cellPixelHeight: 19,
					});
					break;
				}
			}
			const result = store.apply(input);
			let observation;
			if (result.kind === 'snapshot-required') {
				observation = {
					kind: result.kind,
					reason: result.request.reason,
					sequence: result.request.sequence,
				};
			} else if (result.kind === 'connection-terminal' && result.reason.kind === 'protocol') {
				observation = {
					kind: result.kind,
					reason: result.reason.failure.code,
				};
			} else {
				observation = { kind: result.kind, reason: 'unexpected' };
			}
			const expected =
				trace.expectedKind === 'snapshot-required'
					? {
							kind: trace.expectedKind,
							reason: trace.expectedReason,
							sequence: trace.expectedSequence,
						}
					: { kind: trace.expectedKind, reason: trace.expectedReason };
			expect(observation).toEqual(expected);
		}
	});

	it('refines the stateful store model across shrinkable semantic action traces', () => {
		expect(() =>
			fc.assert(
				fc.property(
					fc.array(fc.constantFrom(...presentationStoreActions), { maxLength: 64 }),
					(actions) => {
						const store = new PackedTerminalStore(hostEpoch, attachmentId);
						let lastSequence = 1n;
						let lastRevision = 1n;
						let synchronized = true;
						let lastFrame = bytes(frame('snapshot', 1n, 1n, ['a', 'b']));
						if (presentationDecisionKey(store.apply(lastFrame)) !== 'committed') return false;

						for (const action of actions) {
							let input: Uint8Array;
							let expected: string;
							let nextCommit: { readonly revision: bigint; readonly sequence: bigint } | null =
								null;
							switch (action) {
								case 'valid-delta': {
									const sequence = lastSequence + 1n;
									const revision = lastRevision + 1n;
									input = bytes(frame('delta', sequence, revision, ['', ''], lastRevision, []));
									expected = synchronized
										? 'committed'
										: `snapshot-required:store-reset:${sequence}`;
									if (synchronized) nextCommit = { revision, sequence };
									break;
								}
								case 'exact-duplicate': {
									input = lastFrame;
									expected = 'duplicate';
									break;
								}
								case 'gapped-delta': {
									const sequence = lastSequence + 2n;
									input = bytes(
										frame('delta', sequence, lastRevision + 1n, ['', ''], lastRevision, []),
									);
									expected = `snapshot-required:sequence-gap:${sequence}`;
									break;
								}
								case 'bad-base-delta': {
									const sequence = lastSequence + 1n;
									input = bytes(
										frame('delta', sequence, lastRevision + 2n, ['', ''], lastRevision + 1n, []),
									);
									expected = synchronized
										? `snapshot-required:base-mismatch:${sequence}`
										: `snapshot-required:store-reset:${sequence}`;
									break;
								}
								case 'pixel-mismatch-delta': {
									const sequence = lastSequence + 1n;
									input = bytes({
										...frame('delta', sequence, lastRevision + 1n, ['', ''], lastRevision, []),
										cellPixelWidth: 10,
									});
									expected = synchronized
										? `snapshot-required:dimensions-mismatch:${sequence}`
										: `snapshot-required:store-reset:${sequence}`;
									break;
								}
								case 'recovery-snapshot': {
									const sequence = lastSequence + 1n;
									const revision = lastRevision + 1n;
									input = bytes(frame('snapshot', sequence, revision, ['r', 's']));
									expected = 'committed';
									nextCommit = { revision, sequence };
									break;
								}
								case 'conflicting-duplicate': {
									input = bytes(frame('snapshot', lastSequence, lastRevision + 1n, ['c', 'd']));
									expected = `snapshot-required:sequence-gap:${lastSequence}`;
									break;
								}
							}

							if (presentationDecisionKey(store.apply(input)) !== expected) return false;
							if (nextCommit !== null) {
								lastSequence = nextCommit.sequence;
								lastRevision = nextCommit.revision;
								lastFrame = input;
								synchronized = true;
							} else if (expected.startsWith('snapshot-required:')) {
								synchronized = false;
							}
							if (store.metadata?.revision !== lastRevision) return false;
							const expectedPhase = synchronized
								? ProjectedStorePhase.synchronized
								: ProjectedStorePhase.snapshotRequired;
							if (store.phase !== expectedPhase) return false;
						}
						return true;
					},
				),
				{ numRuns: 200, seed: 0x0b7f_003, verbose: true },
			),
		).not.toThrow();
	});

	it('retains an OBPB 0.0.1 snapshot as direct typed views without per-cell repacking', () => {
		const encoded = encodeTerminalProjectionBuffer(frame('snapshot', 1n, 1n, ['蜂', 'b']));
		if (!encoded.ok) throw new Error(encoded.detail);
		const viewed = decodeTerminalProjectionBuffer(encoded.value);
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
		const viewed = decodeTerminalProjectionBuffer(encoded.value);
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
			extension: () => graphicsExtension('placements', 1n, [graphicsPlacement], false, 0n),
			label: 'stale animation tick',
		},
		{
			extension: () =>
				graphicsExtension('placements', 1n, [{ ...graphicsPlacement, imageId: 8 }], false, 2n),
			label: 'missing retained image reference',
		},
	])('rejects a $label before text or graphics mutation', ({ extension }) => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		expect(
			store.apply(
				bytes({
					...frame('snapshot', 1n, 1n, ['a', 'b']),
					extensions: [graphicsExtension('snapshot', 1n, [graphicsPlacement], true)],
				}),
			).kind,
		).toBe('committed');
		const beforeImages = store.graphics.images;

		expect(
			store.apply(
				bytes({
					...frame('delta', 2n, 2n, ['x', ''], 1n, [0]),
					extensions: [extension()],
				}),
			),
		).toEqual({
			kind: 'snapshot-required',
			request: { attachmentId, reason: 'base-mismatch', sequence: '2' },
		});
		expect(store.metadata?.revision).toBe(1n);
		expect(store.rows[0]?.grapheme(0)).toBe('a');
		expect(store.graphics.images).toBe(beforeImages);
		expect(store.graphics.animationTick).toBe(1n);
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

	it('strictly decodes a duplicate before re-acknowledging its immutable identity', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		store.apply(bytes(frame('snapshot', 1n, 1n, ['a', 'b'])));
		const duplicateBytes = bytes(frame('snapshot', 1n, 1n, ['x', 'b']));
		const lastByte = duplicateBytes.length - 1;
		duplicateBytes[lastByte] = 1;
		expect(decodeTerminalFrame(duplicateBytes).ok).toBe(false);

		expect(store.apply(duplicateBytes)).toEqual({
			kind: 'snapshot-required',
			request: { attachmentId, reason: 'decoder-rejected', sequence: '1' },
		});
		expect(store.rows.map((row) => row.grapheme(0))).toEqual(['a', 'b']);
	});

	it('requests the trusted recovery snapshot sequence after an abandoned gap', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		expect(store.apply(bytes(frame('snapshot', 6n, 41n, ['a', 'b']))).kind).toBe('committed');
		const recovery = bytes(frame('snapshot', 9n, 43n, ['x', 'y']));
		recovery[recovery.length - 1] = 1;
		expect(decodeTerminalFrame(recovery).ok).toBe(false);

		expect(store.apply(recovery)).toEqual({
			kind: 'snapshot-required',
			request: { attachmentId, reason: 'decoder-rejected', sequence: '9' },
		});
		expect(store.metadata?.revision).toBe(41n);
	});

	it('fails closed when a fixed header cannot be trusted or scoped', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		store.apply(bytes(frame('snapshot', 6n, 41n, ['a', 'b'])));
		const truncated = bytes(frame('snapshot', 9n, 43n, ['x', 'y'])).subarray(0, 80);

		const result = store.apply(truncated);
		expect(result).toEqual({
			kind: 'connection-terminal',
			reason: {
				failure: expect.objectContaining({ code: ProtocolErrorCode.truncated, ok: false }),
				kind: 'protocol',
			},
		});
		expect(store.phase).toBe(ProjectedStorePhase.connectionTerminal);
		expect(store.metadata?.revision).toBe(41n);
		expect(store.apply(bytes(frame('snapshot', 10n, 44n, ['n', 'm'])))).toEqual(result);
	});

	it('keeps connection-terminal absorbing across every snapshot replacement entry point', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		store.apply(bytes(frame('snapshot', 6n, 41n, ['a', 'b'])));
		const truncated = bytes(frame('snapshot', 9n, 43n, ['x', 'y'])).subarray(0, 80);
		expect(store.apply(truncated).kind).toBe('connection-terminal');

		const replacement = frame('snapshot', 10n, 44n, ['n', 'm']);
		const encodedProjection = encodeTerminalProjectionBuffer(replacement);
		if (!encodedProjection.ok) throw new Error(encodedProjection.detail);
		const viewedProjection = decodeTerminalProjectionBuffer(encodedProjection.value);
		if (!viewedProjection.ok) throw new Error(viewedProjection.detail);

		expect(store.replaceSnapshot(bytes(replacement), 1024 * 1024)).toEqual({ kind: 'invalid' });
		expect(store.replaceDecodedSnapshot(decodedFrame(replacement), 1024 * 1024)).toEqual({
			kind: 'invalid',
		});
		expect(store.replaceProjectionBufferSnapshot(viewedProjection.value, 1024 * 1024)).toEqual({
			kind: 'invalid',
		});
		expect(store.phase).toBe(ProjectedStorePhase.connectionTerminal);
		expect(store.metadata?.revision).toBe(41n);
		expect(store.rows.map((row) => row.grapheme(0))).toEqual(['a', 'b']);
	});

	it('never infers a successor after the maximum sequence', () => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		const maximum = (1n << 64n) - 1n;
		const snapshot = bytes(frame('snapshot', maximum, maximum, ['a', 'b']));
		expect(store.apply(snapshot).kind).toBe('committed');
		expect(store.apply(snapshot).kind).toBe('duplicate');

		const malformed = bytes(frame('snapshot', maximum, maximum, ['x', 'y']));
		malformed[malformed.length - 1] = 1;
		expect(store.apply(malformed)).toEqual({
			kind: 'snapshot-required',
			request: { attachmentId, reason: 'decoder-rejected', sequence: maximum.toString() },
		});
		expect(store.phase).toBe(ProjectedStorePhase.snapshotRequired);
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

	it.each([
		{
			label: 'columns',
			mutate: (delta: TerminalFrame): TerminalFrame => ({
				...delta,
				columns: 3,
				rowData: delta.rowData.map((row) => ({
					...row,
					cells: [...row.cells, testCell('')],
				})),
			}),
		},
		{
			label: 'grid rows',
			mutate: (delta: TerminalFrame): TerminalFrame => ({ ...delta, rows: 3 }),
		},
		{
			label: 'cell pixel width',
			mutate: (delta: TerminalFrame): TerminalFrame => ({ ...delta, cellPixelWidth: 10 }),
		},
		{
			label: 'cell pixel height',
			mutate: (delta: TerminalFrame): TerminalFrame => ({ ...delta, cellPixelHeight: 19 }),
		},
	])('requires delta continuity for $label', ({ mutate }) => {
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		store.apply(bytes(frame('snapshot', 1n, 1n, ['a', 'b'])));
		const beforeRows = [...store.rows];
		const delta = mutate(frame('delta', 2n, 2n, ['x', ''], 1n, [0]));

		expect(store.apply(bytes(delta))).toEqual({
			kind: 'snapshot-required',
			request: { attachmentId, reason: 'dimensions-mismatch', sequence: '2' },
		});
		expect(store.metadata?.revision).toBe(1n);
		expect(store.rows).toEqual(beforeRows);
	});

	it('validates sparse replacements against both retained neighbors before mutation', () => {
		const base = frame('snapshot', 1n, 1n, ['a', 'b']);
		const row0 = base.rowData[0];
		const row1 = base.rowData[1];
		if (row0 === undefined || row1 === undefined) throw new Error('Topology rows are missing');
		const snapshot: TerminalFrame = {
			...base,
			rowData: [
				{
					...row0,
					cells: [testCell('a'), testCell('', CellWidth.spacerHead)],
					wrap: true,
				},
				{
					...row1,
					cells: [testCell('b', CellWidth.wide), testCell('', CellWidth.spacerTail)],
					wrapContinuation: true,
				},
			],
		};
		const plainRow0 = {
			...row0,
			cells: [testCell('a'), testCell('')],
			wrap: false,
		};
		const plainRow1 = {
			...row1,
			cells: [testCell('b'), testCell('')],
			wrapContinuation: false,
		};
		const nonWideContinuation = { ...plainRow1, wrapContinuation: true };

		for (const replacement of [plainRow0, plainRow1, nonWideContinuation]) {
			const store = new PackedTerminalStore(hostEpoch, attachmentId);
			expect(store.apply(bytes(snapshot)).kind).toBe('committed');
			const beforeRows = [...store.rows];
			const delta: TerminalFrame = {
				...frame('delta', 2n, 2n, ['', ''], 1n, []),
				rowData: [replacement],
			};

			expect(store.apply(bytes(delta))).toEqual({
				kind: 'snapshot-required',
				request: { attachmentId, reason: 'base-mismatch', sequence: '2' },
			});
			expect(store.metadata?.revision).toBe(1n);
			expect(store.rows).toEqual(beforeRows);
			expect(store.rows[0]?.widths[1]).toBe(TerminalCellWidthCode.spacerHead);
			expect(store.rows[1]?.widths[0]).toBe(TerminalCellWidthCode.wide);
		}
	});

	it('isolates terminal outcomes and retained state between attachment stores', () => {
		const otherAttachmentId = AttachmentIdSchema.parse('33333333333333333333333333333333');
		const first = new PackedTerminalStore(hostEpoch, attachmentId);
		const second = new PackedTerminalStore(hostEpoch, otherAttachmentId);
		const firstSnapshot = frame('snapshot', 1n, 1n, ['a', 'b']);
		const secondSnapshot = {
			...frame('snapshot', 1n, 1n, ['x', 'y']),
			attachmentId: otherAttachmentId,
		};
		expect(first.apply(bytes(firstSnapshot)).kind).toBe('committed');
		expect(second.apply(bytes(secondSnapshot)).kind).toBe('committed');

		expect(first.apply(bytes(secondSnapshot))).toMatchObject({ kind: 'connection-terminal' });
		expect(first.phase).toBe(ProjectedStorePhase.connectionTerminal);
		expect(second.phase).toBe(ProjectedStorePhase.synchronized);
		expect(second.rows.map((row) => row.grapheme(0))).toEqual(['x', 'y']);
	});

	it('stops a long sparse stream at the retained aggregate byte budget atomically', () => {
		const gridRows = 3_000;
		const batchRows = 32;
		const textBytes = 13_000;
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		const snapshotRows = Array.from({ length: gridRows }, (_, index) => ({ index, text: '' }));
		expect(
			store.apply(bytes(singleColumnFrame('snapshot', 1n, 1n, gridRows, snapshotRows))).kind,
		).toBe('committed');

		let sequence = 2n;
		let firstRow = 0;
		let committedBatches = 0;
		let failure:
			| {
					readonly beforeAggregateBytes: number;
					readonly beforeBytes: number;
					readonly beforeRevision: bigint | undefined;
					readonly reason: Extract<
						ReturnType<PackedTerminalStore['apply']>,
						{ readonly kind: 'connection-terminal' }
					>['reason'];
			  }
			| undefined;
		while (firstRow < gridRows) {
			const end = Math.min(gridRows, firstRow + batchRows);
			const rows = Array.from({ length: end - firstRow }, (_, offset) => {
				const index = firstRow + offset;
				const prefix = `row-${index.toString().padStart(4, '0')}-`;
				return { index, text: prefix + 'x'.repeat(textBytes - prefix.length) };
			});
			const beforeRevision = store.metadata?.revision;
			const beforeBytes = store.retainedSnapshotBytes;
			const beforeAggregateBytes = store.retainedAggregateBytes;
			const result = store.apply(
				bytes(singleColumnFrame('delta', sequence, sequence, gridRows, rows, sequence - 1n)),
			);
			if (result.kind === 'connection-terminal') {
				failure = { beforeAggregateBytes, beforeBytes, beforeRevision, reason: result.reason };
				break;
			}
			if (result.kind !== 'committed') {
				throw new Error(`Unexpected sparse budget transition: ${result.kind}`);
			}
			if (store.retainedSnapshotBytes > PROTOCOL_LIMITS.maxFrameBytes) {
				throw new Error('Committed sparse presentation exceeds its canonical byte budget');
			}
			if (store.retainedAggregateBytes > PROTOCOL_LIMITS.maxFrameBytes) {
				throw new Error('Committed sparse presentation exceeds its aggregate byte budget');
			}
			committedBatches += 1;
			firstRow = end;
			sequence += 1n;
		}
		if (failure === undefined || failure.reason.kind !== 'presentation-limit-exceeded') {
			throw new Error('Sparse stream did not reach its canonical snapshot byte budget');
		}
		expect(failure.reason).toMatchObject({
			kind: 'presentation-limit-exceeded',
			limit: 'aggregate-bytes',
			maximum: PROTOCOL_LIMITS.maxFrameBytes,
		});
		expect(failure.reason.actual).toBeGreaterThan(failure.reason.maximum);
		expect(store.metadata?.revision).toBe(failure.beforeRevision);
		expect(store.retainedSnapshotBytes).toBe(failure.beforeBytes);
		expect(store.retainedAggregateBytes).toBe(failure.beforeAggregateBytes);
		expect(committedBatches).toBeGreaterThanOrEqual(80);
	}, 30_000);

	it('charges repeated sparse values per retained occurrence before mutation', () => {
		const gridRows = 1_400;
		const repeatedText = 'x'.repeat(16 * 1024);
		const repeatedHyperlink = 'h'.repeat(16 * 1024);
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		const snapshotRows = Array.from({ length: gridRows }, (_, index) => ({ index, text: '' }));
		expect(
			store.apply(bytes(singleColumnFrame('snapshot', 1n, 1n, gridRows, snapshotRows))).kind,
		).toBe('committed');

		for (let rowIndex = 0; rowIndex < gridRows; rowIndex += 1) {
			const sequence = BigInt(rowIndex) + 2n;
			const beforeRevision = store.metadata?.revision;
			const beforeSnapshotBytes = store.retainedSnapshotBytes;
			const beforeAggregateBytes = store.retainedAggregateBytes;
			const result = store.apply(
				bytes(
					singleColumnFrame(
						'delta',
						sequence,
						sequence,
						gridRows,
						[{ index: rowIndex, text: repeatedText }],
						sequence - 1n,
						repeatedHyperlink,
					),
				),
			);
			if (result.kind === 'committed') {
				if (store.retainedSnapshotBytes >= 128 * 1024) {
					throw new Error('Repeated values unexpectedly expanded the canonical snapshot');
				}
				if (store.retainedAggregateBytes > PROTOCOL_LIMITS.maxFrameBytes) {
					throw new Error('Committed repeated values exceeded the aggregate budget');
				}
				continue;
			}
			expect(result).toMatchObject({
				kind: 'connection-terminal',
				reason: {
					kind: 'presentation-limit-exceeded',
					limit: 'aggregate-bytes',
					maximum: PROTOCOL_LIMITS.maxFrameBytes,
				},
			});
			if (result.kind !== 'connection-terminal') {
				throw new Error(`Unexpected repeated sparse budget transition: ${result.kind}`);
			}
			if (result.reason.kind !== 'presentation-limit-exceeded') {
				throw new Error('Repeated sparse budget ended for a non-budget reason');
			}
			expect(result.reason.actual).toBeGreaterThan(result.reason.maximum);
			expect(store.metadata?.revision).toBe(beforeRevision);
			expect(store.retainedSnapshotBytes).toBe(beforeSnapshotBytes);
			expect(store.retainedAggregateBytes).toBe(beforeAggregateBytes);
			expect(rowIndex).toBeGreaterThan(1_000);
			return;
		}
		throw new Error('Repeated sparse values bypassed the aggregate byte budget');
	}, 30_000);

	it('counts retained hyperlink styles before replacing a sparse row', () => {
		const gridRows = 1_025;
		const store = new PackedTerminalStore(hostEpoch, attachmentId);
		const snapshotRows = Array.from({ length: gridRows }, (_, index) => ({ index, text: '' }));
		expect(
			store.apply(bytes(singleColumnFrame('snapshot', 1n, 1n, gridRows, snapshotRows))).kind,
		).toBe('committed');
		let failure:
			| {
					readonly beforeRevision: bigint | undefined;
					readonly result: ReturnType<PackedTerminalStore['apply']>;
			  }
			| undefined;

		for (let rowIndex = 0; rowIndex < gridRows; rowIndex += 1) {
			const sequence = BigInt(rowIndex) + 2n;
			const beforeRevision = store.metadata?.revision;
			const result = store.apply(
				bytes(
					singleColumnFrame(
						'delta',
						sequence,
						sequence,
						gridRows,
						[{ index: rowIndex, text: '' }],
						sequence - 1n,
						`https://example.invalid/${rowIndex}`,
					),
				),
			);
			if (rowIndex < 1_023) {
				if (result.kind !== 'committed') {
					throw new Error(`Unexpected sparse style transition: ${result.kind}`);
				}
				continue;
			}
			failure = { beforeRevision, result };
			break;
		}
		if (failure === undefined) {
			throw new Error('Sparse hyperlink styles did not reach their table budget');
		}
		expect(failure.result).toMatchObject({
			kind: 'connection-terminal',
			reason: {
				actual: 1_025,
				kind: 'presentation-limit-exceeded',
				limit: 'styles',
				maximum: 1_024,
			},
		});
		expect(store.metadata?.revision).toBe(failure.beforeRevision);
	}, 30_000);

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
