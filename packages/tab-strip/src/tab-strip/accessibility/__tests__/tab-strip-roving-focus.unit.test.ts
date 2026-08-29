import { describe, expect, it } from 'vitest';

import type { TabModelSnapshot } from '../../model/tab-model';
import {
	isTabStripKeyboardContextMenuRequest,
	projectTabStripRovingItems,
	resolveTabStripRovingFocus,
	resolveTabStripRovingNavigation,
	resolveTabStripRovingNavigationCommand,
} from '../tab-strip-roving-focus';

function keyboardEvent(
	key: string,
	modifiers: Partial<{
		readonly altKey: boolean;
		readonly ctrlKey: boolean;
		readonly metaKey: boolean;
		readonly shiftKey: boolean;
	}> = {},
) {
	return {
		altKey: false,
		ctrlKey: false,
		key,
		metaKey: false,
		shiftKey: false,
		...modifiers,
	};
}

function snapshot({
	activeTabId = 'b',
	collapsed = false,
}: {
	readonly activeTabId?: string | null;
	readonly collapsed?: boolean;
} = {}): Pick<TabModelSnapshot, 'groups' | 'tabs'> & { readonly activeTabId: string | null } {
	return {
		activeTabId,
		groups: [{ attention: false, collapsed, color: 'blue', id: 'group', title: 'Group' }],
		tabs: [
			{ group: null, id: 'a', pinned: true, title: 'A' },
			{ group: 'group', id: 'b', pinned: false, title: 'B' },
			{ group: 'group', id: 'c', pinned: false, title: 'C' },
			{ group: null, id: 'd', pinned: false, title: 'D' },
		],
	};
}

describe('Tab Strip roving focus', () => {
	it('projects visible Tabs and one group header in stable visual order', () => {
		expect(projectTabStripRovingItems(snapshot())).toEqual([
			{ key: 'tab:a', kind: 'tab', tabId: 'a' },
			{
				groupId: 'group',
				key: 'group:group',
				kind: 'group',
				representedTabIds: [],
			},
			{ key: 'tab:b', kind: 'tab', tabId: 'b' },
			{ key: 'tab:c', kind: 'tab', tabId: 'c' },
			{ key: 'tab:d', kind: 'tab', tabId: 'd' },
		]);
	});

	it('replaces collapsed group children with their visible group header', () => {
		expect(projectTabStripRovingItems(snapshot({ collapsed: true }))).toEqual([
			{ key: 'tab:a', kind: 'tab', tabId: 'a' },
			{
				groupId: 'group',
				key: 'group:group',
				kind: 'group',
				representedTabIds: ['b', 'c'],
			},
			{ key: 'tab:d', kind: 'tab', tabId: 'd' },
		]);
	});

	it('initializes from the active Tab or its collapsed-group representative', () => {
		const expanded = projectTabStripRovingItems(snapshot());
		expect(
			resolveTabStripRovingFocus({
				activeTabId: 'b',
				items: expanded,
				preferredKey: null,
				previousIndex: null,
			}),
		).toBe('tab:b');

		const collapsed = projectTabStripRovingItems(snapshot({ collapsed: true }));
		expect(
			resolveTabStripRovingFocus({
				activeTabId: 'b',
				items: collapsed,
				preferredKey: null,
				previousIndex: null,
			}),
		).toBe('group:group');
	});

	it('keeps a live remembered item and maps a newly hidden Tab to its group', () => {
		const expanded = projectTabStripRovingItems(snapshot());
		expect(
			resolveTabStripRovingFocus({
				activeTabId: 'b',
				items: expanded,
				preferredKey: 'tab:c',
				previousIndex: 3,
			}),
		).toBe('tab:c');

		const collapsed = projectTabStripRovingItems(snapshot({ collapsed: true }));
		expect(
			resolveTabStripRovingFocus({
				activeTabId: 'b',
				items: collapsed,
				preferredKey: 'tab:c',
				previousIndex: 3,
			}),
		).toBe('group:group');
	});

	it('chooses the next neighbor at a removed index and the previous neighbor at the end', () => {
		const items = projectTabStripRovingItems(snapshot()).filter((item) => item.key !== 'tab:c');
		expect(
			resolveTabStripRovingFocus({
				activeTabId: 'b',
				items,
				preferredKey: 'tab:c',
				previousIndex: 3,
			}),
		).toBe('tab:d');
		expect(
			resolveTabStripRovingFocus({
				activeTabId: 'b',
				items: items.filter((item) => item.key !== 'tab:d'),
				preferredKey: 'tab:d',
				previousIndex: 3,
			}),
		).toBe('tab:b');
	});

	it('wraps relative navigation and resolves both boundaries directly', () => {
		const items = projectTabStripRovingItems(snapshot({ collapsed: true }));
		expect(resolveTabStripRovingNavigation({ command: 'next', currentKey: 'tab:d', items })).toBe(
			'tab:a',
		);
		expect(
			resolveTabStripRovingNavigation({ command: 'previous', currentKey: 'tab:a', items }),
		).toBe('tab:d');
		expect(resolveTabStripRovingNavigation({ command: 'first', currentKey: null, items })).toBe(
			'tab:a',
		);
		expect(resolveTabStripRovingNavigation({ command: 'last', currentKey: null, items })).toBe(
			'tab:d',
		);
	});

	it('maps orientation-relative keys and exact keyboard context-menu requests', () => {
		expect(resolveTabStripRovingNavigationCommand(keyboardEvent('ArrowRight'), true)).toBe('next');
		expect(resolveTabStripRovingNavigationCommand(keyboardEvent('ArrowDown'), false)).toBe('next');
		expect(resolveTabStripRovingNavigationCommand(keyboardEvent('Home'), true)).toBe('first');
		expect(
			resolveTabStripRovingNavigationCommand(keyboardEvent('ArrowRight', { ctrlKey: true }), true),
		).toBeNull();
		expect(isTabStripKeyboardContextMenuRequest(keyboardEvent('ContextMenu'))).toBe(true);
		expect(isTabStripKeyboardContextMenuRequest(keyboardEvent('F10', { shiftKey: true }))).toBe(
			true,
		);
		expect(isTabStripKeyboardContextMenuRequest(keyboardEvent('F10'))).toBe(false);
	});

	it('falls back to the first visible item and then the new-Tab control boundary', () => {
		const items = projectTabStripRovingItems(snapshot({ activeTabId: null }));
		expect(
			resolveTabStripRovingFocus({
				activeTabId: null,
				items,
				preferredKey: null,
				previousIndex: null,
			}),
		).toBe('tab:a');
		expect(
			resolveTabStripRovingFocus({
				activeTabId: null,
				items: [],
				preferredKey: 'tab:a',
				previousIndex: 0,
			}),
		).toBeNull();
	});
});
