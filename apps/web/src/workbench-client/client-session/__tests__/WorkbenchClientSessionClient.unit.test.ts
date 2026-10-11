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

const bootstrap = WorkbenchClientSessionBootstrapSchema.parse({
	clientSessionId: 'client-1',
	defaultCwd: '/workspace',
	documentState: { kind: 'missing' },
	initialPresentationId: 'presentation-initial',
	initializationDocument: document,
	sequence: '4',
	terminalHostName: 'orchbee-host',
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
	it('releases a subscription returned after synchronous transport retirement', async () => {
		const harness = createTransportHarness();
		harness.subscribe.mockImplementation((_listener, unavailable) => {
			unavailable();
			return harness.unsubscribe;
		});

		const client = new WorkbenchClientSessionClient(harness.transport, bootstrap);
		client.dispose();

		expect(harness.unsubscribe).toHaveBeenCalledOnce();
		expect(harness.close).toHaveBeenCalledOnce();
		await expect(client.propose(null, document)).resolves.toEqual({
			document: null,
			kind: 'rejected',
			reason: 'control-unavailable',
		});
		expect(harness.send).not.toHaveBeenCalled();
	});

	it('closes the owned transport when subscription acquisition fails', () => {
		const harness = createTransportHarness();
		const failure = new Error('Subscription acquisition failed');
		harness.subscribe.mockImplementation(() => {
			throw failure;
		});

		expect(() => new WorkbenchClientSessionClient(harness.transport, bootstrap)).toThrow(failure);
		expect(harness.close).toHaveBeenCalledOnce();
		expect(harness.unsubscribe).not.toHaveBeenCalled();
	});

	it('preserves acquisition and transport cleanup failures without leaking ownership', () => {
		const harness = createTransportHarness();
		const acquisitionFailure = new Error('Subscription acquisition failed');
		const cleanupFailure = new Error('Transport close failed');
		harness.subscribe.mockImplementation(() => {
			throw acquisitionFailure;
		});
		harness.close.mockImplementation(() => {
			throw cleanupFailure;
		});

		let failure: unknown;
		try {
			new WorkbenchClientSessionClient(harness.transport, bootstrap);
		} catch (error) {
			failure = error;
		}

		expect(failure).toBeInstanceOf(AggregateError);
		if (!(failure instanceof AggregateError)) throw failure;
		expect(failure.errors[0]).toBe(acquisitionFailure);
		expect(failure.errors[1]).toBe(cleanupFailure);
		expect(harness.close).toHaveBeenCalledOnce();
		expect(harness.unsubscribe).not.toHaveBeenCalled();
	});

	it.each(['unsubscribe', 'close'] as const)(
		'settles proposals and completes retirement when %s throws',
		async (failingStep) => {
			const harness = createTransportHarness();
			const client = new WorkbenchClientSessionClient(harness.transport, bootstrap, () =>
				WorkbenchClientSessionRequestIdSchema.parse('request-1'),
			);
			const failure = new Error(`${failingStep} failed`);
			harness[failingStep].mockImplementation(() => {
				client.dispose();
				throw failure;
			});
			const settled = vi.fn<(result: unknown) => void>();
			void client.propose(null, document).then(settled);

			let observedFailure: unknown;
			try {
				client.dispose();
			} catch (error) {
				observedFailure = error;
			}
			client.dispose();
			await Promise.resolve();

			expect(observedFailure).toBe(failure);
			expect(harness.unsubscribe).toHaveBeenCalledOnce();
			expect(harness.close).toHaveBeenCalledOnce();
			expect(settled).toHaveBeenCalledExactlyOnceWith({
				document: null,
				kind: 'rejected',
				reason: 'control-unavailable',
			});
		},
	);

	it('preserves both transport cleanup failures after settling pending proposals', async () => {
		const harness = createTransportHarness();
		const client = new WorkbenchClientSessionClient(harness.transport, bootstrap, () =>
			WorkbenchClientSessionRequestIdSchema.parse('request-1'),
		);
		const unsubscribeFailure = new Error('Unsubscribe failed');
		const closeFailure = new Error('Close failed');
		harness.unsubscribe.mockImplementation(() => {
			throw unsubscribeFailure;
		});
		harness.close.mockImplementation(() => {
			throw closeFailure;
		});
		const settled = vi.fn<(result: unknown) => void>();
		void client.propose(null, document).then(settled);

		let failure: unknown;
		try {
			client.dispose();
		} catch (error) {
			failure = error;
		}
		await Promise.resolve();

		expect(failure).toBeInstanceOf(AggregateError);
		expect(failure).toMatchObject({ errors: [unsubscribeFailure, closeFailure] });
		expect(settled).toHaveBeenCalledExactlyOnceWith({
			document: null,
			kind: 'rejected',
			reason: 'control-unavailable',
		});
	});

	it('does not publish a proposal after request identity allocation retires the session', async () => {
		const harness = createTransportHarness();
		const client = new WorkbenchClientSessionClient(harness.transport, bootstrap, () => {
			client.dispose();
			return WorkbenchClientSessionRequestIdSchema.parse('request-1');
		});
		const settled = vi.fn<(result: unknown) => void>();

		void client.propose(null, document).then(settled);
		await Promise.resolve();

		expect(harness.send).not.toHaveBeenCalled();
		expect(settled).toHaveBeenCalledExactlyOnceWith({
			document: null,
			kind: 'rejected',
			reason: 'control-unavailable',
		});
	});
});

function createTransportHarness() {
	let listener: (value: unknown) => void = () => undefined;
	const close = vi.fn<() => void>();
	const send = vi.fn<WorkbenchClientSessionTransport['send']>();
	const unsubscribe = vi.fn<() => void>(() => {
		listener = () => undefined;
	});
	const subscribe = vi.fn<WorkbenchClientSessionTransport['subscribe']>((nextListener) => {
		listener = nextListener;
		return unsubscribe;
	});
	const transport: WorkbenchClientSessionTransport = { close, send, subscribe };
	return {
		close,
		publish: (value: unknown) => listener(value),
		send,
		subscribe,
		transport,
		unsubscribe,
	};
}
