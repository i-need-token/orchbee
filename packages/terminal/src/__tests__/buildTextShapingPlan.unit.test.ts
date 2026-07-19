import { describe, expect, it } from 'vitest';
import {
	buildTextShapingPlan,
	type TextShapingCell,
	type TextShapingSpan,
} from '../buildTextShapingPlan';

describe('buildTextShapingPlan', () => {
	it('selects the longest programming ligature at each column', () => {
		const plan = buildTextShapingPlan(cells('<==> !=='));

		expect(plan[0]).toEqual({ start: 0, end: 4, columns: 4, text: '<==>' });
		expect(plan[3]).toBe(plan[0]);
		expect(plan[5]).toEqual({ start: 5, end: 8, columns: 3, text: '!==' });
	});

	it("covers Ghostty's Inconsolata and Monaspace ligature cases", () => {
		const plan = buildTextShapingPlan(cells('>= ==='));

		expect(plan[0]).toEqual({ start: 0, end: 2, columns: 2, text: '>=' });
		expect(plan[3]).toEqual({ start: 3, end: 6, columns: 3, text: '===' });
	});

	it('breaks a run at style, selection, cursor, or preedit boundaries', () => {
		const input = cells('=> -> =>');
		const selected = input[1];
		const cursor = input[4];
		const preedit = input[6];
		if (!selected || !cursor || !preedit) throw new Error('Missing shaping test cells');
		input[1] = { ...selected, styleKey: 'selected' };
		input[4] = { ...cursor, eligible: false };
		input[6] = { ...preedit, eligible: false };
		const plan = buildTextShapingPlan(input);

		expect(plan.every((span) => span === null)).toBe(true);
	});

	it('does not join standard text ligatures that Ghostty deliberately splits', () => {
		const plan = buildTextShapingPlan(cells('fi fl st www'));

		expect(plan.slice(0, 8).every((span) => span === null)).toBe(true);
		expect((plan[9] as TextShapingSpan | null)?.text).toBe('www');
	});

	it('keeps adjacent ligatures in independent bounded spans', () => {
		const plan = buildTextShapingPlan(cells('!===>'));

		expect(plan[0]).toEqual({ start: 0, end: 3, columns: 3, text: '!==' });
		expect(plan[2]).toBe(plan[0]);
		expect(plan[3]).toEqual({ start: 3, end: 5, columns: 2, text: '=>' });
		expect(plan[4]).toBe(plan[3]);
	});

	it('does not segment a long repeated-character row into ligatures', () => {
		const repeated = buildTextShapingPlan(cells('************'));
		const standalone = buildTextShapingPlan(cells(' *** '));

		expect(repeated.every((span) => span === null)).toBe(true);
		expect((standalone[1] as TextShapingSpan | null)?.text).toBe('***');
	});

	it('does not shape incomplete candidates at the end of a row', () => {
		expect(buildTextShapingPlan(cells('code ='))).toEqual([null, null, null, null, null, null]);
	});

	it('breaks shaping at wide and continuation cells', () => {
		const input = cells('=>=>');
		const wide = input[1];
		const continuation = input[2];
		if (!wide || !continuation) throw new Error('Missing shaping test cells');
		input[1] = { ...wide, width: 2 };
		input[2] = { ...continuation, width: 0 };

		expect(buildTextShapingPlan(input).every((span) => span === null)).toBe(true);
	});

	it('uses one shared span object for every cell in a shaped run', () => {
		const plan = buildTextShapingPlan(cells('<==>'));
		const span = plan[0];
		expect(span).not.toBeNull();
		expect(plan.every((candidate) => candidate === span)).toBe(true);
	});
});

function cells(text: string): TextShapingCell[] {
	return [...text].map((character) => ({
		text: character,
		width: 1,
		styleKey: 'regular',
		eligible: character !== ' ',
	}));
}
