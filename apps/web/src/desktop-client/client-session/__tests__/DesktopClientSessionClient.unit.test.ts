import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import { describe, expect, it, vi } from 'vitest';

import {
	WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
	WorkbenchClientSessionBootstrapSchema,
	WorkbenchClientSessionRequestIdSchema,
} from '@orchbee/workbench-client-protocol';
import { DesktopClientSessionClient } from '../DesktopClientSessionClient';

class TestMessagePort extends EventTarget {
	readonly close = vi.fn<() => void>();
	readonly postMessage = vi.fn<(message: unknown) => void>();
	readonly start = vi.fn<() => void>();
}

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

describe('DesktopClientSessionClient', () => {
	it('correlates a proposal while independently publishing strict sequenced events', async () => {
		const port = new TestMessagePort();
		const client = new DesktopClientSessionClient(port as unknown as MessagePort, bootstrap, () =>
			WorkbenchClientSessionRequestIdSchema.parse('request-1'),
		);
		const publications: string[] = [];
		client.subscribe((publication) => publications.push(publication.type));

		const proposal = client.propose(null, document);
		expect(port.postMessage).toHaveBeenCalledWith({
			document,
			expectedRevision: null,
			requestId: 'request-1',
			type: 'shared-document-proposed',
		});
		port.dispatchEvent(
			new MessageEvent('message', {
				data: { document, sequence: '5', type: 'shared-document-published' },
			}),
		);
		port.dispatchEvent(
			new MessageEvent('message', {
				data: {
					requestId: 'request-1',
					result: { document, kind: 'committed' },
					type: 'shared-document-proposal-completed',
				},
			}),
		);

		expect(await proposal).toEqual({ document, kind: 'committed' });
		expect(publications).toEqual(['shared-document-published']);
		expect(port.start).toHaveBeenCalledOnce();
	});

	it('retires pending proposals on a sequence gap or unknown completion', async () => {
		const port = new TestMessagePort();
		const client = new DesktopClientSessionClient(port as unknown as MessagePort, bootstrap, () =>
			WorkbenchClientSessionRequestIdSchema.parse('request-1'),
		);
		const proposal = client.propose(null, document);

		port.dispatchEvent(
			new MessageEvent('message', {
				data: { document, sequence: '6', type: 'shared-document-published' },
			}),
		);

		expect(await proposal).toEqual({
			document: null,
			kind: 'rejected',
			reason: 'control-unavailable',
		});
		expect(port.close).toHaveBeenCalledOnce();
	});
});
