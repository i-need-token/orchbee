import { describe, expect, it } from 'vitest';
import {
	buildTextShapingPlan,
	type TextShapingCell,
	type TextShapingSpan,
} from '../buildTextShapingPlan';

describe('buildTextShapingPlan', () => {
	it('keeps programming ligatures inside one complete style run', () => {
		const plan = buildTextShapingPlan(cells('<==> !=='));

		expect(plan[0]).toEqual({ start: 0, end: 8, columns: 8, text: '<==> !==' });
		expect(plan[7]).toBe(plan[0]);
	});

	it("covers Ghostty's Inconsolata and Monaspace ligature cases", () => {
		const plan = buildTextShapingPlan(cells('>= ==='));

		expect(plan[0]).toEqual({ start: 0, end: 6, columns: 6, text: '>= ===' });
		expect(plan[5]).toBe(plan[0]);
	});

	it('builds one browser-shaping run across complex graphemes and empty cells', () => {
		const input: TextShapingCell[] = [
			{ text: 'ক', width: 1, styleKey: 'regular', eligible: true },
			{ text: '', width: 1, styleKey: 'regular', eligible: true },
			{ text: '্ষ', width: 1, styleKey: 'regular', eligible: true },
		];

		const plan = buildTextShapingPlan(input, { allowOptionalLigatures: false });

		expect(plan[0]).toEqual({ start: 0, end: 3, columns: 3, text: 'ক ্ষ' });
		expect(plan[2]).toBe(plan[0]);
	});

	it('leaves optional ASCII ligatures unshaped when the presentation disables them', () => {
		const plan = buildTextShapingPlan(cells('!=='), { allowOptionalLigatures: false });

		expect(plan).toEqual([null, null, null]);
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

		expect(plan[0]).toBeNull();
		expect(plan[1]).toBeNull();
		expect((plan[2] as TextShapingSpan | null)?.text).toBe(' -');
		expect(plan[4]).toBeNull();
		expect(plan[6]).toBeNull();
	});

	it('breaks standard text ligatures that Ghostty deliberately splits', () => {
		const plan = buildTextShapingPlan(cells('fi fl st www'));

		expect(plan[0]).toBeNull();
		expect((plan[1] as TextShapingSpan | null)?.text).toBe('i f');
		expect((plan[4] as TextShapingSpan | null)?.text).toBe('l s');
		expect((plan[7] as TextShapingSpan | null)?.text).toBe('t www');
	});

	it('keeps adjacent programming ligatures in one style run', () => {
		const plan = buildTextShapingPlan(cells('!===>'));

		expect(plan[0]).toEqual({ start: 0, end: 5, columns: 5, text: '!===>' });
		expect(plan.every((candidate) => candidate === plan[0])).toBe(true);
	});

	it('shapes a complete repeated-character run without guessing font ligatures', () => {
		const repeated = buildTextShapingPlan(cells('************'));
		const standalone = buildTextShapingPlan(cells(' *** '));

		expect((repeated[0] as TextShapingSpan | null)?.text).toBe('************');
		expect((standalone[0] as TextShapingSpan | null)?.text).toBe(' *** ');
	});

	it('does not require a hardcoded ligature candidate at the end of a row', () => {
		const plan = buildTextShapingPlan(cells('code ='));
		expect(plan[0]).toEqual({ start: 0, end: 6, columns: 6, text: 'code =' });
		expect(plan[5]).toBe(plan[0]);
	});

	it('retains a wide head and continuation inside its shaping run', () => {
		const input = cells('=>=>');
		const wide = input[1];
		const continuation = input[2];
		if (!wide || !continuation) throw new Error('Missing shaping test cells');
		input[1] = { ...wide, width: 2 };
		input[2] = { ...continuation, width: 0 };

		const plan = buildTextShapingPlan(input);
		expect(plan[0]).toEqual({ start: 0, end: 4, columns: 4, text: '=>>' });
		expect(plan[3]).toBe(plan[0]);
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
		eligible: true,
	}));
}
