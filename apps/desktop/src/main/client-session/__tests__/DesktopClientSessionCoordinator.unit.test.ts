import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
	type WorkbenchSharedPresentationIntent,
} from '@orchbee/workbench/document';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
} from '@orchbee/workbench/terminal-presentation';
import { WorkbenchTerminalMetadataSchema } from '@orchbee/workbench-client-protocol';
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

	it('serializes an owned canonical mutation with renderer proposals and publishes its commit', async () => {
		const current = document('4', 'Canonical');
		const candidate = document('5', 'Owned mutation');
		const store = storeWith({ document: current, kind: 'available' });
		store.commit.mockResolvedValue({ kind: 'committed', revision: candidate.revision });
		const reconcileDocument = vi.fn<() => Promise<void>>(async () => undefined);
		const coordinator = coordinatorWith(store, { reconcileDocument });
		await coordinator.start();
		reconcileDocument.mockClear();
		const publications: DesktopClientSessionPublication[] = [];
		register(coordinator, publisher(publications));

		await expect(
			coordinator.mutateCanonicalDocument((canonical) => {
				expect(canonical).toBe(current);
				return { document: candidate, kind: 'commit' };
			}),
		).resolves.toEqual({ document: candidate, kind: 'committed' });

		expect(store.commit).toHaveBeenCalledWith(current.revision, candidate);
		expect(coordinator.canonicalDocument).toBe(candidate);
		expect(publications).toMatchObject([
			{ document: candidate, sequence: '1', type: 'shared-document-published' },
		]);
		expect(reconcileDocument).toHaveBeenCalledWith(candidate, expect.any(Function));
	});

	it('establishes revision zero before an owned mutation wins the cold-start race', async () => {
		const store = storeWith({ kind: 'missing' });
		store.commit.mockImplementation((_expectedRevision, candidate) =>
			Promise.resolve({ kind: 'committed', revision: candidate.revision }),
		);
		const reconcileDocument = vi.fn<() => Promise<void>>(async () => undefined);
		const coordinator = coordinatorWith(store, { reconcileDocument });
		await coordinator.start();
		const publications: DesktopClientSessionPublication[] = [];
		register(coordinator, publisher(publications));
		const candidate = document('1', 'Owned cold-start mutation');

		await expect(
			coordinator.mutateCanonicalDocument((canonical) => {
				expect(canonical).toEqual(document('0', 'Initial'));
				return { document: candidate, kind: 'commit' };
			}),
		).resolves.toEqual({ document: candidate, kind: 'committed' });

		expect(store.commit.mock.calls).toEqual([
			[null, document('0', 'Initial')],
			[workbenchSharedDocumentRevision('0'), candidate],
		]);
		expect(publications).toMatchObject([
			{ document: document('0', 'Initial'), sequence: '1', type: 'shared-document-published' },
			{ document: candidate, sequence: '2', type: 'shared-document-published' },
		]);
		expect(reconcileDocument).toHaveBeenCalledTimes(2);
	});

	it('reports the durable commit before post-commit publication and reconciliation', async () => {
		const current = document('4', 'Canonical');
		const candidate = document('5', 'Owned mutation');
		const trace: string[] = [];
		const store = storeWith({ document: current, kind: 'available' });
		store.commit.mockImplementation(async () => {
			trace.push('store-committed');
			return { kind: 'committed', revision: candidate.revision };
		});
		const coordinator = coordinatorWith(store, {
			reconcileDocument: async () => {
				trace.push('reconciled');
			},
		});
		await coordinator.start();
		trace.length = 0;
		register(
			coordinator,
			publisher([], false, () => {
				trace.push('published');
				return false;
			}),
		);

		await coordinator.mutateCanonicalDocument(
			() => ({ document: candidate, kind: 'commit' }),
			() => {
				trace.push('commit-observed');
			},
		);

		expect(trace).toEqual(['store-committed', 'commit-observed', 'published', 'reconciled']);
	});

	it('contains a post-commit observer failure without suppressing publication or reconciliation', async () => {
		const current = document('4', 'Canonical');
		const candidate = document('5', 'Owned mutation');
		const trace: string[] = [];
		const store = storeWith({ document: current, kind: 'available' });
		store.commit.mockImplementation(async () => {
			trace.push('store-committed');
			return { kind: 'committed', revision: candidate.revision };
		});
		const coordinator = coordinatorWith(store, {
			reconcileDocument: async () => {
				trace.push('reconciled');
			},
		});
		await coordinator.start();
		trace.length = 0;
		register(
			coordinator,
			publisher([], false, () => {
				trace.push('published');
				return false;
			}),
		);

		await expect(
			coordinator.mutateCanonicalDocument(
				() => ({ document: candidate, kind: 'commit' }),
				() => {
					trace.push('observer-failed');
					throw new Error('post-commit observer failed');
				},
			),
		).resolves.toEqual({ document: candidate, kind: 'committed' });

		expect(coordinator.canonicalDocument).toBe(candidate);
		expect(trace).toEqual(['store-committed', 'observer-failed', 'published', 'reconciled']);
	});

	it('keeps canonical state unchanged when an owned mutation is rejected', async () => {
		const current = document('4', 'Canonical');
		const store = storeWith({ document: current, kind: 'available' });
		const coordinator = coordinatorWith(store);
		await coordinator.start();

		await expect(
			coordinator.mutateCanonicalDocument(() => ({
				kind: 'rejected',
				reason: 'project-not-found',
			})),
		).resolves.toEqual({ kind: 'rejected', reason: 'project-not-found' });
		expect(store.commit).not.toHaveBeenCalled();
		expect(coordinator.canonicalDocument).toBe(current);
	});

	it('returns an unchanged canonical mutation without allocating a revision or writing', async () => {
		const current = document('4', 'Canonical');
		const store = storeWith({ document: current, kind: 'available' });
		const coordinator = coordinatorWith(store);
		await coordinator.start();

		await expect(
			coordinator.mutateCanonicalDocument((canonical) => {
				expect(canonical).toBe(current);
				return { kind: 'unchanged' };
			}),
		).resolves.toEqual({ document: current, kind: 'unchanged' });
		expect(store.commit).not.toHaveBeenCalled();
		expect(coordinator.canonicalDocument).toBe(current);
	});

	it('explicitly reconciles the unchanged canonical document without another durable commit', async () => {
		const current = document('4', 'Canonical');
		const store = storeWith({ document: current, kind: 'available' });
		const reconcileDocument = vi.fn<() => Promise<void>>(async () => undefined);
		const coordinator = coordinatorWith(store, { reconcileDocument });

		await expect(coordinator.reconcileCanonicalDocument()).resolves.toEqual({
			kind: 'rejected',
			reason: 'control-unavailable',
		});
		await coordinator.start();
		reconcileDocument.mockClear();

		await expect(coordinator.reconcileCanonicalDocument()).resolves.toEqual({
			document: current,
			kind: 'reconciled',
		});
		expect(reconcileDocument).toHaveBeenCalledWith(current, expect.any(Function));
		expect(store.commit).not.toHaveBeenCalled();
	});

	it('freezes canonical control after a post-rename mutation outcome is indeterminate', async () => {
		const current = document('4', 'Canonical');
		const candidate = document('5', 'Possibly durable');
		const store = storeWith({ document: current, kind: 'available' });
		store.commit.mockResolvedValue({ kind: 'unavailable', reason: 'write-indeterminate' });
		const coordinator = coordinatorWith(store);
		await coordinator.start();
		const publications: DesktopClientSessionPublication[] = [];
		register(coordinator, publisher(publications));

		await expect(
			coordinator.mutateCanonicalDocument(() => ({ document: candidate, kind: 'commit' })),
		).resolves.toEqual({ kind: 'indeterminate', reason: 'durability-unconfirmed' });
		expect(coordinator.canonicalDocument).toBe(current);
		expect(publications).toMatchObject([
			{
				reason: 'write-failed',
				type: 'client-session-coordinator-unavailable',
			},
		]);
		await expect(
			coordinator.mutateCanonicalDocument(() => ({ document: candidate, kind: 'commit' })),
		).resolves.toEqual({ kind: 'rejected', reason: 'control-unavailable' });
		expect(store.commit).toHaveBeenCalledOnce();
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

	it('commits development restart staging without provisioning against the retiring Host', async () => {
		const current = document('3', 'Terminal', { intent: { kind: 'stopped' } });
		const candidate = document('4', 'Terminal', {
			intent: {
				kind: 'provision',
				operationId: terminalPresentationOperationId('restart-operation'),
			},
			nextTerminalSequence: 2,
			presentationId: 'presentation-2',
		});
		const store = storeWith({ document: current, kind: 'available' });
		store.commit.mockResolvedValue({ kind: 'committed', revision: candidate.revision });
		const reconcileDocument = vi.fn<() => Promise<void>>(async () => undefined);
		const coordinator = coordinatorWith(store, { reconcileDocument });
		await coordinator.start();
		reconcileDocument.mockClear();
		const client = register(coordinator, publisher());
		expect(coordinator.beginQuiescing(client.clientSessionId, 'development-host-restart')).toBe(
			true,
		);

		await expect(
			client.propose('request-restart-staging', current.revision, candidate),
		).resolves.toEqual({ document: candidate, kind: 'committed' });
		expect(store.commit).toHaveBeenCalledWith(current.revision, candidate);
		expect(reconcileDocument).not.toHaveBeenCalled();
	});

	it('reconciles local archive ownership on load and every committed document replacement', async () => {
		const current = document('3', 'Stopped', { intent: { kind: 'stopped' } });
		const candidate = document('4', 'Fresh', {
			intent: {
				kind: 'provision',
				operationId: terminalPresentationOperationId('fresh-operation'),
			},
			presentationId: 'presentation-fresh',
		});
		const store = storeWith({ document: current, kind: 'available' });
		store.commit.mockResolvedValue({ kind: 'committed', revision: candidate.revision });
		const reconcileTerminalArchives = vi
			.fn<NonNullable<DesktopClientSessionCoordinatorConstructor['reconcileTerminalArchives']>>()
			.mockResolvedValue();
		const coordinator = coordinatorWith(store, { reconcileTerminalArchives });

		await coordinator.start();
		const client = register(coordinator, publisher());
		await client.propose('replace-stopped', current.revision, candidate);

		expect(reconcileTerminalArchives.mock.calls).toEqual([[current], [candidate]]);
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

	it('bootstraps the latest terminal metadata and publishes only current revisions', async () => {
		const coordinator = coordinatorWith(storeWith({ kind: 'missing' }));
		await coordinator.start();
		const first = WorkbenchTerminalMetadataSchema.parse({
			currentDirectory: 'file://orchbee-host/workspace',
			identity: {
				creationId: '11'.repeat(16),
				hostEpoch: '22'.repeat(16),
				terminalId: '33'.repeat(16),
			},
			presentationId: 'presentation-1',
			revision: '7',
			title: 'first title',
		});

		expect(coordinator.publishTerminalMetadata(first)).toBe(true);
		const publications: DesktopClientSessionPublication[] = [];
		const session = register(coordinator, publisher(publications));
		expect(session.bootstrap.terminalMetadata).toEqual([first]);
		expect(publications).toEqual([]);

		expect(
			coordinator.publishTerminalMetadata(
				WorkbenchTerminalMetadataSchema.parse({
					...first,
					revision: '6',
					title: 'stale title',
				}),
			),
		).toBe(true);
		expect(publications).toEqual([]);

		const current = WorkbenchTerminalMetadataSchema.parse({
			...first,
			revision: '8',
			title: 'current title',
		});
		expect(coordinator.publishTerminalMetadata(current)).toBe(true);
		expect(publications).toMatchObject([
			{ metadata: current, sequence: '2', type: 'terminal-metadata-published' },
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

	it('drains its captured proposal tail before retiring session attachments', async () => {
		let finishCommit:
			| ((result: Awaited<ReturnType<WorkbenchDocumentStore['commit']>>) => void)
			| undefined;
		const trace: string[] = [];
		const store = storeWith({ kind: 'missing' });
		store.commit.mockImplementation(
			() =>
				new Promise((resolve) => {
					finishCommit = resolve;
				}),
		);
		const coordinator = coordinatorWith(store, {
			reconcileDocument: async () => {
				trace.push('reconciled');
			},
		});
		await coordinator.start();
		const firstPublisher = publisher([], false, () => {
			trace.push('published:first');
			return false;
		});
		firstPublisher.disposeAttachments.mockImplementation(async () => {
			trace.push('retired:first');
		});
		const secondPublisher = publisher([], false, () => {
			trace.push('published:second');
			return false;
		});
		secondPublisher.disposeAttachments.mockImplementation(async () => {
			trace.push('retired:second');
		});
		const first = register(coordinator, firstPublisher);
		const second = register(coordinator, secondPublisher);
		const accepted = first.propose('request-accepted', null, document('0', 'Accepted'));
		await vi.waitFor(() => expect(store.commit).toHaveBeenCalledOnce());
		const queued = second.propose('request-queued', null, document('0', 'Queued'));

		let disposalSettled = false;
		const disposal = coordinator.dispose().then(() => {
			disposalSettled = true;
		});
		const postSealMutation = vi.fn<() => { readonly kind: 'unchanged' }>(() => ({
			kind: 'unchanged',
		}));
		const rejectedAfterSeal = [
			first.propose('request-after-seal', null, document('0', 'After seal')),
			coordinator.mutateCanonicalDocument(postSealMutation),
			coordinator.reconcileCanonicalDocument(),
		];
		let rejectedAfterSealCount = 0;
		for (const result of rejectedAfterSeal) {
			void result.then(() => {
				rejectedAfterSealCount += 1;
			});
		}
		await Promise.resolve();
		await Promise.resolve();

		expect(disposalSettled).toBe(false);
		expect(firstPublisher.disposeAttachments).not.toHaveBeenCalled();
		expect(secondPublisher.disposeAttachments).not.toHaveBeenCalled();
		expect(rejectedAfterSealCount).toBe(3);
		expect(postSealMutation).not.toHaveBeenCalled();

		finishCommit?.({ kind: 'committed', revision: workbenchSharedDocumentRevision('0') });

		expect(await accepted).toMatchObject({ kind: 'committed' });
		expect(await queued).toMatchObject({ kind: 'rejected', reason: 'control-unavailable' });
		await expect(Promise.all(rejectedAfterSeal)).resolves.toEqual([
			{ document: null, kind: 'rejected', reason: 'control-unavailable' },
			{ kind: 'rejected', reason: 'control-unavailable' },
			{ kind: 'rejected', reason: 'control-unavailable' },
		]);
		await disposal;
		expect(trace).toEqual([
			'published:first',
			'published:second',
			'reconciled',
			'retired:first',
			'retired:second',
		]);
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
		readonly reconcileTerminalArchives: NonNullable<
			DesktopClientSessionCoordinatorConstructor['reconcileTerminalArchives']
		>;
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
		...(overrides.reconcileTerminalArchives === undefined
			? {}
			: { reconcileTerminalArchives: overrides.reconcileTerminalArchives }),
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

function document(
	revision: string,
	title: string,
	options: {
		readonly intent?: WorkbenchSharedPresentationIntent;
		readonly nextTerminalSequence?: number;
		readonly presentationId?: string;
	} = {},
): WorkbenchSharedDocument {
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
					nextTerminalSequence: options.nextTerminalSequence ?? 2,
					pendingRemovals: [],
					tabs: [
						{
							root: {
								childPresentations: [],
								id: 'panel-1',
								kind: 'panel',
								primaryPresentation: {
									intent: options.intent ?? {
										kind: 'provision',
										operationId: terminalPresentationOperationId('operation-1'),
									},
									launchCwd: '/workspace',
									presentationId: terminalPresentationId(
										options.presentationId ?? 'presentation-1',
									),
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
