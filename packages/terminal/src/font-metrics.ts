import type { FontMetricModifier, FontMetricModifiers, FontSizeAdjustment } from './types';

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

export interface FontFaceMetrics {
	readonly pixelsPerEm: number;
	readonly cellWidth: number;
	readonly ascent: number;
	readonly descent: number;
	readonly lineGap: number;
	readonly capHeight?: number;
	readonly exHeight?: number;
	readonly asciiHeight?: number;
	readonly icWidth?: number;
}

export type ParsedFontMetricModifier =
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
		return { kind: 'percent', factor: Math.max(0, 1 + delta / 100) };
	}
	if (ABSOLUTE_MODIFIER.test(input)) {
		return { kind: 'absolute', delta: Number.parseInt(input, 10) };
	}
	throw new TypeError(`Invalid font metric modifier: ${input}`);
}

/** Apply one parsed modifier with Ghostty's integer rounding and saturation rules. */
export function applyFontMetricModifier(
	value: number,
	modifier: FontMetricModifier,
	options: { readonly integer?: boolean; readonly minimum?: number } = {},
): number {
	const parsed = parseFontMetricModifier(modifier);
	const adjusted = parsed.kind === 'percent' ? value * parsed.factor : value + parsed.delta;
	const rounded = options.integer === true ? Math.round(adjusted) : adjusted;
	return Math.max(options.minimum ?? 0, rounded);
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

/** Calculate Ghostty's fallback-face scale using normalized font metrics. */
export function fontSizeAdjustmentScale(
	primary: FontFaceMetrics,
	fallback: FontFaceMetrics,
	adjustment: FontSizeAdjustment,
): number {
	if (adjustment === 'none') return 1;
	const metric = resolveAdjustmentMetric(fallback, adjustment);
	const primaryValue = metricValue(primary, metric) / positiveEm(primary.pixelsPerEm);
	const fallbackValue = metricValue(fallback, metric) / positiveEm(fallback.pixelsPerEm);
	if (!Number.isFinite(primaryValue) || !Number.isFinite(fallbackValue) || fallbackValue <= 0) {
		return 1;
	}
	return primaryValue / fallbackValue;
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

function resolveAdjustmentMetric(
	fallback: FontFaceMetrics,
	requested: Exclude<FontSizeAdjustment, 'none'>,
): Exclude<FontSizeAdjustment, 'none'> {
	if (requested === 'ic-width' && isPositive(fallback.icWidth)) return 'ic-width';
	if ((requested === 'ic-width' || requested === 'ex-height') && isPositive(fallback.exHeight)) {
		return 'ex-height';
	}
	if (
		(requested === 'ic-width' || requested === 'ex-height' || requested === 'cap-height') &&
		isPositive(fallback.capHeight)
	) {
		return 'cap-height';
	}
	return 'line-height';
}

function metricValue(
	metrics: FontFaceMetrics,
	metric: Exclude<FontSizeAdjustment, 'none'>,
): number {
	switch (metric) {
		case 'ic-width':
			return isPositive(metrics.icWidth)
				? metrics.icWidth
				: Math.min(asciiHeight(metrics), 2 * metrics.cellWidth);
		case 'ex-height':
			return isPositive(metrics.exHeight) ? metrics.exHeight : 0.75 * capHeight(metrics);
		case 'cap-height':
			return capHeight(metrics);
		case 'line-height':
			return metrics.ascent - metrics.descent + metrics.lineGap;
	}
}

function capHeight(metrics: FontFaceMetrics): number {
	return isPositive(metrics.capHeight) ? metrics.capHeight : 0.75 * metrics.ascent;
}

function asciiHeight(metrics: FontFaceMetrics): number {
	return isPositive(metrics.asciiHeight) ? metrics.asciiHeight : 1.5 * capHeight(metrics);
}

function positiveEm(value: number): number {
	return Number.isFinite(value) && value > 0 ? value : 1;
}

function isPositive(value: number | undefined): value is number {
	return value !== undefined && Number.isFinite(value) && value > 0;
}
