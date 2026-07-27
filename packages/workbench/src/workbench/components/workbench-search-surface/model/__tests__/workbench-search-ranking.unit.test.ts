import type { PreparedActionInvocation } from '@orchbee/actions';
import { describe, expect, it } from 'vitest';

import {
	filterWorkbenchSearchArgumentOptions,
	isWorkbenchSearchResultEnabled,
	resolveWorkbenchSearchHighlight,
	selectWorkbenchSearchSections,
} from '../workbench-search-ranking';
import type { WorkbenchSearchCatalog, WorkbenchSearchResult } from '../workbench-search-result';

const prepared: PreparedActionInvocation = {
	actionId: 'test',
	invocation: {
		scope: { id: 'test', parent: null },
		source: 'command-surface',
	},
};

function result(
	id: string,
	label: string,
	overrides: Partial<WorkbenchSearchResult> = {},
): WorkbenchSearchResult {
	return {
		actionId: id,
		availability: { kind: 'enabled' },
		canonicalLabel: label,
		groupId: 'actions',
		groupLabel: 'Actions',
		groupOrder: 20,
		icon: 'action',
		id,
		invocation: { kind: 'ready', prepared },
		keywords: [],
		label,
		providerId: 'actions',
		recommended: false,
		...overrides,
	};
}

describe('Workbench search ranking', () => {
	it('uses exact, prefix, token, keyword, canonical, then fuzzy title tiers', () => {
		const catalog: WorkbenchSearchCatalog = {
			actionResults: [
				result('fuzzy', 'T-e-r-m-i-n-a-l action'),
				result('canonical', '新建终端', { canonicalLabel: 'Terminal creator' }),
				result('keyword', 'New session', { keywords: ['terminal'] }),
				result('token', 'New terminal'),
				result('prefix', 'Terminal settings'),
				result('exact', 'terminal'),
			],
			recommendedGroupLabel: 'Recommended Actions',
			settingsResults: [],
		};

		const resultIds = selectWorkbenchSearchSections({
			catalog,
			filter: 'actions',
			query: 'terminal',
		}).flatMap((section) => section.results.map((item) => item.id));

		expect(resultIds).toEqual(['exact', 'prefix', 'token', 'keyword', 'canonical', 'fuzzy']);
	});

	it('shows a curated All projection without duplicating Settings in typed results', () => {
		const settings = result('settings', 'Settings', {
			actionId: 'settings',
			groupId: 'settings',
			groupLabel: 'Settings',
			providerId: 'settings',
		});
		const catalog: WorkbenchSearchCatalog = {
			actionResults: [
				result('recommended', 'New Terminal', { recommended: true }),
				result('settings-action', 'Settings', { actionId: 'settings' }),
			],
			recommendedGroupLabel: 'Recommended Actions',
			settingsResults: [settings],
		};

		expect(
			selectWorkbenchSearchSections({ catalog, filter: 'all', query: '' }).map(
				(section) => section.label,
			),
		).toEqual(['Recommended Actions', 'Settings']);
		expect(
			selectWorkbenchSearchSections({ catalog, filter: 'all', query: 'settings' }).flatMap(
				(section) => section.results,
			),
		).toHaveLength(1);
	});

	it('keeps disabled rows searchable and filters nested argument options', () => {
		const disabled = result('disabled', 'Split panel', {
			surfaceDisabledReason: 'Open a panel first',
		});
		expect(isWorkbenchSearchResultEnabled(disabled)).toBe(false);
		expect(resolveWorkbenchSearchHighlight('stale', ['disabled', 'ready'], ['ready'])).toBe(
			'ready',
		);
		expect(
			filterWorkbenchSearchArgumentOptions(
				[
					{ id: 'horizontal', input: {}, keywords: ['top'], label: 'Horizontal tabs' },
					{ id: 'vertical', input: {}, keywords: ['left'], label: 'Vertical tabs' },
				],
				'left',
			).map((option) => option.id),
		).toEqual(['vertical']);
	});
});
