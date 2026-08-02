import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import { describe, expect, it } from 'vitest';

import {
	WorkbenchDocumentControlCommandSchema,
	WorkbenchDocumentControlEventSchema,
} from '../workbench-document-control-contract';

const document = createWorkbenchSharedDocument({
	projectWorkspace: {
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
			tabCollection: {
				groups: [],
				tabs: [],
			},
			terminalContextId: 'terminal-context-1',
		},
	],
});

describe('workbench document control contract', () => {
	it('admits only complete load and CAS commit commands', () => {
		expect(
			WorkbenchDocumentControlCommandSchema.safeParse({
				requestId: 'document-request-1',
				type: 'workbench-document-load-requested',
			}).success,
		).toBe(true);
		expect(
			WorkbenchDocumentControlCommandSchema.safeParse({
				document,
				expectedRevision: null,
				requestId: 'document-request-2',
				type: 'workbench-document-commit-requested',
			}).success,
		).toBe(true);
		expect(
			WorkbenchDocumentControlCommandSchema.safeParse({
				filePath: '/private/document.json',
				requestId: 'document-request-3',
				type: 'workbench-document-load-requested',
			}).success,
		).toBe(false);
	});

	it('rejects Host authority and malformed document in results', () => {
		expect(
			WorkbenchDocumentControlEventSchema.safeParse({
				requestId: 'document-request-1',
				result: { document, kind: 'available' },
				type: 'workbench-document-load-completed',
			}).success,
		).toBe(true);
		expect(
			WorkbenchDocumentControlEventSchema.safeParse({
				requestId: 'document-request-1',
				result: {
					document: { ...document, terminalId: '11'.repeat(16) },
					kind: 'available',
				},
				type: 'workbench-document-load-completed',
			}).success,
		).toBe(false);
	});
});
