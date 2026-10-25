import { type DetailedFontMetrics, applyFontMetricModifiers } from './font-metric-modifiers';
import type { FontMetricModifiers, FontWeight } from './types';

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

export { parseFontMetricModifier } from './font-metric-modifiers';
export type { DetailedFontMetrics } from './font-metric-modifiers';
