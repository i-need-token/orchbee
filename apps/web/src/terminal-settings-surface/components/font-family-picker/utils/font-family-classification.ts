/** An installed family as the font list offers it. */
export interface InstalledFontFamily {
	readonly family: string;
	/** Every letter takes the same width, which a terminal's grid needs. */
	readonly fixedWidth: boolean;
	/** Its letters are pictures, so the list draws its name in the normal face. */
	readonly symbol: boolean;
}

/**
 * Families that put pictures where letters belong. A family without Latin letters needs no entry:
 * its name falls back to the normal face by itself.
 */
const SYMBOL_FAMILY =
	/dingbat|webdings|wingdings|marlett|symbol|mdl2|icons|mt extra|bookshelf|ms outlook|specialty/iu;

const MEASURE_SIZE = '32px';
/** Ten narrow and ten wide letters; a fixed-width face draws both runs at one width. */
const NARROW_RUN = 'iiiiiiiiii';
const WIDE_RUN = 'MMMMMMMMMM';
/** The share of the wide run's width the two runs may differ by and still count as one width. */
const WIDTH_TOLERANCE = 0.02;

const classified = new WeakMap<readonly string[], readonly InstalledFontFamily[]>();

/**
 * Sorts the listed families into fixed-width and other faces by measuring two runs of letters in
 * each. A family the canvas cannot draw falls back to a serif face and so counts as not fixed-width.
 * The answer for one listing is kept, since a listing does not change while the document lives.
 */
export function classifyFontFamilies(families: readonly string[]): readonly InstalledFontFamily[] {
	const known = classified.get(families);
	if (known !== undefined) return known;
	const context = document.createElement('canvas').getContext('2d');
	const result = families.map((family) => ({
		family,
		fixedWidth: context !== null && isFixedWidth(context, family),
		symbol: SYMBOL_FAMILY.test(family),
	}));
	classified.set(families, result);
	return result;
}

function isFixedWidth(context: CanvasRenderingContext2D, family: string): boolean {
	// An unparseable font string leaves the previous font in place, so each probe starts from serif.
	context.font = `${MEASURE_SIZE} serif`;
	context.font = `${MEASURE_SIZE} ${cssFamilyName(family)}, serif`;
	const narrow = context.measureText(NARROW_RUN).width;
	const wide = context.measureText(WIDE_RUN).width;
	return wide > 0 && Math.abs(wide - narrow) <= wide * WIDTH_TOLERANCE;
}

/** One family name as a quoted CSS string. */
export function cssFamilyName(family: string): string {
	return `"${family.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}"`;
}
