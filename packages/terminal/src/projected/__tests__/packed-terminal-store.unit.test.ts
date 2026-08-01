import {
	ActiveScreen,
	CellWidth,
	CursorShape,
	encodeTerminalFrame,
	parseAttachmentId,
	parseHostEpoch,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalFrameKind,
	UnderlineKind,
	type TerminalFrame,
} from '@orchbee/terminal-protocol';
import { describe, expect, it } from 'vitest';

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
		title: 'test',
	};
}

function bytes(value: TerminalFrame): Uint8Array {
	const encoded = encodeTerminalFrame(value);
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

describe('PackedTerminalStore', () => {
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
