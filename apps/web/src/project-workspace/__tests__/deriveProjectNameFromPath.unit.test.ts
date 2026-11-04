import { describe, expect, it } from 'vitest';

import { deriveProjectNameFromPath } from '../deriveProjectNameFromPath';

describe('deriveProjectNameFromPath', () => {
	it.each([
		['/workspace/orchbee', 'orchbee'],
		['/workspace/orchbee///', 'orchbee'],
		['C:\\Users\\orchbee', 'orchbee'],
		['C:\\Users\\orchbee\\', 'orchbee'],
		['~/projects/orchbee', 'orchbee'],
		[' /workspace/orchbee ', 'orchbee'],
		['/', '/'],
		['C:\\', 'C:'],
	])('derives %s as %s', (path, expectedName) => {
		expect(deriveProjectNameFromPath(path)).toBe(expectedName);
	});
});
