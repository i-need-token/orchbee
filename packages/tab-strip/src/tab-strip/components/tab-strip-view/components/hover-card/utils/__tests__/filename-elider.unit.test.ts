import { describe, expect, it } from 'vitest';
import {
	ELLIPSIS,
	elideFilenameWithLineLengths,
	findImageDimensions,
	FIRST_STRONG_ISOLATE,
	getFadeOutOpacity,
	interpolateFadeSize,
	POP_DIRECTIONAL_ISOLATE,
} from '../filename-elider';

describe('FilenameElider', () => {
	it('recognizes only terminal image dimensions using the multiplication sign', () => {
		expect(findImageDimensions('photo.png (640×480)')).toBe(10);
		expect(findImageDimensions('photo.png (640x480)')).toBeNull();
		expect(findImageDimensions('(640×480)')).toBeNull();
		expect(findImageDimensions('photo (640×480) copy')).toBeNull();
	});

	it('keeps a fitting filename unchanged and returns empty when a line cannot fit', () => {
		expect(elideFilenameWithLineLengths('file.txt', { first: 8, second: 8 })).toBe('file.txt');
		expect(elideFilenameWithLineLengths('file.txt', { first: 0, second: 3 })).toBe('');
	});

	it('prefers putting an extension on an isolated second line', () => {
		const text = 'verylongfilename.txt';
		expect(elideFilenameWithLineLengths(text, { first: 16, second: 7 })).toBe(
			`verylongfilename\n${FIRST_STRONG_ISOLATE}.txt${POP_DIRECTIONAL_ISOLATE}`,
		);
	});

	it('uses an isolated ellipsis when the two lines do not overlap', () => {
		expect(elideFilenameWithLineLengths('abcdefghijklmnop', { first: 5, second: 4 })).toBe(
			`abcde\n${FIRST_STRONG_ISOLATE}${ELLIPSIS}mnop${POP_DIRECTIONAL_ISOLATE}`,
		);
	});
});

describe('FadeView math', () => {
	it('fades the old view out and interpolates its maximum size', () => {
		expect(getFadeOutOpacity(0.25)).toBe(0.75);
		expect(getFadeOutOpacity(2)).toBe(0);
		expect(
			interpolateFadeSize(0.5, { width: 100, height: 40 }, { width: 200, height: 60 }),
		).toEqual({
			width: 150,
			height: 50,
		});
	});
});
