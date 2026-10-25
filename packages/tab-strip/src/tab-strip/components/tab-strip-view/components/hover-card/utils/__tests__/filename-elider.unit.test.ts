import { describe, expect, it } from 'vitest';
import { elideFilenameWithLineLengths, findImageDimensions } from '../filename-elider';

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
			`verylongfilename\n${'\u2068'}.txt${'\u2069'}`,
		);
	});

	it('uses an isolated ellipsis when the two lines do not overlap', () => {
		expect(elideFilenameWithLineLengths('abcdefghijklmnop', { first: 5, second: 4 })).toBe(
			`abcde\n${'\u2068'}${'\u2026'}mnop${'\u2069'}`,
		);
	});
});
