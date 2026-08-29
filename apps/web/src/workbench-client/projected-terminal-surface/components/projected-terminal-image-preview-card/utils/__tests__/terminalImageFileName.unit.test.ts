import { describe, expect, it } from 'vitest';

import { terminalImageFileName } from '../terminalImageFileName';

describe('terminalImageFileName', () => {
	it.each([
		['/workspace/artifacts/final.png', 'final.png'],
		['C:\\workspace\\artifacts\\final.webp', 'final.webp'],
		['single.gif', 'single.gif'],
	])('extracts the caption from %s', (filePath, expected) => {
		expect(terminalImageFileName(filePath)).toBe(expected);
	});
});
