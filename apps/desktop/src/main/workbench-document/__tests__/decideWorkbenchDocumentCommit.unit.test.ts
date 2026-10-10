import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
} from '@orchbee/workbench/document';
import { describe, expect, it } from 'vitest';

import { decideWorkbenchDocumentCommit } from '../decideWorkbenchDocumentCommit';

function document(revision: string): WorkbenchSharedDocument {
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
		revision: workbenchSharedDocumentRevision(revision),
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
}

describe('workbench document store model', () => {
	it('admits only the initial revision for a missing store', () => {
		const initial = document('0');

		expect(decideWorkbenchDocumentCommit({ kind: 'missing' }, null, initial)).toEqual({
			document: initial,
			kind: 'write',
		});
		expect(
			decideWorkbenchDocumentCommit(
				{ kind: 'missing' },
				workbenchSharedDocumentRevision('0'),
				document('1'),
			),
		).toEqual({ actualRevision: null, kind: 'conflict' });
	});

	it('requires an exact current expectation and exact-next candidate', () => {
		const current = document('7');
		const next = document('8');

		expect(
			decideWorkbenchDocumentCommit(
				{ document: current, kind: 'available' },
				current.revision,
				next,
			),
		).toEqual({ document: next, kind: 'write' });
		expect(
			decideWorkbenchDocumentCommit(
				{ document: current, kind: 'available' },
				workbenchSharedDocumentRevision('6'),
				document('7'),
			),
		).toEqual({ actualRevision: current.revision, kind: 'conflict' });
		expect(
			decideWorkbenchDocumentCommit(
				{ document: current, kind: 'available' },
				current.revision,
				document('9'),
			),
		).toEqual({ kind: 'rejected', reason: 'revision-sequence' });
	});

	it('never authorizes a write over unavailable current data', () => {
		expect(
			decideWorkbenchDocumentCommit(
				{ kind: 'unavailable', reason: 'invalid' },
				null,
				document('0'),
			),
		).toEqual({ kind: 'unavailable', reason: 'invalid' });
	});

	it('increments canonical revisions without a JavaScript number ceiling', () => {
		const current = document('999999999999999999999999999999999999');
		const next = document('1000000000000000000000000000000000000');
		expect(
			decideWorkbenchDocumentCommit(
				{ document: current, kind: 'available' },
				current.revision,
				next,
			),
		).toEqual({ document: next, kind: 'write' });
	});
});
