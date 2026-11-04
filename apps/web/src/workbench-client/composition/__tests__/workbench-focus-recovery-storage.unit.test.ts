import { createDefaultWorkbenchClientView, createWorkbenchClientView } from '../../../workbench';
import { describe, expect, it } from 'vitest';
import {
	terminalPresentationId,
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench-client-protocol';

import {
	createFocusRecoveredWorkbenchClientView,
	readWorkbenchFocusRecovery,
	resolveFocusedWorkbenchPresentationId,
	writeWorkbenchFocusRecovery,
	type WorkbenchFocusRecoveryStorage,
} from '../workbench-focus-recovery-storage';

const EXPECTED_STORAGE_KEY = 'orchbee:web:terminal-focus-recovery:v1';
const primaryPresentationId = terminalPresentationId('presentation-primary');
const splitPresentationId = terminalPresentationId('presentation-split');
const childPresentationId = terminalPresentationId('presentation-child');
const otherTabPresentationId = terminalPresentationId('presentation-other-tab');

const document = createWorkbenchSharedDocument({
	projectWorkspace: {
		nextResourceScopeSequence: 1,
		resourceScopes: [],
		workspaces: [],
		nextProjectSequence: 1,
		nextTerminalContextSequence: 2,
		nextWorkspaceSequence: 1,
		projects: [],
		standaloneTerminalContextId: 'terminal-context-1',
	},
	revision: workbenchSharedDocumentRevision('0'),
	terminalContexts: [
		{
			nextTabSequence: 3,
			panelWorkspace: {
				nextPanelSequence: 4,
				nextPanelSlotSequence: 3,
				nextSplitSequence: 2,
				nextTerminalSequence: 5,
				tabs: [
					{
						root: {
							children: [
								{
									id: 'panel-slot-1',
									node: {
										childPresentations: [],
										id: 'panel-1',
										kind: 'panel',
										primaryPresentation: presentation(primaryPresentationId, 'Terminal 1'),
									},
								},
								{
									id: 'panel-slot-2',
									node: {
										childPresentations: [presentation(childPresentationId, 'Agent child')],
										id: 'panel-2',
										kind: 'panel',
										primaryPresentation: presentation(splitPresentationId, 'Terminal 2'),
									},
								},
							],
							id: 'split-1',
							kind: 'split',
							orientation: 'horizontal',
						},
						tabId: 'terminal-context-1-tab-1',
					},
					{
						root: {
							childPresentations: [],
							id: 'panel-3',
							kind: 'panel',
							primaryPresentation: presentation(otherTabPresentationId, 'Terminal 3'),
						},
						tabId: 'terminal-context-1-tab-2',
					},
				],
			},
			tabCollection: {
				groups: [],
				tabs: [
					{
						customTitle: null,
						group: null,
						id: 'terminal-context-1-tab-1',
						pinned: false,
						title: 'Terminal 1',
					},
					{
						customTitle: null,
						group: null,
						id: 'terminal-context-1-tab-2',
						pinned: false,
						title: 'Terminal 3',
					},
				],
			},
			terminalContextId: 'terminal-context-1',
		},
	],
});

describe('Workbench focus recovery storage', () => {
	it('strictly reads only one bounded presentation identity', () => {
		const storage = createMemoryStorage();
		storage.setItem(EXPECTED_STORAGE_KEY, JSON.stringify({ presentationId: splitPresentationId }));
		expect(readWorkbenchFocusRecovery(storage)).toBe(splitPresentationId);

		for (const value of [
			'{',
			JSON.stringify({ presentationId: '' }),
			JSON.stringify({ presentationId: splitPresentationId, terminalId: 'forbidden' }),
		]) {
			storage.setItem(EXPECTED_STORAGE_KEY, value);
			expect(readWorkbenchFocusRecovery(storage)).toBeNull();
		}
		storage.removeItem(EXPECTED_STORAGE_KEY);
		expect(readWorkbenchFocusRecovery(storage)).toBeNull();
		expect(
			readWorkbenchFocusRecovery({
				...storage,
				getItem: () => {
					throw new Error('blocked');
				},
			}),
		).toBeNull();
	});

	it('writes, removes, and isolates storage failures from the current view', () => {
		const storage = createMemoryStorage();
		expect(writeWorkbenchFocusRecovery(storage, splitPresentationId)).toBe(true);
		expect(storage.getItem(EXPECTED_STORAGE_KEY)).toBe(
			JSON.stringify({ presentationId: splitPresentationId }),
		);
		expect(writeWorkbenchFocusRecovery(storage, null)).toBe(true);
		expect(storage.getItem(EXPECTED_STORAGE_KEY)).toBeNull();
		expect(
			writeWorkbenchFocusRecovery(
				{
					...storage,
					setItem: () => {
						throw new Error('quota');
					},
				},
				splitPresentationId,
			),
		).toBe(false);
		expect(
			writeWorkbenchFocusRecovery(
				{
					...storage,
					removeItem: () => {
						throw new Error('blocked');
					},
				},
				null,
			),
		).toBe(false);
	});

	it('restores the exact Workspace, Tab, Panel and child for a surviving presentation', () => {
		const defaultView = createDefaultWorkbenchClientView(document);
		const recoveredPrimary = createFocusRecoveredWorkbenchClientView(document, splitPresentationId);
		const recoveredChild = createFocusRecoveredWorkbenchClientView(document, childPresentationId);
		const recoveredOtherTab = createFocusRecoveredWorkbenchClientView(
			document,
			otherTabPresentationId,
		);

		expect(recoveredPrimary?.terminalContexts[0]?.panelWorkspace.tabs[0]).toMatchObject({
			focusedPanelId: 'panel-2',
			panels: [
				{ activeChildPresentationId: null, panelId: 'panel-1' },
				{ activeChildPresentationId: null, panelId: 'panel-2' },
			],
		});
		expect(recoveredChild?.terminalContexts[0]?.panelWorkspace.tabs[0]).toMatchObject({
			focusedPanelId: 'panel-2',
			panels: [
				{ activeChildPresentationId: null, panelId: 'panel-1' },
				{ activeChildPresentationId: childPresentationId, panelId: 'panel-2' },
			],
		});
		expect(recoveredOtherTab?.terminalContexts[0]?.tabCollection).toMatchObject({
			activeTabId: 'terminal-context-1-tab-2',
			selectedTabIds: ['terminal-context-1-tab-2'],
			selectionAnchorId: 'terminal-context-1-tab-2',
		});
		expect(recoveredOtherTab?.terminalContexts[0]?.panelWorkspace.tabs[1]?.focusedPanelId).toBe(
			'panel-3',
		);
		expect(recoveredPrimary?.activeLocation).toEqual(defaultView.activeLocation);
	});

	it('ignores a stale presentation and resolves the currently displayed one', () => {
		expect(
			createFocusRecoveredWorkbenchClientView(
				document,
				terminalPresentationId('presentation-missing'),
			),
		).toBeUndefined();
		expect(createFocusRecoveredWorkbenchClientView(document, null)).toBeUndefined();

		const recoveredChild = createFocusRecoveredWorkbenchClientView(document, childPresentationId);
		if (recoveredChild === undefined) throw new Error('Expected a recovered Client View.');
		expect(resolveFocusedWorkbenchPresentationId(document, recoveredChild)).toBe(
			childPresentationId,
		);

		const context = recoveredChild.terminalContexts[0];
		if (context === undefined) throw new Error('Expected one terminal context.');
		const otherTabView = createWorkbenchClientView({
			...recoveredChild,
			terminalContexts: [
				{
					...context,
					tabCollection: {
						...context.tabCollection,
						activeTabId: 'terminal-context-1-tab-2',
						selectedTabIds: ['terminal-context-1-tab-2'],
						selectionAnchorId: 'terminal-context-1-tab-2',
					},
				},
			],
		});
		expect(resolveFocusedWorkbenchPresentationId(document, otherTabView)).toBe(
			otherTabPresentationId,
		);
	});
});

function presentation(presentationId: ReturnType<typeof terminalPresentationId>, title: string) {
	return {
		intent: { kind: 'retain' as const },
		presentationId,
		title,
	};
}

function createMemoryStorage(): WorkbenchFocusRecoveryStorage {
	const values = new Map<string, string>();
	return {
		getItem: (key) => values.get(key) ?? null,
		removeItem: (key) => values.delete(key),
		setItem: (key, value) => values.set(key, value),
	};
}
