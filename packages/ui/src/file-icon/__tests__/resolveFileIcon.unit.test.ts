import { describe, expect, it } from 'vitest';

import { resolveFileIcon } from '../resolveFileIcon';
import fixture from './fixtures/vscode-seti-icons.json';

// Each case's glyph and hue are what the pinned VS Code draws with Seti, read from its own icon
// classes, language registry and icon-theme stylesheet under both the dark and the light theme.
const cases = fixture.cases.map(([path, glyph, hue]) => {
	if (typeof path !== 'string' || typeof glyph !== 'string' || hue === undefined) {
		throw new Error('Malformed file-icon fixture row.');
	}
	return { glyph: String.fromCodePoint(Number.parseInt(glyph, 16)), hue, path };
});

describe('resolveFileIcon', () => {
	it.each(cases)('draws $path as VS Code does', ({ glyph, hue, path }) => {
		const name = path.split(/[\\/]/).pop() ?? path;

		expect(resolveFileIcon(name, path)).toEqual([glyph, hue]);
	});
});
