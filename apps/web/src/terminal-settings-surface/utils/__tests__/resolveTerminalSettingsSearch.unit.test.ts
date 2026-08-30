import { describe, expect, it } from 'vitest';

import type { TerminalSettingsSearchEntry } from '../../types/terminal-settings-surface';
import { resolveTerminalSettingsSearch } from '../resolveTerminalSettingsSearch';

const ENTRIES = [
	entry({ id: 'category:appearance', kind: 'category', label: 'Appearance' }),
	entry({
		description: 'Choose the typeface used by terminal cells.',
		id: 'text.font-family',
		label: 'Terminal font family',
		sectionLabel: 'Typography',
	}),
	entry({
		description: 'Set the text size used by terminal cells.',
		id: 'text.font-size',
		label: 'Terminal font size',
		sectionLabel: 'Typography',
	}),
	entry({
		description: 'Choose between light and dark application chrome.',
		id: 'appearance.application-theme',
		label: 'Application theme',
		sectionLabel: 'Application appearance',
	}),
] as const satisfies readonly TerminalSettingsSearchEntry[];

describe('resolveTerminalSettingsSearch', () => {
	it('returns no search projection for an empty query', () => {
		expect(
			resolveTerminalSettingsSearch({ entries: ENTRIES, locale: 'en-US', query: '  ' }),
		).toEqual([]);
	});

	it('searches every category and setting instead of one active page', () => {
		const results = resolveTerminalSettingsSearch({
			entries: ENTRIES,
			locale: 'en-US',
			query: 'font',
		});

		expect(results.map((result) => result.id)).toEqual(['text.font-family', 'text.font-size']);
	});

	it('matches category labels without flooding results with every setting in that category', () => {
		const results = resolveTerminalSettingsSearch({
			entries: ENTRIES,
			locale: 'en-US',
			query: 'appearance',
		});

		expect(results.map((result) => result.id)).toEqual(['category:appearance']);
	});

	it('matches descriptions and sections while ranking direct labels first', () => {
		const results = resolveTerminalSettingsSearch({
			entries: ENTRIES,
			locale: 'en-US',
			query: 'theme',
		});

		expect(results.map((result) => result.id)).toEqual(['appearance.application-theme']);
		expect(
			resolveTerminalSettingsSearch({ entries: ENTRIES, locale: 'en-US', query: 'typography' }).map(
				(result) => result.id,
			),
		).toEqual(['text.font-family', 'text.font-size']);
	});

	it('requires every normalized query term and preserves catalog order for ties', () => {
		expect(
			resolveTerminalSettingsSearch({
				entries: ENTRIES,
				locale: 'en-US',
				query: 'ＦＯＮＴ cells',
			}).map((result) => result.id),
		).toEqual(['text.font-family', 'text.font-size']);
		expect(
			resolveTerminalSettingsSearch({
				entries: ENTRIES,
				locale: 'en-US',
				query: 'no matching term',
			}),
		).toEqual([]);
	});
});

function entry(
	overrides: Partial<TerminalSettingsSearchEntry> &
		Pick<TerminalSettingsSearchEntry, 'id' | 'label'>,
): TerminalSettingsSearchEntry {
	return {
		category: overrides.id.startsWith('text.') ? 'text' : 'appearance',
		categoryLabel: overrides.id.startsWith('text.') ? 'Text' : 'Appearance',
		description: '',
		icon: 'settings',
		keywords: [],
		kind: 'setting',
		sectionLabel: null,
		targetId: overrides.id,
		...overrides,
	};
}
