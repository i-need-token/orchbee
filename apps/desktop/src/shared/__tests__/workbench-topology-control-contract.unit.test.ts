import {
	createWorkbenchTopologyDocument,
	workbenchTopologyRevision,
} from '@orchbee/workbench/topology';
import { describe, expect, it } from 'vitest';

import {
	WorkbenchTopologyControlCommandSchema,
	WorkbenchTopologyControlEventSchema,
} from '../workbench-topology-control-contract';

const document = createWorkbenchTopologyDocument({
	projectWorkspace: {
		activeLocation: { kind: 'standalone' },
		expandedProjectIds: [],
		nextProjectSequence: 1,
		nextTerminalContextSequence: 2,
		nextWorkspaceSequence: 1,
		projects: [],
		standaloneTerminalContextId: 'terminal-context-1',
	},
	revision: workbenchTopologyRevision('0'),
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

describe('workbench topology control contract', () => {
	it('admits only complete load and CAS commit commands', () => {
		expect(
			WorkbenchTopologyControlCommandSchema.safeParse({
				requestId: 'topology-request-1',
				type: 'workbench-topology-load-requested',
			}).success,
		).toBe(true);
		expect(
			WorkbenchTopologyControlCommandSchema.safeParse({
				document,
				expectedRevision: null,
				requestId: 'topology-request-2',
				type: 'workbench-topology-commit-requested',
			}).success,
		).toBe(true);
		expect(
			WorkbenchTopologyControlCommandSchema.safeParse({
				filePath: '/private/topology.json',
				requestId: 'topology-request-3',
				type: 'workbench-topology-load-requested',
			}).success,
		).toBe(false);
	});

	it('rejects Host authority and malformed topology in results', () => {
		expect(
			WorkbenchTopologyControlEventSchema.safeParse({
				requestId: 'topology-request-1',
				result: { document, kind: 'available' },
				type: 'workbench-topology-load-completed',
			}).success,
		).toBe(true);
		expect(
			WorkbenchTopologyControlEventSchema.safeParse({
				requestId: 'topology-request-1',
				result: {
					document: { ...document, terminalId: '11'.repeat(16) },
					kind: 'available',
				},
				type: 'workbench-topology-load-completed',
			}).success,
		).toBe(false);
	});
});
