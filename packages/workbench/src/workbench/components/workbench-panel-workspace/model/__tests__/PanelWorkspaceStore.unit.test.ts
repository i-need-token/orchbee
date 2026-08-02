import { TabModel } from '@orchbee/tab-strip';
import { describe, expect, it, vi } from 'vitest';

import { terminalPresentationId } from '../../../../../terminal-presentation';
import { findPanel } from '../panel-workspace-state';
import { PanelWorkspaceStore } from '../PanelWorkspaceStore';

function createPreviewComposition(scope: string) {
	let sequence = 1;
	return {
		createPresentationId: () => terminalPresentationId(`${scope}:dynamic:${sequence++}`),
		initialPresentationId: terminalPresentationId(`${scope}:initial`),
		kind: 'preview' as const,
	};
}

describe('PanelWorkspaceStore', () => {
	it('keeps the stable initial identity and never reuses a removed dynamic identity', () => {
		const tabModel = new TabModel({
			activeTabId: 'tab-a',
			tabs: [{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' }],
		});
		const store = new PanelWorkspaceStore(tabModel, {
			composition: createPreviewComposition('identity'),
		});
		const initialTab = store.getSnapshot().tabs[0];
		if (initialTab === undefined) throw new Error('Expected initial Tab.');
		expect(findPanel(initialTab.root, 'panel-1')?.primaryPresentation.presentationId).toBe(
			'identity:initial',
		);

		store.transition({
			newTerminalTitle: 'Terminal 2',
			panelId: 'panel-1',
			presentation: store.createTerminalPresentation(),
			tabId: 'tab-a',
			type: 'child-terminal-created',
		});
		const firstId = findPanel(store.getSnapshot().tabs[0]?.root ?? initialTab.root, 'panel-1')
			?.childPresentations[0]?.presentationId;
		if (firstId === undefined) throw new Error('Expected first child presentation.');
		store.transition({
			childPresentationId: firstId,
			close: { kind: 'preview' },
			panelId: 'panel-1',
			tabId: 'tab-a',
			type: 'child-terminal-closed',
		});
		store.transition({
			newTerminalTitle: 'Terminal 3',
			panelId: 'panel-1',
			presentation: store.createTerminalPresentation(),
			tabId: 'tab-a',
			type: 'child-terminal-created',
		});
		const secondId = findPanel(store.getSnapshot().tabs[0]?.root ?? initialTab.root, 'panel-1')
			?.childPresentations[0]?.presentationId;

		expect(firstId).toBe('identity:dynamic:1');
		expect(secondId).toBe('identity:dynamic:2');
	});

	it('publishes semantic transitions and reconciles the host-owned tab collection', () => {
		const tabModel = new TabModel({
			activeTabId: 'tab-a',
			tabs: [{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' }],
		});
		const store = new PanelWorkspaceStore(tabModel, {
			composition: createPreviewComposition('reconciliation'),
		});
		const listener = vi.fn<() => void>();
		const unsubscribe = store.subscribe(listener);

		store.transition({
			direction: 'right',
			newTerminalTitle: 'Terminal 2',
			panelId: 'panel-1',
			presentation: store.createTerminalPresentation(),
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

	it('uses a prepared CWD when reconciling a new Tab', () => {
		const tabModel = new TabModel({
			tabs: [{ group: null, id: 'tab-a', pinned: false, title: 'Terminal A' }],
			activeTabId: 'tab-a',
		});
		const store = new PanelWorkspaceStore(tabModel, {
			composition: createPreviewComposition('prepared-cwd'),
			initialCwd: '/initial',
		});
		store.prepareTabCwd('tab-b', '/feature');
		tabModel.addTab({ title: 'Terminal B' }, { id: 'tab-b' });
		store.reconcileTabs();

		expect(store.getSnapshot().tabs[0]?.root).toMatchObject({
			kind: 'panel',
			primaryPresentation: { currentCwd: '/initial', launchCwd: '/initial' },
		});
		expect(store.getSnapshot().tabs[1]?.root).toMatchObject({
			kind: 'panel',
			primaryPresentation: { currentCwd: '/feature', launchCwd: '/feature' },
		});
	});
});
