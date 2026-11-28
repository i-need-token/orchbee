import { ORCHBEE_LOCALES } from '@orchbee/i18n';
import { describe, expect, it } from 'vitest';

import { explorerItemComparer } from '../explorerItemComparer';
import fixture from './fixtures/vscode-explorer-order.json';

// Each order is the pinned VS Code's Explorer order of the same entries, sorted by its own
// comparer with Intl's default locale set to the UI locale.
const entries = fixture.entries.map(([name, directory]) => {
	if (typeof name !== 'string' || typeof directory !== 'boolean') {
		throw new Error('Malformed Explorer order fixture entry.');
	}
	return { directory, name };
});

describe('explorerItemComparer', () => {
	it.each(ORCHBEE_LOCALES)('orders a folder as VS Code does in %s', (locale) => {
		const sorted = [...entries].sort(explorerItemComparer(locale)).map((entry) => entry.name);

		expect(sorted).toEqual(fixture.orders[locale]);
	});
});
