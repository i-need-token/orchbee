import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import {
	WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
	WorkbenchClientSessionBootstrapSchema,
	WorkbenchClientSessionRequestIdSchema,
} from '@orchbee/workbench-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import {
	WorkbenchClientSessionClient,
	type WorkbenchClientSessionTransport,
} from '../WorkbenchClientSessionClient';

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

const bootstrap = WorkbenchClientSessionBootstrapSchema.parse({
	clientSessionId: 'client-1',
	defaultCwd: '/workspace',
	documentState: { kind: 'missing' },
	initialPresentationId: 'presentation-initial',
	initializationDocument: document,
	sequence: '4',
	terminalEvents: [],
	type: WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
});

describe('WorkbenchClientSessionClient', () => {
	it('correlates proposals and applies transport-neutral publication ordering', async () => {
		const harness = createTransportHarness();
		const client = new WorkbenchClientSessionClient(harness.transport, bootstrap, () =>
			WorkbenchClientSessionRequestIdSchema.parse('request-1'),
		);
		const publications: string[] = [];
		client.subscribe((publication) => publications.push(publication.type));

		const proposal = client.propose(null, document);
		expect(harness.send).toHaveBeenCalledWith({
			document,
			expectedRevision: null,
			requestId: 'request-1',
			type: 'shared-document-proposed',
		});
		harness.publish({ document, sequence: '5', type: 'shared-document-published' });
		harness.publish({
			requestId: 'request-1',
			result: { document, kind: 'committed' },
			type: 'shared-document-proposal-completed',
		});

		expect(await proposal).toEqual({ document, kind: 'committed' });
		expect(publications).toEqual(['shared-document-published']);
	});

	it('retires only its own transport when publication order has a gap', async () => {
		const harness = createTransportHarness();
		const client = new WorkbenchClientSessionClient(harness.transport, bootstrap, () =>
			WorkbenchClientSessionRequestIdSchema.parse('request-1'),
		);
		const proposal = client.propose(null, document);

		harness.publish({ document, sequence: '6', type: 'shared-document-published' });

		expect(await proposal).toEqual({
			document: null,
			kind: 'rejected',
			reason: 'control-unavailable',
		});
		expect(harness.close).toHaveBeenCalledOnce();
	});
});

function createTransportHarness() {
	let listener: (value: unknown) => void = () => undefined;
	const close = vi.fn<() => void>();
	const send = vi.fn<WorkbenchClientSessionTransport['send']>();
	const transport: WorkbenchClientSessionTransport = {
		close,
		send,
		subscribe: (nextListener) => {
			listener = nextListener;
			return () => {
				listener = () => undefined;
			};
		},
	};
	return {
		close,
		publish: (value: unknown) => listener(value),
		send,
		transport,
	};
}
