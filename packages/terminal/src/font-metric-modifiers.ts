import type { FontMetricModifier, FontMetricModifiers } from './types';

export interface DetailedFontMetrics {
	readonly cellWidth: number;
	readonly cellHeight: number;
	/** Distance from the bottom of the cell to the baseline. */
	readonly cellBaseline: number;
	readonly underlinePosition: number;
	readonly underlineThickness: number;
	readonly strikethroughPosition: number;
	readonly strikethroughThickness: number;
	readonly overlinePosition: number;
	readonly overlineThickness: number;
	readonly boxThickness: number;
	readonly cursorThickness: number;
	readonly cursorHeight: number;
	readonly iconHeight: number;
	readonly iconHeightSingle: number;
	readonly faceWidth: number;
	readonly faceHeight: number;
	readonly faceY: number;
}

type ParsedFontMetricModifier =
	| { readonly kind: 'percent'; readonly factor: number }
	| { readonly kind: 'absolute'; readonly delta: number };

const PERCENT_MODIFIER = /^([+-]?(?:\d+(?:\.\d+)?|\.\d+))%$/u;

const ABSOLUTE_MODIFIER = /^[+-]?\d+$/u;

/** Parse Ghostty's delta syntax: percentages scale and integers add. */
export function parseFontMetricModifier(input: FontMetricModifier): ParsedFontMetricModifier {
	if (typeof input === 'number') {
		if (!Number.isSafeInteger(input)) throw new TypeError('Font metric deltas must be integers');
		return { kind: 'absolute', delta: input };
	}

	const percent = input.match(PERCENT_MODIFIER);
	if (percent) {
		const delta = Number(percent[1]);
		if (!Number.isFinite(delta)) {
			throw new TypeError('Font metric percentages must be finite');
		}
		const factor = Math.max(0, 1 + delta / 100);
		if (!Number.isFinite(factor)) {
			throw new TypeError('Font metric percentage factors must be finite');
		}
		return { kind: 'percent', factor };
	}
	if (ABSOLUTE_MODIFIER.test(input)) {
		const delta = Number.parseInt(input, 10);
		if (!Number.isSafeInteger(delta)) {
			throw new TypeError('Font metric deltas must be safe integers');
		}
		return { kind: 'absolute', delta };
	}
	throw new TypeError(`Invalid font metric modifier: ${input}`);
}

/** Apply one modifier with Ghostty's integer rounding and saturation rules. */
function applyFontMetricModifier(
	value: number,
	modifier: FontMetricModifier,
	options: { readonly integer?: boolean; readonly minimum?: number } = {},
): number {
	if (!Number.isFinite(value)) throw new TypeError('Font metrics must be finite');
	const parsed = parseFontMetricModifier(modifier);
	const adjusted = parsed.kind === 'percent' ? value * parsed.factor : value + parsed.delta;
	if (!Number.isFinite(adjusted)) throw new TypeError('Adjusted font metrics must be finite');
	const rounded = options.integer === true ? Math.round(adjusted) : adjusted;
	const result = Math.max(options.minimum ?? 0, rounded);
	if (!Number.isFinite(result)) throw new TypeError('Accepted font metrics must be finite');
	return result;
}

/** Apply all renderer-owned metric modifiers using Ghostty's field semantics. */
export function applyFontMetricModifiers(
	source: DetailedFontMetrics,
	modifiers: Readonly<FontMetricModifiers>,
): DetailedFontMetrics {
	const metrics = { ...source };

	if (modifiers.cellWidth !== undefined) {
		metrics.cellWidth = applyFontMetricModifier(metrics.cellWidth, modifiers.cellWidth, {
			integer: true,
			minimum: 1,
		});
	}

	if (modifiers.cellHeight !== undefined) {
		const original = metrics.cellHeight;
		const adjusted = applyFontMetricModifier(original, modifiers.cellHeight, {
			integer: true,
			minimum: 1,
		});
		if (adjusted !== original) {
			const difference = adjusted - original;
			const halfDifference = difference / 2;
			const positionFromCenter = metrics.faceY - (original - metrics.faceHeight) / 2;
			const differenceTop =
				positionFromCenter > 0 ? Math.ceil(halfDifference) : Math.floor(halfDifference);
			const differenceBottom =
				positionFromCenter > 0 ? Math.floor(halfDifference) : Math.ceil(halfDifference);

			metrics.cellHeight = adjusted;
			metrics.cellBaseline = Math.max(0, metrics.cellBaseline + differenceBottom);
			metrics.faceY += differenceBottom;
			metrics.underlinePosition = Math.max(0, metrics.underlinePosition + differenceTop);
			metrics.strikethroughPosition = Math.max(0, metrics.strikethroughPosition + differenceTop);
			metrics.overlinePosition += differenceTop;
		}
	}

	applyIntegerMetric(metrics, 'cellBaseline', modifiers.fontBaseline, 0);
	applyIntegerMetric(metrics, 'underlinePosition', modifiers.underlinePosition, 0);
	applyIntegerMetric(metrics, 'underlineThickness', modifiers.underlineThickness, 1);
	applyIntegerMetric(metrics, 'strikethroughPosition', modifiers.strikethroughPosition, 0);
	applyIntegerMetric(metrics, 'strikethroughThickness', modifiers.strikethroughThickness, 1);
	applyIntegerMetric(
		metrics,
		'overlinePosition',
		modifiers.overlinePosition,
		Number.MIN_SAFE_INTEGER,
	);
	applyIntegerMetric(metrics, 'overlineThickness', modifiers.overlineThickness, 1);
	applyIntegerMetric(metrics, 'cursorThickness', modifiers.cursorThickness, 1);
	applyIntegerMetric(metrics, 'cursorHeight', modifiers.cursorHeight, 1);
	applyIntegerMetric(metrics, 'boxThickness', modifiers.boxThickness, 1);

	if (modifiers.iconHeight !== undefined) {
		metrics.iconHeight = applyFontMetricModifier(metrics.iconHeight, modifiers.iconHeight, {
			minimum: 1,
		});
		metrics.iconHeightSingle = applyFontMetricModifier(
			metrics.iconHeightSingle,
			modifiers.iconHeight,
			{ minimum: 1 },
		);
	}

	return metrics;
}

function applyIntegerMetric<K extends keyof DetailedFontMetrics>(
	metrics: { -readonly [P in keyof DetailedFontMetrics]: DetailedFontMetrics[P] },
	key: K,
	modifier: FontMetricModifier | undefined,
	minimum: number,
): void {
	if (modifier === undefined) return;
	metrics[key] = applyFontMetricModifier(metrics[key], modifier, {
		integer: true,
		minimum,
	});
}
