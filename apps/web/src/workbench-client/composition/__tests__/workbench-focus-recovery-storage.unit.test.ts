import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	activeTerminalContext,
	createSnapshotRuntime,
	fixturePanelId,
	fixturePresentationId,
	fixtureTabId,
	oneWorkspaceSnapshot,
	snapshotPanel,
	snapshotPresentation,
	snapshotPublication,
	snapshotSplit,
	snapshotTab,
} from '../../../__tests__/server-workbench-snapshot';
import {
	createDefaultWorkbenchClientView,
	resolveActiveTerminalPresentationId,
} from '../../../workbench';
import {
	createFocusRecoveredWorkbenchClientView,
	readWorkbenchFocusRecovery,
	writeWorkbenchFocusRecovery,
	type WorkbenchFocusRecoveryStorage,
} from '../workbench-focus-recovery-storage';

const EXPECTED_STORAGE_KEY = 'orchbee:web:terminal-focus-recovery:v1';
const splitPresentationId = terminalPresentationId(fixturePresentationId(2));
const otherTabPresentationId = terminalPresentationId(fixturePresentationId(3));
const childPresentationId = terminalPresentationId(fixturePresentationId(4));

/** Tab 1 splits Panels 1 and 2, and Panel 2 holds child Presentation 4; Tab 2 holds Panel 3. */
const snapshot = oneWorkspaceSnapshot(1, [
	snapshotTab(
		1,
		snapshotSplit(
			1,
			'horizontal',
			snapshotPanel(1),
			snapshotPanel(2, snapshotPresentation(2), [snapshotPresentation(4)]),
		),
	),
	snapshotTab(2, snapshotPanel(3)),
]);
const publication = snapshotPublication(snapshot);

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
		const defaultView = createDefaultWorkbenchClientView(publication);
		const recoveredPrimary = createFocusRecoveredWorkbenchClientView(
			publication,
			splitPresentationId,
		);
		const recoveredChild = createFocusRecoveredWorkbenchClientView(
			publication,
			childPresentationId,
		);
		const recoveredOtherTab = createFocusRecoveredWorkbenchClientView(
			publication,
			otherTabPresentationId,
		);

		expect(recoveredPrimary?.terminalContexts[0]?.panelWorkspace.tabs[0]).toMatchObject({
			focusedPanelId: fixturePanelId(2),
			panels: [
				{ activeChildPresentationId: null, panelId: fixturePanelId(1) },
				{ activeChildPresentationId: null, panelId: fixturePanelId(2) },
			],
		});
		expect(recoveredChild?.terminalContexts[0]?.panelWorkspace.tabs[0]).toMatchObject({
			focusedPanelId: fixturePanelId(2),
			panels: [
				{ activeChildPresentationId: null, panelId: fixturePanelId(1) },
				{ activeChildPresentationId: childPresentationId, panelId: fixturePanelId(2) },
			],
		});
		expect(recoveredOtherTab?.terminalContexts[0]?.tabCollection).toMatchObject({
			activeTabId: fixtureTabId(2),
			selectedTabIds: [fixtureTabId(2)],
			selectionAnchorId: fixtureTabId(2),
		});
		expect(recoveredOtherTab?.terminalContexts[0]?.panelWorkspace.tabs[1]?.focusedPanelId).toBe(
			fixturePanelId(3),
		);
		expect(recoveredPrimary?.activeLocation).toEqual(defaultView.activeLocation);
	});

	it('ignores a stale presentation, and a recovered window shows the stored one again', () => {
		expect(
			createFocusRecoveredWorkbenchClientView(
				publication,
				terminalPresentationId(fixturePresentationId(9)),
			),
		).toBeUndefined();
		expect(createFocusRecoveredWorkbenchClientView(publication, null)).toBeUndefined();

		const runtime = createSnapshotRuntime(snapshot, {
			clientView: createFocusRecoveredWorkbenchClientView(publication, childPresentationId),
		});
		expect(resolveActiveTerminalPresentationId(runtime)).toBe(childPresentationId);

		activeTerminalContext(runtime).tabModel.selectTab(fixtureTabId(2));
		expect(resolveActiveTerminalPresentationId(runtime)).toBe(otherTabPresentationId);
	});
});

function createMemoryStorage(): WorkbenchFocusRecoveryStorage {
	const values = new Map<string, string>();
	return {
		getItem: (key) => values.get(key) ?? null,
		removeItem: (key) => values.delete(key),
		setItem: (key, value) => values.set(key, value),
	};
}
