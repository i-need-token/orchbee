import { describe, expect, it } from 'vitest';
import corpus from '../../../../../../schemas/workbench/v1/tab-creation-conformance-v1.json';
import { TabModel, type TabGroupColor } from '../tab-model';

function color(value: string): TabGroupColor {
	if (value === 'blue' || value === 'grey') return value;
	throw new Error(`Unexpected fixture color ${value}`);
}

describe('canonical terminal creation placement matches GUI TabModel', () => {
	// The GUI no longer creates a new group with a Tab; the Rust corpus test still covers that placement.
	for (const scenario of corpus.cases.filter((entry) => entry.operation?.kind !== 'new-group')) {
		it(`${scenario.name}`, () => {
			const model = new TabModel({
				tabs: scenario.tabs,
				groups: scenario.groups.map((group) => ({
					...group,
					color: color(group.color),
					collapsed: false,
				})),
				activeTabId: scenario.activeTabId,
			});
			const operation = scenario.operation;
			let created: string;
			if (operation?.kind === 'duplicate' && operation.tabId !== undefined) {
				created = model.duplicateTab(operation.tabId);
			} else if (operation?.kind === 'after-tab' && operation.tabId !== undefined) {
				const index = model.getIndexOfTab(operation.tabId);
				const source = scenario.tabs.find((tab) => tab.id === operation.tabId);
				if (index === null || source === undefined) throw new Error('Missing relative source');
				created = model.addTab(
					{ title: 'New Tab', group: source.group, pinned: false },
					{ index: index + 1 },
				);
			} else {
				created = model.addTab({ title: 'New Tab', group: operation?.groupId ?? null });
			}
			const snapshot = model.getSnapshot();
			const freshGroup = snapshot.groups.find(
				(group) => !scenario.groups.some((prior) => prior.id === group.id),
			)?.id;
			const normalizeTab = (id: string | null) => (id === created ? scenario.createdTabId : id);
			const normalizeGroup = (id: string | null) =>
				id === freshGroup ? scenario.createdGroupId : id;
			expect({
				tabs: snapshot.tabs.map((tab) => ({
					...tab,
					id: normalizeTab(tab.id),
					group: normalizeGroup(tab.group),
				})),
				groups: snapshot.groups.map((group) => ({
					id: normalizeGroup(group.id),
					title: group.title,
					color: group.color,
				})),
				activeTabId: normalizeTab(snapshot.activeTabId),
			}).toEqual(scenario.expected);
		});
	}
});
