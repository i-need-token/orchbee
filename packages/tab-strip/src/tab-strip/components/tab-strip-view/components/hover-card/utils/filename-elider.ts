import { requireDefined } from '../../../../../utils/requireDefined';
const FIRST_STRONG_ISOLATE = '\u2068';
const POP_DIRECTIONAL_ISOLATE = '\u2069';
const ELLIPSIS = '\u2026';
interface FilenameLineLengths {
	first: number;
	second: number;
}

/** Port of FilenameElider::FindImageDimensions. */
export function findImageDimensions(text: string): number | null {
	const match = / \(\d+×\d+\)$/.exec(text);
	return match?.index === undefined ? null : match.index + 1;
}

/**
 * Port of FilenameElider::ElideImpl. The caller supplies character cut points
 * obtained from its own font-measurement adapter.
 */
export function elideFilenameWithLineLengths(
	text: string,
	lineLengths: Readonly<FilenameLineLengths>,
): string {
	if (
		lineLengths.first < 0 ||
		lineLengths.second < 0 ||
		lineLengths.first > text.length ||
		lineLengths.second > text.length
	) {
		throw new RangeError('Filename line lengths must be within the text.');
	}
	if (lineLengths.first === text.length || lineLengths.second === text.length) return text;
	if (lineLengths.first === 0 || lineLengths.second === 0) return '';
	const secondLineCut = text.length - lineLengths.second;
	let cutPoint = Math.max(secondLineCut, lineLengths.first);
	const isWholeString = cutPoint === lineLengths.first;
	let adjustedCutPoint = false;
	if (isWholeString && cutPoint >= secondLineCut) {
		const dimensions = findImageDimensions(text);
		if (dimensions !== null && dimensions >= secondLineCut && dimensions <= cutPoint) {
			cutPoint = dimensions;
			adjustedCutPoint = true;
		}
		if (!adjustedCutPoint) {
			const extension = text.lastIndexOf('.');
			if (extension >= secondLineCut && extension <= cutPoint) {
				cutPoint = extension;
				adjustedCutPoint = true;
			}
		}
	}
	if (!adjustedCutPoint) {
		while (cutPoint < text.length && /\s/u.test(requireDefined(text[cutPoint]))) cutPoint += 1;
	}
	let result = `${text.slice(0, Math.min(lineLengths.first, cutPoint))}\n`;
	if (adjustedCutPoint || !isWholeString) result += FIRST_STRONG_ISOLATE;
	if (!isWholeString) result += ELLIPSIS;
	result += text.slice(cutPoint);
	if (adjustedCutPoint || !isWholeString) result += POP_DIRECTIONAL_ISOLATE;
	return result;
}
