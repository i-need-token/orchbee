import type {
	FontMetricModifier,
	FontMetricModifiers,
	FontSizeAdjustment,
	FontWeight,
} from './types';

const FONT_METRIC_SAMPLE_SIZE = 1_000;
const PRINTABLE_ASCII = Array.from({ length: 95 }, (_, index) => String.fromCodePoint(index + 32));
const TERMINAL_FONT_LOAD_SAMPLE = 'MW你🚀\uF120';

export interface TerminalFontMeasurementConfiguration {
	readonly fontFamily: string;
	readonly fontMetricModifiers: FontMetricModifiers;
	readonly fontSize: number;
	readonly fontWeight: FontWeight;
	readonly fontWeightBold: FontWeight;
}

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

/** Load every face used by the first terminal presentation. */
export async function loadTerminalFonts(
	document: Document,
	configuration: TerminalFontMeasurementConfiguration,
): Promise<void> {
	const fonts = document.fonts;
	if (!fonts || typeof fonts.load !== 'function') return;
	const descriptions = new Set([
		`${configuration.fontWeight} ${configuration.fontSize}px ${configuration.fontFamily}`,
		`italic ${configuration.fontWeight} ${configuration.fontSize}px ${configuration.fontFamily}`,
		`${configuration.fontWeightBold} ${configuration.fontSize}px ${configuration.fontFamily}`,
		`italic ${configuration.fontWeightBold} ${configuration.fontSize}px ${configuration.fontFamily}`,
	]);
	await Promise.all(
		[...descriptions].map((description) => fonts.load(description, TERMINAL_FONT_LOAD_SAMPLE)),
	);
}

/** Measure the exact cell metrics used by the renderer's first presentation. */
export function measureTerminalFont(
	document: Document,
	configuration: TerminalFontMeasurementConfiguration,
	devicePixelRatio: number,
): DetailedFontMetrics {
	const canvas = document.createElement('canvas');
	const context = canvas.getContext('2d');
	if (!context) throw new Error('Failed to create font measurement context');
	Reflect.set(context, 'textRendering', 'geometricPrecision');
	context.direction = 'ltr';
	context.font = measurementCanvasFont(
		'normal',
		configuration.fontWeight,
		configuration.fontSize,
		configuration.fontFamily,
	);

	const nominalMetrics = context.measureText('M');
	context.font = measurementCanvasFont(
		'normal',
		configuration.fontWeight,
		FONT_METRIC_SAMPLE_SIZE,
		configuration.fontFamily,
	);
	const sampledMetrics = context.measureText('M');
	const sampledScale = configuration.fontSize / FONT_METRIC_SAMPLE_SIZE;
	const sampledWidth = sampledMetrics.width * sampledScale;
	const nominalAscent =
		nominalMetrics.fontBoundingBoxAscent ||
		nominalMetrics.actualBoundingBoxAscent ||
		configuration.fontSize * 0.8;
	const nominalDescent =
		nominalMetrics.fontBoundingBoxDescent ||
		nominalMetrics.actualBoundingBoxDescent ||
		configuration.fontSize * 0.2;
	const sampledAscent = sampledMetrics.fontBoundingBoxAscent * sampledScale;
	const sampledDescent = sampledMetrics.fontBoundingBoxDescent * sampledScale;
	const sampledFaceHeight = sampledAscent + sampledDescent;
	const nominalFaceHeight = nominalAscent + nominalDescent;
	const sampledMetricsCoherent =
		positiveFiniteMetric(sampledWidth) &&
		positiveFiniteMetric(sampledAscent) &&
		positiveFiniteMetric(sampledDescent) &&
		Math.abs(sampledWidth - nominalMetrics.width) <= 1 &&
		Math.abs(sampledFaceHeight - nominalFaceHeight) <= 1;
	const faceWidth = sampledMetricsCoherent
		? Math.max(...PRINTABLE_ASCII.map((character) => context.measureText(character).width)) *
			sampledScale
		: nominalMetrics.width;
	const width = Math.max(1, Math.round(faceWidth));
	const ascent = sampledMetricsCoherent ? sampledAscent : nominalAscent;
	const descent = sampledMetricsCoherent ? sampledDescent : nominalDescent;
	const faceHeight = Math.max(1, ascent + descent);
	const height = Math.max(1, Math.round(faceHeight));
	const baselineFromBottom =
		Math.round((descent - (height - faceHeight) / 2) * devicePixelRatio) / devicePixelRatio;
	const baseline = Math.min(height, Math.max(0, height - baselineFromBottom));
	const underlineThickness = 1;
	const sampledCapHeight = context.measureText('H').actualBoundingBoxAscent * sampledScale;
	const measuredCapHeight =
		sampledMetricsCoherent && positiveFiniteMetric(sampledCapHeight)
			? sampledCapHeight
			: 0.75 * ascent;
	return applyFontMetricModifiers(
		{
			cellWidth: width,
			cellHeight: height,
			cellBaseline: baselineFromBottom,
			underlinePosition: Math.max(0, Math.round(baseline + underlineThickness)),
			underlineThickness,
			strikethroughPosition: Math.max(0, Math.round(height / 2)),
			strikethroughThickness: 1,
			overlinePosition: 0,
			overlineThickness: 1,
			boxThickness: 1,
			cursorThickness: 1 / devicePixelRatio,
			cursorHeight: height,
			iconHeight: faceHeight,
			iconHeightSingle: (2 * measuredCapHeight + faceHeight) / 3,
			faceWidth: Math.max(1, faceWidth),
			faceHeight,
			faceY: baselineFromBottom - descent,
		},
		configuration.fontMetricModifiers,
	);
}

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

/** Apply one parsed modifier with Ghostty's integer rounding and saturation rules. */
export function applyFontMetricModifier(
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

function measurementCanvasFont(
	style: 'normal' | 'italic',
	weight: FontWeight,
	size: number,
	family: string,
): string {
	const stylePrefix = style === 'italic' ? 'italic ' : '';
	return `${stylePrefix}${weight} ${size}px ${family}`;
}

function positiveFiniteMetric(value: number): boolean {
	return Number.isFinite(value) && value > 0;
}

function isPositive(value: number | undefined): value is number {
	return value !== undefined && Number.isFinite(value) && value > 0;
}
