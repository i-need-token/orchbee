import type { DetailedFontMetrics } from './font-metrics';
import {
	NERD_FONT_CONSTRAINT_RULES,
	type GeneratedGlyphConstraint,
} from './nerd-font-constraints.generated';

export interface GlyphSize {
	readonly width: number;
	readonly height: number;
	readonly x: number;
	readonly y: number;
}

export interface GlyphConstraint {
	readonly size: 'none' | 'fit' | 'cover' | 'fit_cover1' | 'stretch';
	readonly alignVertical: 'none' | 'start' | 'end' | 'center' | 'center1';
	readonly alignHorizontal: 'none' | 'start' | 'end' | 'center' | 'center1';
	readonly padTop: number;
	readonly padLeft: number;
	readonly padRight: number;
	readonly padBottom: number;
	readonly relativeWidth: number;
	readonly relativeHeight: number;
	readonly relativeX: number;
	readonly relativeY: number;
	readonly maxXyRatio?: number;
	readonly maxConstraintWidth: number;
	readonly height: 'cell' | 'icon';
}

const NO_GLYPH_CONSTRAINT: GlyphConstraint = {
	size: 'none',
	alignVertical: 'none',
	alignHorizontal: 'none',
	padTop: 0,
	padLeft: 0,
	padRight: 0,
	padBottom: 0,
	relativeWidth: 1,
	relativeHeight: 1,
	relativeX: 0,
	relativeY: 0,
	maxConstraintWidth: 2,
	height: 'cell',
};

const FIT_SYMBOL_CONSTRAINT: GlyphConstraint = {
	...NO_GLYPH_CONSTRAINT,
	size: 'fit',
};
const nerdFontConstraintCache = new Map<number, GlyphConstraint | null>();
const SYMBOL = /\p{Symbol}/u;

/** Return the Ghostty-generated Nerd Font constraint for a codepoint. */
function getNerdFontConstraint(codepoint: number): GlyphConstraint | null {
	const cached = nerdFontConstraintCache.get(codepoint);
	if (cached !== undefined) return cached;
	for (const rule of NERD_FONT_CONSTRAINT_RULES) {
		if (!rule.ranges.some(([start, end]) => codepoint >= start && codepoint <= end)) continue;
		const constraint = resolveConstraint(rule.constraint);
		nerdFontConstraintCache.set(codepoint, constraint);
		return constraint;
	}
	nerdFontConstraintCache.set(codepoint, null);
	return null;
}

/** Apply Ghostty's generated Nerd Font rule, or fit any other symbol into its cell span. */
export function glyphConstraintForCodepoint(codepoint: number): GlyphConstraint {
	return (
		getNerdFontConstraint(codepoint) ??
		(isSymbolCodepoint(codepoint) ? FIT_SYMBOL_CONSTRAINT : NO_GLYPH_CONSTRAINT)
	);
}

/** Resolve symbol spill without materializing a cell array. */
export function cellConstraintWidthFromReader(
	column: number,
	columns: number,
	codepointAt: (index: number) => number,
	widthAt: (index: number) => number,
): 1 | 2 {
	if (column < 0 || column >= columns) return 1;
	const codepoint = codepointAt(column);
	const gridWidth = widthAt(column) >= 2 ? 2 : 1;
	if (gridWidth > 1) return gridWidth;
	if (!isSymbolCodepoint(codepoint)) return gridWidth;
	if (column === columns - 1) return 1;

	const previous = codepointAt(column - 1);
	if (previous !== 0 && isSymbolCodepoint(previous) && !isGraphicsElement(previous)) {
		return 1;
	}

	const nextCodepoint = codepointAt(column + 1);
	return nextCodepoint === 0 || isExpandableSpace(nextCodepoint) ? 2 : 1;
}

/** Scale and align a glyph using Ghostty's Nerd Font constraint geometry. */
export function constrainGlyph(
	glyph: GlyphSize,
	metrics: DetailedFontMetrics,
	constraint: GlyphConstraint,
	constraintWidth: 1 | 2,
): GlyphSize {
	if (!doesAnything(constraint) || glyph.width <= 0 || glyph.height <= 0) return glyph;
	if (constraint.size !== 'stretch') {
		return constrainGlyphInner(glyph, metrics, constraint, constraintWidth);
	}

	const gridMetrics: DetailedFontMetrics = {
		...metrics,
		faceWidth: metrics.cellWidth,
		faceHeight: metrics.cellHeight,
		faceY: 0,
	};
	return constrainGlyphInner(
		glyph,
		gridMetrics,
		{
			...constraint,
			padTop: Math.max(0, constraint.padTop),
			padLeft: Math.max(0, constraint.padLeft),
			padRight: Math.max(0, constraint.padRight),
			padBottom: Math.max(0, constraint.padBottom),
		},
		constraintWidth,
	);
}

export function isSymbolCodepoint(codepoint: number): boolean {
	if (!Number.isInteger(codepoint) || codepoint < 0 || codepoint > 0x10ffff) return false;
	if (
		(codepoint >= 0xe000 && codepoint <= 0xf8ff) ||
		(codepoint >= 0xf0000 && codepoint <= 0xffffd) ||
		(codepoint >= 0x100000 && codepoint <= 0x10fffd)
	) {
		return true;
	}
	return SYMBOL.test(String.fromCodePoint(codepoint));
}

function constrainGlyphInner(
	glyph: GlyphSize,
	metrics: DetailedFontMetrics,
	constraint: GlyphConstraint,
	constraintWidth: 1 | 2,
): GlyphSize {
	let availableWidth: 1 | 2 = 1;
	const forceSingleCell =
		constraint.size === 'stretch' && metrics.faceWidth > 0.9 * metrics.faceHeight;
	if (!forceSingleCell && constraintWidth > 1 && constraint.maxConstraintWidth > 1) {
		availableWidth = 2;
	}
	const groupWidth = glyph.width / constraint.relativeWidth;
	const groupHeight = glyph.height / constraint.relativeHeight;
	const group: MutableGlyphSize = {
		width: groupWidth,
		height: groupHeight,
		x: glyph.x - groupWidth * constraint.relativeX,
		y: glyph.y - groupHeight * constraint.relativeY,
	};
	const [widthFactor, heightFactor] = scaleFactors(group, metrics, constraint, availableWidth);
	const centerX = group.x + group.width / 2;
	const centerY = group.y + group.height / 2;
	group.width *= widthFactor;
	group.height *= heightFactor;
	group.x = centerX - group.width / 2;
	group.y = centerY - group.height / 2;
	group.y = alignedY(group, metrics, constraint);
	group.x = alignedX(group, metrics, constraint, availableWidth);

	return {
		width: widthFactor * glyph.width,
		height: heightFactor * glyph.height,
		x: group.x + group.width * constraint.relativeX,
		y: group.y + group.height * constraint.relativeY,
	};
}

function scaleFactors(
	group: GlyphSize,
	metrics: DetailedFontMetrics,
	constraint: GlyphConstraint,
	width: 1 | 2,
): readonly [number, number] {
	if (constraint.size === 'none') return [1, 1];
	const multipleCells = width > 1;
	const targetWidth = (width - constraint.padLeft - constraint.padRight) * metrics.faceWidth;
	let heightMetric = metrics.faceHeight;
	if (constraint.height === 'icon') {
		heightMetric = multipleCells ? metrics.iconHeight : metrics.iconHeightSingle;
	}
	const targetHeight = (1 - constraint.padBottom - constraint.padTop) * heightMetric;
	let widthFactor = targetWidth / group.width;
	let heightFactor = targetHeight / group.height;

	switch (constraint.size) {
		case 'fit':
			heightFactor = Math.min(1, widthFactor, heightFactor);
			widthFactor = heightFactor;
			break;
		case 'cover':
			heightFactor = Math.min(widthFactor, heightFactor);
			widthFactor = heightFactor;
			break;
		case 'fit_cover1':
			heightFactor = Math.min(widthFactor, heightFactor);
			if (multipleCells && heightFactor > 1) {
				const singleCellFactor = scaleFactors(group, metrics, constraint, 1)[1];
				heightFactor = Math.max(1, singleCellFactor);
			}
			widthFactor = heightFactor;
			break;
		case 'stretch':
			break;
	}

	if (
		constraint.maxXyRatio !== undefined &&
		group.width * widthFactor > group.height * heightFactor * constraint.maxXyRatio
	) {
		widthFactor = (group.height * heightFactor * constraint.maxXyRatio) / group.width;
	}
	return [widthFactor, heightFactor];
}

function alignedY(
	group: GlyphSize,
	metrics: DetailedFontMetrics,
	constraint: GlyphConstraint,
): number {
	if (constraint.size === 'none' && constraint.alignVertical === 'none') return group.y;
	const start = metrics.faceY + constraint.padBottom * metrics.faceHeight;
	const end =
		metrics.faceY + metrics.faceHeight - group.height - constraint.padTop * metrics.faceHeight;
	const center = (start + end) / 2;
	switch (constraint.alignVertical) {
		case 'none':
			return end < start ? center : Math.max(start, Math.min(group.y, end));
		case 'start':
			return start;
		case 'end':
			return end;
		case 'center':
		case 'center1':
			return center;
	}
}

function alignedX(
	group: GlyphSize,
	metrics: DetailedFontMetrics,
	constraint: GlyphConstraint,
	width: 1 | 2,
): number {
	if (constraint.size === 'none' && constraint.alignHorizontal === 'none') return group.x;
	const fullFaceSpan = metrics.faceWidth + (width - 1) * metrics.cellWidth;
	const start = constraint.padLeft * metrics.faceWidth;
	const end = fullFaceSpan - group.width - constraint.padRight * metrics.faceWidth;
	switch (constraint.alignHorizontal) {
		case 'none':
			return Math.max(start, Math.min(group.x, end));
		case 'start':
			return start;
		case 'end':
			return Math.max(start, end);
		case 'center':
			return Math.max(start, (start + end) / 2);
		case 'center1': {
			const firstCellEnd =
				metrics.faceWidth - group.width - constraint.padRight * metrics.faceWidth;
			return Math.max(start, (start + firstCellEnd) / 2);
		}
	}
}

function resolveConstraint(generated: GeneratedGlyphConstraint): GlyphConstraint {
	return { ...NO_GLYPH_CONSTRAINT, ...generated };
}

function doesAnything(constraint: GlyphConstraint): boolean {
	return (
		constraint.size !== 'none' ||
		constraint.alignHorizontal !== 'none' ||
		constraint.alignVertical !== 'none'
	);
}

function isExpandableSpace(codepoint: number): boolean {
	return codepoint === 0x20 || codepoint === 0x2002;
}

function isGraphicsElement(codepoint: number): boolean {
	return (
		(codepoint >= 0x2500 && codepoint <= 0x257f) ||
		(codepoint >= 0x2580 && codepoint <= 0x259f) ||
		(codepoint >= 0x1fb00 && codepoint <= 0x1fbff) ||
		(codepoint >= 0x1cc00 && codepoint <= 0x1cebf) ||
		(codepoint >= 0xe0b0 && codepoint <= 0xe0d7)
	);
}

type MutableGlyphSize = { -readonly [K in keyof GlyphSize]: GlyphSize[K] };
