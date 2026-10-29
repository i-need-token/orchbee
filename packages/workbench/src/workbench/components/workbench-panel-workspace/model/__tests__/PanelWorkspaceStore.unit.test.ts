import { TabModel } from '@orchbee/tab-strip';
import { describe, expect, it, vi } from 'vitest';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '@orchbee/workbench-client-protocol';

import { findPanel } from '../panel-workspace-state';
import { PanelWorkspaceStore } from '../PanelWorkspaceStore';

type ManagedComposition = Extract<
	ConstructorParameters<typeof PanelWorkspaceStore>[1]['composition'],
	{ readonly kind: 'managed' }
>;
type ManagedCommand = Parameters<ManagedComposition['dispatchCommands']>[0][number];

function createPreviewComposition(scope: string) {
	let sequence = 1;
	return {
		createPresentationId: () => terminalPresentationId(`${scope}:dynamic:${sequence++}`),
		initialPresentationId: terminalPresentationId(`${scope}:initial`),
		kind: 'preview' as const,
	};
}

function createManagedComposition(scope: string, dispatched: ManagedCommand[]): ManagedComposition {
	let operationSequence = 1;
	let presentationSequence = 1;
	let removalSequence = 1;
	return {
		createOperationId: () =>
			terminalPresentationOperationId(`${scope}:operation:${operationSequence++}`),
		createPresentationId: () =>
			terminalPresentationId(`${scope}:dynamic:${presentationSequence++}`),
		createRemovalId: () => terminalPresentationRemovalId(`${scope}:removal:${removalSequence++}`),
		dispatchCommands: (commands) => dispatched.push(...commands),
		initialPresentationId: terminalPresentationId(`${scope}:initial`),
		kind: 'managed',
	};
}

describe('PanelWorkspaceStore', () => {
	it('duplicates metadata into a fresh single terminal without copying the source tree or CWD', () => {
		const tabModel = new TabModel({
			activeTabId: 'tab-a',
			tabs: [{ group: null, id: 'tab-a', pinned: true, title: 'Custom source' }],
		});
		const dispatched: ManagedCommand[] = [];
		const store = new PanelWorkspaceStore(tabModel, {
			composition: createManagedComposition('duplicate', dispatched),
			initialCwd: '/source',
			initialTabTitlePolicies: [
				{ tabId: 'tab-a', customTitle: 'Custom source', fallbackTitle: 'Terminal' },
			],
		});
		const unsubscribe = store.subscribe(() => undefined);
		store.transition({
			direction: 'right',
			newTerminalTitle: 'Split',
			panelId: 'panel-1',
			presentation: store.createTerminalPresentation(),
			tabId: 'tab-a',
			type: 'panel-split',
		});
		const source = store.getSnapshot().tabs[0];
		dispatched.length = 0;
		const duplicateId = tabModel.duplicateTab('tab-a');
		const duplicate = store.getSnapshot().tabs[1];
		if (source === undefined || duplicate?.root.kind !== 'panel')
			throw new Error('Expected new single Panel.');
		expect(source.root.kind).toBe('split');
		expect(store.getSnapshot().tabs[0]).toEqual(source);
		expect(store.getActiveTabId()).toBe('tab-a');
		expect(tabModel.getSnapshot().tabs[1]).toMatchObject({
			id: duplicateId,
			title: 'Custom source',
			pinned: true,
			group: null,
		});
		expect(duplicate.root.childPresentations).toEqual([]);
		expect(duplicate.root.primaryPresentation).toMatchObject({
			presentationId: 'duplicate:dynamic:2',
			currentCwd: null,
			launchCwd: null,
		});
		expect(store.getTabTitlePolicies().find((policy) => policy.tabId === duplicateId)).toEqual({
			tabId: duplicateId,
			customTitle: null,
			fallbackTitle: 'Custom source',
		});
		expect(dispatched).toHaveLength(1);
		unsubscribe();
	});

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

	it('advances across dozens of pending managed Tab closes without Host completion', () => {
		const tabCount = 32;
		const tabModel = new TabModel({
			activeTabId: `tab-${tabCount}`,
			tabs: Array.from({ length: tabCount }, (_, index) => ({
				group: null,
				id: `tab-${index + 1}`,
				pinned: false,
				title: `Terminal ${index + 1}`,
			})),
		});
		const dispatched: ManagedCommand[] = [];
		const store = new PanelWorkspaceStore(tabModel, {
			composition: createManagedComposition('rapid-close', dispatched),
		});
		dispatched.length = 0;
		const unsubscribe = store.subscribe(() => undefined);

		for (let tabSequence = tabCount; tabSequence >= 1; tabSequence -= 1) {
			expect(store.requestTabClose([`tab-${tabSequence}`]).outcome).toBe('applied');
			expect(tabModel.getSnapshot().activeTabId).toBe(
				tabSequence === 1 ? 'tab-1' : `tab-${tabSequence - 1}`,
			);
		}
		expect(store.getSnapshot().tabs).toHaveLength(tabCount);
		expect(store.getSnapshot().pendingRemovals).toHaveLength(tabCount);
		expect(
			dispatched
				.filter((command) => command.type === 'terminal-close-requested')
				.map((command) => command.presentationId),
		).toEqual([
			...Array.from(
				{ length: tabCount - 1 },
				(_, index) => `rapid-close:dynamic:${tabCount - index - 1}`,
			),
			'rapid-close:initial',
		]);

		unsubscribe();
	});
});
