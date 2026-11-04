import { describe, expect, it } from 'vitest';
import { workbenchSharedDocumentRevision } from '@orchbee/workbench-client-protocol';

import {
	createSnapshotRuntime,
	fixturePanelId,
	fixtureTabId,
	oneWorkspaceSnapshot,
	publishSnapshot,
	snapshotPanel,
	snapshotSplit,
	snapshotTab,
} from '../../../__tests__/server-workbench-snapshot';
import { hydrateWorkbenchDocuments } from '../workbench-document-runtime';

describe('Workbench document runtime', () => {
	it('adopts a published document and reconciles the current Client View', () => {
		const runtime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [snapshotTab(1), snapshotTab(2)]),
		);
		runtime.getActiveTerminalContext().tabModel.selectTab(fixtureTabId(2));
		expect(runtime.getClientView().terminalContexts[0]?.tabCollection.activeTabId).toBe(
			fixtureTabId(2),
		);

		publishSnapshot(runtime, oneWorkspaceSnapshot(2, [snapshotTab(1)]));

		expect(runtime.getClientView().terminalContexts[0]?.tabCollection.activeTabId).toBe(
			fixtureTabId(1),
		);
		expect(
			runtime.getActiveTerminalContext().panelWorkspaceStore.getSnapshot().tabs[0]?.root,
		).toMatchObject({ primaryPresentation: { lifecycle: { kind: 'running' } } });
	});

	it('preserves live Panel focus history across a publication without serializing it', () => {
		const threePanels = snapshotTab(
			1,
			snapshotSplit(
				1,
				'horizontal',
				snapshotPanel(1),
				snapshotSplit(2, 'horizontal', snapshotPanel(2), snapshotPanel(3)),
			),
		);
		const runtime = createSnapshotRuntime(oneWorkspaceSnapshot(1, [threePanels]));
		const context = runtime.getActiveTerminalContext();
		for (const sequence of [1, 3, 2]) {
			context.panelWorkspaceStore.transition({
				panelId: fixturePanelId(sequence),
				tabId: fixtureTabId(1),
				type: 'panel-focused',
			});
		}
		const clientView = runtime.getClientView();
		const document = runtime.getSharedDocument(workbenchSharedDocumentRevision('11'));
		const focusOrder = [fixturePanelId(1), fixturePanelId(3), fixturePanelId(2)];

		expect(context.panelWorkspaceStore.getSnapshot().tabs[0]?.panelFocusHistory).toEqual(
			focusOrder,
		);
		expect(JSON.stringify(document)).not.toContain('panelFocusHistory');
		expect(
			hydrateWorkbenchDocuments(document, clientView).terminalContexts[0]?.panelWorkspace.tabs[0]
				?.panelFocusHistory,
		).toEqual([fixturePanelId(2)]);

		publishSnapshot(runtime, oneWorkspaceSnapshot(2, [threePanels, snapshotTab(4)]));
		expect(
			runtime.getActiveTerminalContext().panelWorkspaceStore.getSnapshot().tabs[0]
				?.panelFocusHistory,
		).toEqual(focusOrder);
	});
});
