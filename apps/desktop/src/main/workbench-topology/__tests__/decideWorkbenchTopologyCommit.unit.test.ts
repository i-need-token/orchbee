import {
	createWorkbenchTopologyDocument,
	workbenchTopologyRevision,
	type WorkbenchTopologyDocument,
} from '@orchbee/workbench/topology';
import { describe, expect, it } from 'vitest';

import { decideWorkbenchTopologyCommit } from '../decideWorkbenchTopologyCommit';

function topology(revision: string): WorkbenchTopologyDocument {
	return createWorkbenchTopologyDocument({
		projectWorkspace: {
			activeLocation: { kind: 'standalone' },
			expandedProjectIds: [],
			nextProjectSequence: 1,
			nextTerminalContextSequence: 2,
			nextWorkspaceSequence: 1,
			projects: [],
			standaloneTerminalContextId: 'terminal-context-1',
		},
		revision: workbenchTopologyRevision(revision),
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
					activeTabId: null,
					groups: [],
					mostRecentGroupId: null,
					tabs: [],
				},
				terminalContextId: 'terminal-context-1',
			},
		],
	});
}

describe('workbench topology store model', () => {
	it('admits only the initial revision for a missing store', () => {
		const initial = topology('0');

		expect(decideWorkbenchTopologyCommit({ kind: 'missing' }, null, initial)).toEqual({
			document: initial,
			kind: 'write',
		});
		expect(
			decideWorkbenchTopologyCommit(
				{ kind: 'missing' },
				workbenchTopologyRevision('0'),
				topology('1'),
			),
		).toEqual({ actualRevision: null, kind: 'conflict' });
	});

	it('requires an exact current expectation and exact-next candidate', () => {
		const current = topology('7');
		const next = topology('8');

		expect(
			decideWorkbenchTopologyCommit(
				{ document: current, kind: 'available' },
				current.revision,
				next,
			),
		).toEqual({ document: next, kind: 'write' });
		expect(
			decideWorkbenchTopologyCommit(
				{ document: current, kind: 'available' },
				workbenchTopologyRevision('6'),
				topology('7'),
			),
		).toEqual({ actualRevision: current.revision, kind: 'conflict' });
		expect(
			decideWorkbenchTopologyCommit(
				{ document: current, kind: 'available' },
				current.revision,
				topology('9'),
			),
		).toEqual({ kind: 'rejected', reason: 'revision-sequence' });
	});

	it('never authorizes a write over unavailable current data', () => {
		expect(
			decideWorkbenchTopologyCommit(
				{ kind: 'unavailable', reason: 'invalid' },
				null,
				topology('0'),
			),
		).toEqual({ kind: 'unavailable', reason: 'invalid' });
	});

	it('increments canonical revisions without a JavaScript number ceiling', () => {
		const current = topology('999999999999999999999999999999999999');
		const next = topology('1000000000000000000000000000000000000');
		expect(
			decideWorkbenchTopologyCommit(
				{ document: current, kind: 'available' },
				current.revision,
				next,
			),
		).toEqual({ document: next, kind: 'write' });
	});
});
