import { TabModel } from '@orchbee/tab-strip';
import { describe, expect, it, vi } from 'vitest';

import { PanelWorkspaceStore } from '../PanelWorkspaceStore';

describe('PanelWorkspaceStore', () => {
	it('publishes semantic transitions and reconciles the host-owned tab collection', () => {
		const tabModel = new TabModel({
			activeTabId: 'tab-a',
			tabs: [{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' }],
		});
		const store = new PanelWorkspaceStore(tabModel);
		const listener = vi.fn<() => void>();
		const unsubscribe = store.subscribe(listener);

		store.transition({
			direction: 'right',
			newTerminalTitle: 'Terminal 2',
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'panel-split',
		});
		tabModel.addTab({ title: 'Terminal B' }, { id: 'tab-b' });

		expect(store.getSnapshot().tabs.map((tab) => tab.tabId)).toEqual(['tab-a', 'tab-b']);
		expect(store.getActiveTabId()).toBe('tab-b');
		expect(listener).toHaveBeenCalledTimes(2);

		unsubscribe();
		tabModel.addTab({ title: 'Terminal C' }, { id: 'tab-c' });
		expect(store.getSnapshot().tabs.map((tab) => tab.tabId)).toEqual(['tab-a', 'tab-b']);
	});
});
