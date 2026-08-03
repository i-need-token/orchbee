import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import { describe, expect, it } from 'vitest';

import {
	WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
	WorkbenchClientSessionBootstrapSchema,
	WorkbenchClientSessionCommandSchema,
	WorkbenchClientSessionEventSchema,
} from '../workbench-client-session-contract';

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
			tabCollection: { groups: [], tabs: [] },
			terminalContextId: 'terminal-context-1',
		},
	],
});

describe('Workbench client session contract', () => {
	it('carries one atomic document and terminal-observation bootstrap without count limits', () => {
		const terminalEvents = Array.from({ length: 300 }, (_, index) => ({
			exitCode: 0,
			presentationId: `presentation-${index + 1}`,
			signal: null,
			type: 'terminal-exited' as const,
		}));
		const bootstrap = WorkbenchClientSessionBootstrapSchema.parse({
			clientSessionId: 'client-session-1',
			defaultCwd: '/workspace',
			documentState: { document, kind: 'available' },
			initialPresentationId: 'presentation-initial',
			initializationDocument: document,
			sequence: '999999999999999999999999',
			terminalEvents,
			type: WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
		});

		expect(bootstrap.terminalEvents).toHaveLength(300);
		expect(bootstrap.documentState).toMatchObject({ kind: 'available' });
	});

	it('accepts only complete strict shared-document proposals and correlated results', () => {
		expect(
			WorkbenchClientSessionCommandSchema.parse({
				document,
				expectedRevision: null,
				requestId: 'request-1',
				type: 'shared-document-proposed',
			}),
		).toMatchObject({ requestId: 'request-1' });
		expect(
			WorkbenchClientSessionCommandSchema.safeParse({
				patch: [{ op: 'replace', path: '/revision', value: '1' }],
				requestId: 'request-1',
				type: 'shared-document-proposed',
			}).success,
		).toBe(false);
		expect(
			WorkbenchClientSessionEventSchema.parse({
				requestId: 'request-1',
				result: { document, kind: 'stale' },
				type: 'shared-document-proposal-completed',
			}),
		).toMatchObject({ result: { kind: 'stale' } });
	});

	it('requires exact publication sequences', () => {
		expect(
			WorkbenchClientSessionEventSchema.safeParse({
				document,
				sequence: '01',
				type: 'shared-document-published',
			}).success,
		).toBe(false);
		expect(
			WorkbenchClientSessionBootstrapSchema.parse({
				clientSessionId: 'client-session-1',
				defaultCwd: '/workspace',
				documentState: { kind: 'missing' },
				initialPresentationId: 'presentation-initial',
				initializationDocument: document,
				sequence: '0',
				terminalEvents: [],
				type: WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
			}),
		).toMatchObject({ sequence: '0' });
	});
});
