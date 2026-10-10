import { describe, expect, it } from 'vitest';

import {
	WorkbenchClientViewSchema,
	createDefaultWorkbenchClientView,
	createWorkbenchClientView,
	createWorkbenchSharedDocument,
	parseWorkbenchClientView,
	reconcileWorkbenchClientView,
	workbenchSharedDocumentRevision,
} from '..';

function emptySharedDocument() {
	return createWorkbenchSharedDocument({
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
				nextTabSequence: 1,
				panelWorkspace: {
					nextPanelSequence: 1,
					nextPanelSlotSequence: 1,
					nextSplitSequence: 1,
					nextTerminalSequence: 1,
					pendingRemovals: [],
					tabs: [],
				},
				tabCollection: { groups: [], tabs: [] },
				terminalContextId: 'terminal-context-1',
			},
		],
	});
}

describe('Workbench Client View', () => {
	it('creates and parses a deterministic disposable default', () => {
		const view = createDefaultWorkbenchClientView(emptySharedDocument());

		expect(parseWorkbenchClientView(JSON.parse(JSON.stringify(view)))).toEqual(view);
		expect(view).toEqual({
			activeLocation: { kind: 'standalone' },
			expandedProjectIds: [],
			terminalContexts: [
				{
					panelWorkspace: { tabs: [] },
					tabCollection: {
						activeTabId: null,
						collapsedGroupIds: [],
						mostRecentGroupId: null,
						selectedTabIds: [],
						selectionAnchorId: null,
					},
					terminalContextId: 'terminal-context-1',
				},
			],
			version: 2,
		});
	});

	it('drops a well-formed stale context and rejects unknown fields', () => {
		const document = emptySharedDocument();
		const stale = createWorkbenchClientView({
			activeLocation: { kind: 'standalone' },
			expandedProjectIds: ['missing-project'],
			terminalContexts: [
				{
					panelWorkspace: { tabs: [] },
					tabCollection: {
						activeTabId: null,
						collapsedGroupIds: [],
						mostRecentGroupId: null,
						selectedTabIds: [],
						selectionAnchorId: null,
					},
					terminalContextId: 'missing-context',
				},
			],
		});

		expect(reconcileWorkbenchClientView(document, stale)).toEqual(
			createDefaultWorkbenchClientView(document),
		);
		expect(
			WorkbenchClientViewSchema.safeParse({ ...stale, attachmentId: 'forbidden' }).success,
		).toBe(false);
	});
});
