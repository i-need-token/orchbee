import type { TabData } from './tab-model';

/** Disclosure and local marking never exclude canonical group members from close. */
export function getGroupCloseTabIds(
	tabs: readonly Pick<TabData, 'id' | 'group'>[],
	group: string,
): string[] {
	return tabs.filter((tab) => tab.group === group).map((tab) => tab.id);
}

/** The context-menu target determines whether a local selection participates in close. */
export function getTabCloseTargets(
	tabs: readonly Pick<TabData, 'id' | 'pinned' | 'group'>[],
	selected: ReadonlySet<string>,
	target: string,
): { selected: string[]; other: string[]; trailing: string[]; group: string[] } {
	const index = tabs.findIndex((tab) => tab.id === target);
	const invoked = tabs[index];
	if (invoked === undefined) return { selected: [], other: [], trailing: [], group: [] };
	const inSelection = selected.has(target);
	const lastSelected = inSelection
		? tabs.reduce((last, tab, position) => (selected.has(tab.id) ? position : last), index)
		: index;
	const relative = (after: number) =>
		tabs.flatMap((tab, position) =>
			position > after && !tab.pinned && tab.id !== target && !(inSelection && selected.has(tab.id))
				? [tab.id]
				: [],
		);
	return {
		selected: inSelection
			? tabs.filter((tab) => selected.has(tab.id)).map((tab) => tab.id)
			: [target],
		other: relative(-1),
		trailing: relative(lastSelected),
		group: invoked.group === null ? [] : getGroupCloseTabIds(tabs, invoked.group),
	};
}
