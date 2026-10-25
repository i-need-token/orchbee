import { describe, expect, it } from 'vitest';
import {
	applyFontMetricModifiers,
	parseFontMetricModifier,
	type DetailedFontMetrics,
} from '../font-metric-modifiers';

describe('font metric modifiers', () => {
	it('applies metric modifiers', () => {
		const metrics = applyFontMetricModifiers(createMetrics({ cellWidth: 100 }), {
			cellWidth: '20%',
		});
		expect(metrics.cellWidth).toBe(120);
	});

	it('centers the face when making the cell height smaller', () => {
		const metrics = applyFontMetricModifiers(
			createMetrics({
				faceY: 0.33,
				cellBaseline: 50,
				underlinePosition: 55,
				strikethroughPosition: 30,
				overlinePosition: 0,
				cellHeight: 100,
				faceHeight: 99.67,
				cursorHeight: 100,
			}),
			{ cellHeight: '-25%' },
		);

		expect(metrics).toMatchObject({
			faceY: -12.67,
			cellHeight: 75,
			cellBaseline: 37,
			underlinePosition: 43,
			strikethroughPosition: 18,
			overlinePosition: -12,
			cursorHeight: 100,
		});
	});

	it('centers the face when making the cell height larger', () => {
		const metrics = applyFontMetricModifiers(
			createMetrics({
				faceY: 0.33,
				cellBaseline: 50,
				underlinePosition: 55,
				strikethroughPosition: 30,
				overlinePosition: 0,
				cellHeight: 100,
				faceHeight: 99.67,
				cursorHeight: 100,
			}),
			{ cellHeight: '75%' },
		);

		expect(metrics).toMatchObject({
			faceY: 37.33,
			cellHeight: 175,
			cellBaseline: 87,
			underlinePosition: 93,
			strikethroughPosition: 68,
			overlinePosition: 38,
			cursorHeight: 100,
		});
	});

	it('adjusts both icon heights by percentage without changing face metrics', () => {
		const metrics = applyFontMetricModifiers(
			createMetrics({ iconHeight: 100, iconHeightSingle: 80, faceHeight: 100, faceY: 1 }),
			{ iconHeight: '-25%' },
		);
		expect(metrics).toMatchObject({
			iconHeight: 75,
			iconHeightSingle: 60,
			faceHeight: 100,
			faceY: 1,
		});
	});

	it('adjusts both icon heights by absolute pixels without changing face metrics', () => {
		const metrics = applyFontMetricModifiers(
			createMetrics({ iconHeight: 100, iconHeightSingle: 80, faceHeight: 100, faceY: 1 }),
			{ iconHeight: -5 },
		);
		expect(metrics).toMatchObject({
			iconHeight: 95,
			iconHeightSingle: 75,
			faceHeight: 100,
			faceY: 1,
		});
	});

	it('parses and applies percent modifiers', () => {
		expect(parseFontMetricModifier('20%')).toEqual({ kind: 'percent', factor: 1.2 });
		expect(parseFontMetricModifier('-20%')).toEqual({ kind: 'percent', factor: 0.8 });
		expect(parseFontMetricModifier('0%')).toEqual({ kind: 'percent', factor: 1 });
		expect(
			applyFontMetricModifiers(createMetrics({ cellBaseline: 100 }), { fontBaseline: '-20%' })
				.cellBaseline,
		).toBe(80);
		expect(
			applyFontMetricModifiers(createMetrics({ cellBaseline: 100 }), { fontBaseline: '80%' })
				.cellBaseline,
		).toBe(180);
	});

	it('parses and saturates absolute modifiers', () => {
		expect(parseFontMetricModifier('100')).toEqual({ kind: 'absolute', delta: 100 });
		expect(parseFontMetricModifier('-100')).toEqual({ kind: 'absolute', delta: -100 });
		expect(
			applyFontMetricModifiers(createMetrics({ cellBaseline: 100 }), { fontBaseline: -100 })
				.cellBaseline,
		).toBe(0);
		expect(
			applyFontMetricModifiers(createMetrics({ cellBaseline: 100 }), { fontBaseline: -120 })
				.cellBaseline,
		).toBe(0);
		expect(
			applyFontMetricModifiers(createMetrics({ cellBaseline: 100 }), { fontBaseline: 100 })
				.cellBaseline,
		).toBe(200);
	});

	it('rejects non-finite parsed and applied metric results', () => {
		expect(() => parseFontMetricModifier(`${'9'.repeat(400)}%`)).toThrow(TypeError);
		expect(() => parseFontMetricModifier('9'.repeat(400))).toThrow(TypeError);
		expect(
			() =>
				applyFontMetricModifiers(createMetrics({ cellBaseline: Number.MAX_VALUE }), {
					fontBaseline: '100%',
				}).cellBaseline,
		).toThrow(TypeError);
		expect(
			() =>
				applyFontMetricModifiers(createMetrics({ cellBaseline: Number.POSITIVE_INFINITY }), {
					fontBaseline: 0,
				}).cellBaseline,
		).toThrow(TypeError);
	});
});

function createMetrics(overrides: Partial<DetailedFontMetrics> = {}): DetailedFontMetrics {
	return {
		cellWidth: 10,
		cellHeight: 22,
		cellBaseline: 5,
		underlinePosition: 19,
		underlineThickness: 1,
		strikethroughPosition: 12,
		strikethroughThickness: 1,
		overlinePosition: 0,
		overlineThickness: 1,
		boxThickness: 1,
		cursorThickness: 1,
		cursorHeight: 22,
		iconHeight: 21.12,
		iconHeightSingle: 44.48 / 3,
		faceWidth: 9.6,
		faceHeight: 21.12,
		faceY: 0.2,
		...overrides,
	};
}
