import { describe, expect, it } from 'vitest';
import corpus from '../../../../../../schemas/workbench/v1/tab-configuration-conformance-v1.json';
import { TabModel, type TabGroupColor } from '../tab-model';

function color(value: string): TabGroupColor {
	switch (value) {
		case 'blue':
		case 'cyan':
		case 'green':
		case 'grey':
		case 'orange':
		case 'pink':
		case 'purple':
		case 'red':
		case 'yellow':
			return value;
		default:
			throw new Error(`Invalid corpus color ${value}`);
	}
}

function required(value: string | null | undefined): string {
	if (typeof value !== 'string') throw new Error('Missing corpus operation field');
	return value;
}

describe('canonical Tab configuration parity corpus', () => {
	for (const scenario of corpus.cases) {
		it(scenario.name, () => {
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
			switch (operation.kind) {
				case 'create-group':
					model.createGroup(
						{
							id: required(scenario.createdGroupId),
							title: required(operation.label),
							color: color(required(operation.color)),
							collapsed: false,
						},
						[required(operation.tabId)],
					);
					break;
				case 'set-group':
					model.moveTabsToGroup([required(operation.tabId)], operation.groupId ?? null);
					break;
				case 'update-group':
					model.updateGroup(required(operation.groupId), {
						title: required(operation.label),
						color: color(required(operation.color)),
					});
					break;
				case 'ungroup':
					model.ungroup(required(operation.groupId));
					break;
				case 'set-pinned': {
					if (typeof operation.pinned !== 'boolean') throw new Error('Missing pinned value');
					model.setTabsPinned([required(operation.tabId)], operation.pinned);
					break;
				}
				case 'move-group':
					model.shiftGroupRelative(
						required(operation.groupId),
						operation.direction === 'previous' ? -1 : 1,
					);
					break;
				default:
					throw new Error(`Unknown corpus operation ${operation.kind}`);
			}
			const snapshot = model.getSnapshot();
			expect({
				tabs: snapshot.tabs,
				groups: snapshot.groups.map(({ id, title, color: groupColor }) => ({
					id,
					title,
					color: groupColor,
				})),
			}).toEqual(scenario.expected);
			expect(snapshot.activeTabId).toBe(scenario.activeTabId);
		});
	}
});
