import type { TabModelSnapshot } from '../model/tab-model';

export type TabStripRovingItem =
	| {
			readonly groupId: string;
			readonly key: `group:${string}`;
			readonly kind: 'group';
			readonly representedTabIds: readonly string[];
	  }
	| { readonly key: `tab:${string}`; readonly kind: 'tab'; readonly tabId: string };

export type TabStripRovingNavigation = 'first' | 'last' | 'next' | 'previous';

interface TabStripKeyboardEvent {
	readonly altKey: boolean;
	readonly ctrlKey: boolean;
	readonly key: string;
	readonly metaKey: boolean;
	readonly shiftKey: boolean;
}

export function isTabStripKeyboardContextMenuRequest(event: TabStripKeyboardEvent): boolean {
	return (
		event.key === 'ContextMenu' ||
		(event.key === 'F10' && event.shiftKey && !event.altKey && !event.ctrlKey && !event.metaKey)
	);
}

export function resolveTabStripRovingNavigationCommand(
	event: TabStripKeyboardEvent,
	horizontal: boolean,
): TabStripRovingNavigation | null {
	if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return null;
	if (event.key === 'Home') return 'first';
	if (event.key === 'End') return 'last';
	if (event.key === (horizontal ? 'ArrowLeft' : 'ArrowUp')) return 'previous';
	if (event.key === (horizontal ? 'ArrowRight' : 'ArrowDown')) return 'next';
	return null;
}

export function projectTabStripRovingItems(
	snapshot: Pick<TabModelSnapshot, 'groups' | 'tabs'>,
): readonly TabStripRovingItem[] {
	const groups = new Map(snapshot.groups.map((group) => [group.id, group]));
	const emittedGroups = new Set<string>();
	const items: TabStripRovingItem[] = [];

	for (const tab of snapshot.tabs) {
		if (tab.group === null) {
			items.push({ key: `tab:${tab.id}`, kind: 'tab', tabId: tab.id });
			continue;
		}

		const group = groups.get(tab.group);
		if (group === undefined) continue;
		if (!emittedGroups.has(group.id)) {
			emittedGroups.add(group.id);
			items.push({
				groupId: group.id,
				key: `group:${group.id}`,
				kind: 'group',
				representedTabIds: group.collapsed
					? snapshot.tabs
							.filter((candidate) => candidate.group === group.id)
							.map((candidate) => candidate.id)
					: [],
			});
		}
		if (!group.collapsed) {
			items.push({ key: `tab:${tab.id}`, kind: 'tab', tabId: tab.id });
		}
	}

	return items;
}

export function resolveTabStripRovingFocus({
	activeTabId,
	items,
	preferredKey,
	previousIndex,
}: {
	readonly activeTabId: string | null;
	readonly items: readonly TabStripRovingItem[];
	readonly preferredKey: TabStripRovingItem['key'] | null;
	readonly previousIndex: number | null;
}): TabStripRovingItem['key'] | null {
	if (items.length === 0) return null;

	if (preferredKey !== null) {
		const preferred = items.find((item) => item.key === preferredKey);
		if (preferred !== undefined) return preferred.key;

		if (preferredKey.startsWith('tab:')) {
			const preferredTabId = preferredKey.slice('tab:'.length);
			const representative = items.find(
				(item) => item.kind === 'group' && item.representedTabIds.includes(preferredTabId),
			);
			if (representative !== undefined) return representative.key;
		}

		if (previousIndex !== null) {
			return items[Math.min(Math.max(previousIndex, 0), items.length - 1)]?.key ?? null;
		}
	}

	if (activeTabId !== null) {
		const active = items.find(
			(item) =>
				(item.kind === 'tab' && item.tabId === activeTabId) ||
				(item.kind === 'group' && item.representedTabIds.includes(activeTabId)),
		);
		if (active !== undefined) return active.key;
	}

	return items[0]?.key ?? null;
}

export function resolveTabStripRovingNavigation({
	command,
	currentKey,
	items,
}: {
	readonly command: TabStripRovingNavigation;
	readonly currentKey: TabStripRovingItem['key'] | null;
	readonly items: readonly TabStripRovingItem[];
}): TabStripRovingItem['key'] | null {
	if (items.length === 0) return null;
	if (command === 'first') return items[0]?.key ?? null;
	if (command === 'last') return items.at(-1)?.key ?? null;

	const currentIndex = items.findIndex((item) => item.key === currentKey);
	if (currentIndex === -1) return items[0]?.key ?? null;
	const offset = command === 'next' ? 1 : -1;
	const targetIndex = (currentIndex + offset + items.length) % items.length;
	return items[targetIndex]?.key ?? null;
}
