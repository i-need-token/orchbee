import { TabModel } from '@orchbee/tab-strip';
import { describe, expect, it, vi } from 'vitest';

import {
	createSnapshotRuntime,
	fixturePanelId,
	fixtureTabId,
	oneWorkspaceSnapshot,
	snapshotPanel,
	snapshotSplit,
	snapshotTab,
} from '../../../../../__tests__/server-workbench-snapshot';
import { PanelWorkspaceStore } from '../PanelWorkspaceStore';

describe('PanelWorkspaceStore', () => {
	it('publishes semantic transitions and reconciles the host-owned tab collection', () => {
		const initialState = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [
				snapshotTab(1, snapshotSplit(1, 'horizontal', snapshotPanel(1), snapshotPanel(2))),
				snapshotTab(3),
			]),
		)
			.getActiveTerminalContext()
			.panelWorkspaceStore.getSnapshot();
		const tabModel = new TabModel({
			activeTabId: fixtureTabId(1),
			tabs: [
				{ group: null, id: fixtureTabId(1), pinned: false, title: 'Terminal A' },
				{ group: null, id: fixtureTabId(3), pinned: false, title: 'Terminal B' },
			],
		});
		const store = new PanelWorkspaceStore(tabModel, {
			initialState,
			tabTitlePolicies: [
				{ customTitle: 'Terminal A', fallbackTitle: 'Tab 1', tabId: fixtureTabId(1) },
				{ customTitle: 'Terminal B', fallbackTitle: 'Tab 3', tabId: fixtureTabId(3) },
			],
		});
		const listener = vi.fn<() => void>();
		const unsubscribe = store.subscribe(listener);

		store.transition({
			panelId: fixturePanelId(2),
			tabId: fixtureTabId(1),
			type: 'panel-focused',
		});
		tabModel.moveTabFirst(fixtureTabId(3));

		expect(store.getSnapshot().tabs.map((tab) => tab.tabId)).toEqual([
			fixtureTabId(3),
			fixtureTabId(1),
		]);
		expect(store.getSnapshot().tabs[1]?.focusedPanelId).toBe(fixturePanelId(2));
		expect(store.getActiveTabId()).toBe(fixtureTabId(1));
		expect(listener).toHaveBeenCalledTimes(2);

		unsubscribe();
		tabModel.moveTabFirst(fixtureTabId(1));
		expect(store.getSnapshot().tabs.map((tab) => tab.tabId)).toEqual([
			fixtureTabId(3),
			fixtureTabId(1),
		]);
	});
});
