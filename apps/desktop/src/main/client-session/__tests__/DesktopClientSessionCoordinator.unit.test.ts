import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
} from '@orchbee/workbench/document';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
} from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import type { WorkbenchDocumentStore } from '../../workbench-document';
import {
	DesktopClientSessionCoordinator,
	type DesktopClientSessionPublication,
	type DesktopClientSessionPublisher,
} from '../DesktopClientSessionCoordinator';

describe('DesktopClientSessionCoordinator', () => {
	it('loads one canonical document and gives every client one atomic bootstrap', async () => {
		const current = document('7', 'Canonical');
		const store = storeWith({ document: current, kind: 'available' });
		const reconciled: string[] = [];
		const coordinator = coordinatorWith(store, {
			reconcileDocument: async (candidate) => {
				reconciled.push(candidate.revision);
			},
		});

		expect(await coordinator.start()).toEqual({ kind: 'ready' });
		const first = register(coordinator, publisher());
		const second = register(coordinator, publisher());

		expect(store.load).toHaveBeenCalledOnce();
		expect(reconciled).toEqual(['7']);
		expect(first.bootstrap).toMatchObject({
			document: current,
			kind: 'available',
			sequence: '0',
			terminalHostName: 'orchbee-host',
		});
		expect(second.bootstrap).toMatchObject({
			document: current,
			kind: 'available',
			sequence: '0',
			terminalHostName: 'orchbee-host',
		});
		expect(first.clientSessionId).not.toBe(second.clientSessionId);
	});

	it('commits the first proposal, publishes before its effect, and converges an equivalent race', async () => {
		const store = storeWith({ kind: 'missing' });
		store.commit.mockResolvedValue({
			kind: 'committed',
			revision: workbenchSharedDocumentRevision('0'),
		});
		const trace: string[] = [];
		const terminalEvent = {
			exitCode: 0,
			presentationId: terminalPresentationId('presentation-1'),
			signal: null,
			type: 'terminal-exited',
		} as const;
		const coordinator = coordinatorWith(store, {
			reconcileDocument: async (candidate, publish) => {
				trace.push(`reconcile:${candidate.revision}`);
				publish(terminalEvent);
			},
		});
		await coordinator.start();
		const firstPublications: DesktopClientSessionPublication[] = [];
		const secondPublications: DesktopClientSessionPublication[] = [];
		const first = register(coordinator, publisher(firstPublications));
		const second = register(coordinator, publisher(secondPublications));
		const candidate = document('0', 'Winner');

		const firstResult = first.propose('request-first', null, candidate);
		const secondResult = second.propose('request-second', null, candidate);

		expect(await firstResult).toEqual({ document: candidate, kind: 'committed' });
		expect(await secondResult).toEqual({ document: candidate, kind: 'converged' });
		expect(store.commit).toHaveBeenCalledOnce();
		expect(firstPublications.map((publication) => publication.type)).toEqual([
			'shared-document-published',
			'terminal-presentation-event-published',
		]);
		expect(secondPublications).toEqual(firstPublications);
		expect(firstPublications.map((publication) => publication.sequence)).toEqual(['1', '2']);
		expect(trace).toEqual(['reconcile:0']);
	});

	it('does not replay a terminal event live after it was included in bootstrap', async () => {
		const terminalEvent = {
			exitCode: 0,
			presentationId: terminalPresentationId('presentation-bootstrapped-exit'),
			signal: null,
			type: 'terminal-exited',
		} as const;
		const coordinator = coordinatorWith(storeWith({ kind: 'missing' }), {
			getTerminalEvents: () => [terminalEvent],
		});
		await coordinator.start();
		const publications: DesktopClientSessionPublication[] = [];
		const session = register(coordinator, publisher(publications));

		expect(session.bootstrap.terminalEvents).toEqual([terminalEvent]);
		expect(coordinator.publishTerminalEvent(terminalEvent)).toBe(true);
		expect(publications).toEqual([]);
		const laterEvent = { ...terminalEvent, presentationId: terminalPresentationId('later-exit') };
		expect(coordinator.publishTerminalEvent(laterEvent)).toBe(true);
		expect(publications).toMatchObject([
			{ event: laterEvent, sequence: '1', type: 'terminal-presentation-event-published' },
		]);
	});

	it('retries a partially published exit with its original sequence and skips successes', async () => {
		const terminalEvent = {
			exitCode: 0,
			presentationId: terminalPresentationId('partial-publication'),
			signal: null,
			type: 'terminal-exited',
		} as const;
		const coordinator = coordinatorWith(storeWith({ kind: 'missing' }));
		await coordinator.start();
		const firstPublications: DesktopClientSessionPublication[] = [];
		let failSecond = true;
		const first = register(coordinator, publisher(firstPublications));
		const secondPublications: DesktopClientSessionPublication[] = [];
		const second = register(
			coordinator,
			publisher(secondPublications, false, () => {
				if (failSecond) {
					failSecond = false;
					return true;
				}
				return false;
			}),
		);

		expect(coordinator.publishTerminalEvent(terminalEvent)).toBe(false);
		expect(firstPublications).toHaveLength(1);
		expect(secondPublications).toHaveLength(0);
		await second.dispose();
		const replacementPublications: DesktopClientSessionPublication[] = [];
		register(coordinator, publisher(replacementPublications));

		expect(coordinator.publishTerminalEvent(terminalEvent)).toBe(true);
		expect(firstPublications).toHaveLength(1);
		expect(replacementPublications).toHaveLength(1);
		expect(replacementPublications[0]?.sequence).toBe(firstPublications[0]?.sequence);
		expect(second.clientSessionId).not.toBe(first.clientSessionId);
	});

	it('returns canonical state without a write for a divergent stale proposal', async () => {
		const canonical = document('4', 'Canonical');
		const store = storeWith({ document: canonical, kind: 'available' });
		const coordinator = coordinatorWith(store);
		await coordinator.start();
		const client = register(coordinator, publisher());

		expect(
			await client.propose(
				'request-stale',
				workbenchSharedDocumentRevision('3'),
				document('4', 'Divergent'),
			),
		).toEqual({ document: canonical, kind: 'stale' });
		expect(store.commit).not.toHaveBeenCalled();
	});

	it('finishes an accepted proposal after its client disconnects and discards a queued one', async () => {
		let finishCommit:
			| ((result: Awaited<ReturnType<WorkbenchDocumentStore['commit']>>) => void)
			| undefined;
		const store = storeWith({ kind: 'missing' });
		store.commit.mockImplementation(
			() =>
				new Promise((resolve) => {
					finishCommit = resolve;
				}),
		);
		const coordinator = coordinatorWith(store);
		await coordinator.start();
		const firstPublisher = publisher();
		const remainingPublications: DesktopClientSessionPublication[] = [];
		const first = register(coordinator, firstPublisher);
		const second = register(coordinator, publisher(remainingPublications));
		const accepted = first.propose('request-accepted', null, document('0', 'Accepted'));
		await vi.waitFor(() => expect(store.commit).toHaveBeenCalledOnce());
		const queued = second.propose('request-queued', null, document('0', 'Queued'));

		await first.dispose();
		await second.dispose();
		finishCommit?.({ kind: 'committed', revision: workbenchSharedDocumentRevision('0') });

		expect(await accepted).toMatchObject({ kind: 'committed' });
		expect(await queued).toMatchObject({ kind: 'rejected', reason: 'control-unavailable' });
		expect(firstPublisher.disposeAttachments).toHaveBeenCalledOnce();
		expect(remainingPublications).toEqual([]);
		expect(store.commit).toHaveBeenCalledOnce();
	});

	it('does not let a later commit eclipse an unpublished terminal effect', async () => {
		const finishReconciliations: Array<() => void> = [];
		const store = storeWith({ kind: 'missing' });
		store.commit.mockImplementation(async (_expectedRevision, candidate) => ({
			kind: 'committed',
			revision: candidate.revision,
		}));
		const coordinator = coordinatorWith(store, {
			reconcileDocument: () =>
				new Promise((resolve) => {
					finishReconciliations.push(resolve);
				}),
		});
		await coordinator.start();
		const first = register(coordinator, publisher());
		const second = register(coordinator, publisher());
		const initial = document('0', 'Initial');
		const next = document('1', 'Next');
		const firstResult = first.propose('request-initial', null, initial);
		await vi.waitFor(() => expect(store.commit).toHaveBeenCalledOnce());
		const secondResult = second.propose('request-next', initial.revision, next);
		await Promise.resolve();

		expect(store.commit).toHaveBeenCalledOnce();
		finishReconciliations[0]?.();
		expect(await firstResult).toMatchObject({ kind: 'committed' });
		await vi.waitFor(() => expect(store.commit).toHaveBeenCalledTimes(2));
		await vi.waitFor(() => expect(finishReconciliations).toHaveLength(2));
		finishReconciliations[1]?.();
		expect(await secondResult).toMatchObject({ kind: 'committed' });
	});

	it('retires only a client that reuses a request identity with different content', async () => {
		let finishCommit:
			| ((result: Awaited<ReturnType<WorkbenchDocumentStore['commit']>>) => void)
			| undefined;
		const store = storeWith({ kind: 'missing' });
		store.commit.mockImplementation(
			() =>
				new Promise((resolve) => {
					finishCommit = resolve;
				}),
		);
		const coordinator = coordinatorWith(store);
		await coordinator.start();
		const offenderPublisher = publisher();
		const offender = register(coordinator, offenderPublisher);
		coordinator.registerSession(publisher());
		const accepted = offender.propose('same-request', null, document('0', 'First'));
		await vi.waitFor(() => expect(store.commit).toHaveBeenCalledOnce());

		expect(await offender.propose('same-request', null, document('0', 'Different'))).toMatchObject({
			kind: 'rejected',
			reason: 'control-unavailable',
		});
		expect(coordinator.sessionCount).toBe(1);
		expect(offenderPublisher.disposeAttachments).toHaveBeenCalledOnce();

		finishCommit?.({ kind: 'committed', revision: workbenchSharedDocumentRevision('0') });
		expect(await accepted).toMatchObject({ kind: 'committed' });
	});

	it('isolates a publication failure and freezes later mutations after storage failure', async () => {
		const store = storeWith({ kind: 'missing' });
		store.commit.mockResolvedValue({ kind: 'unavailable', reason: 'write-failed' });
		const coordinator = coordinatorWith(store);
		await coordinator.start();
		const broken = publisher([], true);
		coordinator.registerSession(broken);
		const healthyPublications: DesktopClientSessionPublication[] = [];
		const healthy = register(coordinator, publisher(healthyPublications));

		expect(await healthy.propose('request-fails', null, document('0', 'Failure'))).toEqual({
			document: null,
			kind: 'unavailable',
			reason: 'write-failed',
		});
		expect(coordinator.sessionCount).toBe(1);
		expect(broken.disposeAttachments).toHaveBeenCalledOnce();
		expect(healthyPublications).toMatchObject([
			{ reason: 'write-failed', sequence: '1', type: 'client-session-coordinator-unavailable' },
		]);
		expect(
			await healthy.propose('request-after-failure', null, document('0', 'Later')),
		).toMatchObject({ kind: 'rejected', reason: 'control-unavailable' });
		expect(store.commit).toHaveBeenCalledOnce();
	});

	it('lets a fresh session resume only the same quiescing mode after the exit owner disconnects', async () => {
		const store = storeWith({ document: document('3', 'Canonical'), kind: 'available' });
		const coordinator = coordinatorWith(store);
		await coordinator.start();
		const first = register(coordinator, publisher());
		const replacement = register(coordinator, publisher());

		expect(coordinator.beginQuiescing(first.clientSessionId, 'stop')).toBe(true);
		expect(coordinator.beginQuiescing(replacement.clientSessionId, 'stop')).toBe(false);
		await first.dispose();
		expect(coordinator.beginQuiescing(replacement.clientSessionId, 'preserve')).toBe(false);
		expect(coordinator.beginQuiescing(replacement.clientSessionId, 'stop')).toBe(true);
	});

	it('retains a failed attachment disposer for a later retirement retry', async () => {
		const coordinator = coordinatorWith(storeWith({ kind: 'missing' }));
		await coordinator.start();
		const failingPublisher = publisher();
		failingPublisher.disposeAttachments.mockRejectedValueOnce(new Error('attachment failure'));
		const session = register(coordinator, failingPublisher);

		await expect(session.dispose()).rejects.toThrow('attachment failure');
		expect(coordinator.sessionCount).toBe(0);
		await expect(coordinator.dispose()).resolves.toBeUndefined();
		expect(failingPublisher.disposeAttachments).toHaveBeenCalledTimes(2);
	});
});

function coordinatorWith(
	store: WorkbenchDocumentStore,
	overrides: Partial<{
		readonly reconcileDocument: DesktopClientSessionCoordinatorConstructor['reconcileDocument'];
		readonly getTerminalEvents: DesktopClientSessionCoordinatorConstructor['getTerminalEvents'];
	}> = {},
): DesktopClientSessionCoordinator {
	let clientSequence = 0;
	return new DesktopClientSessionCoordinator({
		createClientSessionId: () => `client-${++clientSequence}`,
		defaultCwd: '/workspace',
		getTerminalEvents: overrides.getTerminalEvents ?? (() => []),
		initialPresentationId: terminalPresentationId('presentation-initial'),
		initializationDocument: document('0', 'Initial'),
		reconcileDocument: overrides.reconcileDocument ?? (async () => undefined),
		store,
		terminalHostName: 'orchbee-host',
		validateDocument: () => true,
	});
}

type DesktopClientSessionCoordinatorConstructor = ConstructorParameters<
	typeof DesktopClientSessionCoordinator
>[0];

function register(
	coordinator: DesktopClientSessionCoordinator,
	sessionPublisher: DesktopClientSessionPublisher,
) {
	const registration = coordinator.registerSession(sessionPublisher);
	if (registration.kind !== 'ready') throw new Error('Expected a ready client session.');
	return registration.handle;
}

function publisher(
	publications: DesktopClientSessionPublication[] = [],
	throwOnPublish = false,
	onPublish: (() => boolean) | undefined = undefined,
): DesktopClientSessionPublisher & {
	readonly disposeAttachments: ReturnType<typeof vi.fn<() => Promise<void>>>;
} {
	return {
		disposeAttachments: vi.fn<() => Promise<void>>().mockResolvedValue(undefined),
		publish: (publication) => {
			if (throwOnPublish || onPublish?.() === true) throw new Error('publication failed');
			publications.push(publication);
		},
	};
}

function storeWith(
	loaded: Awaited<ReturnType<WorkbenchDocumentStore['load']>>,
): WorkbenchDocumentStore & {
	readonly commit: ReturnType<typeof vi.fn<WorkbenchDocumentStore['commit']>>;
	readonly load: ReturnType<typeof vi.fn<WorkbenchDocumentStore['load']>>;
} {
	return {
		commit: vi.fn<WorkbenchDocumentStore['commit']>(),
		load: vi.fn<WorkbenchDocumentStore['load']>().mockResolvedValue(loaded),
	};
}

function document(revision: string, title: string): WorkbenchSharedDocument {
	return createWorkbenchSharedDocument({
		projectWorkspace: {
			nextProjectSequence: 1,
			nextTerminalContextSequence: 2,
			nextWorkspaceSequence: 1,
			projects: [],
			standaloneTerminalContextId: 'terminal-context-1',
		},
		revision: workbenchSharedDocumentRevision(revision),
		terminalContexts: [
			{
				nextTabSequence: 2,
				panelWorkspace: {
					nextPanelSequence: 2,
					nextPanelSlotSequence: 1,
					nextSplitSequence: 1,
					nextTerminalSequence: 2,
					pendingRemovals: [],
					tabs: [
						{
							root: {
								childPresentations: [],
								id: 'panel-1',
								kind: 'panel',
								primaryPresentation: {
									intent: {
										kind: 'provision',
										operationId: terminalPresentationOperationId('operation-1'),
									},
									launchCwd: '/workspace',
									presentationId: terminalPresentationId('presentation-1'),
									title,
								},
							},
							tabId: 'tab-1',
						},
					],
				},
				tabCollection: {
					groups: [],
					tabs: [{ customTitle: null, group: null, id: 'tab-1', pinned: false, title }],
				},
				terminalContextId: 'terminal-context-1',
			},
		],
	});
}
