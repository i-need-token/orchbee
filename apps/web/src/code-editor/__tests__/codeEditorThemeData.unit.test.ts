import { readFileSync } from 'node:fs';

import { describe, expect, it } from 'vitest';

import { codeEditorThemeData } from '../codeEditorThemeData';

/**
 * The theme reads its tokens by name while the page runs, where no style check sees them: a token
 * that the registry renamed or never declared resolves to nothing, and its colors silently stay at
 * Monaco's defaults. The registry declares every system color in each theme, which the design-token
 * checks enforce, so a name declared here exists everywhere.
 */
const registeredColors = new Set(
	Array.from(
		readFileSync(
			new URL('../../../../../packages/design-tokens/src/tokens.css', import.meta.url),
			'utf8',
		).matchAll(/(--ob-sys-color-[\w-]+)\s*:/gu),
		(match) => match[1],
	),
);

describe('codeEditorThemeData', () => {
	it('reads only registered system colors', () => {
		const read = new Set<string>();
		const record = (token: string): string => {
			read.add(token);
			return '#808080';
		};
		for (const dark of [false, true])
			codeEditorThemeData({ color: record, onCanvas: record }, dark);
		expect(read.size).toBeGreaterThan(0);
		expect([...read].filter((token) => !registeredColors.has(token))).toEqual([]);
	});
});
