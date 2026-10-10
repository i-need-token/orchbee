import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
} from '@orchbee/workbench/document';
import {
	terminalPresentationId,
	type TerminalPresentationEvent,
} from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import {
	WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
	WorkbenchClientSessionBootstrapSchema,
	WorkbenchClientSessionEventSchema,
	type WorkbenchClientSessionEvent,
	type WorkbenchDocumentProposalResult,
} from '@orchbee/workbench-client-protocol';
import { WorkbenchDocumentCoordinator } from '../WorkbenchDocumentCoordinator';

describe('WorkbenchDocumentCoordinator', () => {
	it('proposes the common initialization document for a missing canonical store', async () => {
		const initial = document('0', 1);
		const runtime = runtimeWith(initial);
		const session = sessionWith(
			bootstrapWith({ kind: 'missing' }, initial),
			vi.fn().mockResolvedValue({ document: initial, kind: 'committed' }),
		);
		const coordinator = new WorkbenchDocumentCoordinator(runtime, session);

		expect(await coordinator.start()).toEqual({ kind: 'ready', revision: '0' });
		expect(session.propose).toHaveBeenCalledWith(null, initial);
		expect(coordinator.committedRevision).toBe('0');
	});

	it('starts from one available bootstrap and replays its terminal observations without effects', async () => {
		const current = document('7', 1);
		const event = {
			exitCode: 0,
			presentationId: terminalPresentationId('presentation-1'),
			signal: null,
			type: 'terminal-exited',
		} as const;
		const runtime = runtimeWith(current);
		const session = sessionWith(
			bootstrapWith({ document: current, kind: 'available' }, document('0', 1), [event]),
			vi.fn(),
		);
		const coordinator = new WorkbenchDocumentCoordinator(runtime, session);

		expect(await coordinator.start()).toEqual({ kind: 'ready', revision: '7' });
		expect(runtime.applyTerminalPresentationEvent).toHaveBeenCalledWith(event);
		expect(session.propose).not.toHaveBeenCalled();
	});

	it('applies canonical stale state without replaying the losing local change', async () => {
		const canonical = document('3', 1);
		const local = document('4', 2);
		const runtime = runtimeWith(canonical);
		const propose = vi
			.fn<
				(
					expectedRevision: WorkbenchSharedDocument['revision'] | null,
					document: WorkbenchSharedDocument,
				) => Promise<WorkbenchDocumentProposalResult>
			>()
			.mockResolvedValue({ document: canonical, kind: 'stale' });
		const session = sessionWith(
			bootstrapWith({ document: canonical, kind: 'available' }, document('0', 1)),
			propose,
		);
		const coordinator = new WorkbenchDocumentCoordinator(runtime, session);
		await coordinator.start();

		runtime.mutate(local);
		expect(await coordinator.flush()).toEqual({ kind: 'flushed', revision: '3' });

		expect(propose).toHaveBeenCalledOnce();
		expect(runtime.applySharedDocument).toHaveBeenCalledWith(canonical);
		expect(runtime.current).toEqual(canonical);
	});

	it('does not roll a terminal publication back when its document proposal settles later', async () => {
		const initial = document('0', 1);
		const afterTerminalEvent = document('0', 2);
		const runtime = runtimeWith(initial, afterTerminalEvent);
		const event = {
			exitCode: 0,
			presentationId: terminalPresentationId('presentation-1'),
			signal: null,
			type: 'terminal-exited',
		} as const;
		let session: ReturnType<typeof sessionWith>;
		const propose = vi.fn<
			(
				expectedRevision: WorkbenchSharedDocument['revision'] | null,
				candidate: WorkbenchSharedDocument,
			) => Promise<WorkbenchDocumentProposalResult>
		>(
			async (
				expectedRevision: WorkbenchSharedDocument['revision'] | null,
				candidate: WorkbenchSharedDocument,
			): Promise<WorkbenchDocumentProposalResult> => {
				if (expectedRevision === null) {
					session.publish({ document: initial, sequence: '1', type: 'shared-document-published' });
					session.publish({
						event,
						sequence: '2',
						type: 'terminal-presentation-event-published',
					});
					return { document: initial, kind: 'committed' };
				}
				session.publish({
					document: candidate,
					sequence: '3',
					type: 'shared-document-published',
				});
				return { document: candidate, kind: 'committed' };
			},
		);
		session = sessionWith(bootstrapWith({ kind: 'missing' }, initial), propose);
		const coordinator = new WorkbenchDocumentCoordinator(runtime, session);

		expect(await coordinator.start()).toEqual({ kind: 'ready', revision: '1' });
		expect(runtime.applyTerminalPresentationEvent).toHaveBeenCalledWith(event);
		expect(runtime.applySharedDocument).not.toHaveBeenCalled();
		expect(runtime.current.terminalContexts[0]?.nextTabSequence).toBe(2);
	});

	it('preserves a burst of later local Tab intent when an active proposal is acknowledged', async () => {
		const initial = document('0', 1);
		const firstResult = createDeferred<WorkbenchDocumentProposalResult>();
		const secondResult = createDeferred<WorkbenchDocumentProposalResult>();
		const propose = vi
			.fn<
				(
					expectedRevision: WorkbenchSharedDocument['revision'] | null,
					candidate: WorkbenchSharedDocument,
				) => Promise<WorkbenchDocumentProposalResult>
			>()
			.mockImplementationOnce(() => firstResult.promise)
			.mockImplementationOnce(() => secondResult.promise);
		const runtime = runtimeWith(initial);
		const session = sessionWith(
			bootstrapWith({ document: initial, kind: 'available' }, initial),
			propose,
		);
		const coordinator = new WorkbenchDocumentCoordinator(runtime, session);
		await coordinator.start();

		runtime.mutate(document('1', 2));
		await vi.waitFor(() => expect(propose).toHaveBeenCalledOnce());
		const firstCandidate = propose.mock.calls[0]?.[1];
		if (firstCandidate === undefined) throw new Error('Expected the first proposal candidate.');
		for (let nextTabSequence = 3; nextTabSequence <= 33; nextTabSequence += 1) {
			runtime.mutate(document('2', nextTabSequence));
		}
		session.publish({
			document: firstCandidate,
			sequence: '1',
			type: 'shared-document-published',
		});
		firstResult.resolve({ document: firstCandidate, kind: 'committed' });

		await vi.waitFor(() => expect(propose).toHaveBeenCalledTimes(2));
		expect(runtime.applySharedDocument).not.toHaveBeenCalled();
		expect(runtime.current.terminalContexts[0]?.nextTabSequence).toBe(33);
		const secondCandidate = propose.mock.calls[1]?.[1];
		if (secondCandidate === undefined) throw new Error('Expected the second proposal candidate.');
		expect(secondCandidate.terminalContexts[0]?.nextTabSequence).toBe(33);
		const flush = coordinator.flush();
		session.publish({
			document: secondCandidate,
			sequence: '2',
			type: 'shared-document-published',
		});
		secondResult.resolve({ document: secondCandidate, kind: 'committed' });

		await expect(flush).resolves.toEqual({ kind: 'flushed', revision: '2' });
		expect(runtime.current.terminalContexts[0]?.nextTabSequence).toBe(33);
	});

	it('applies a divergent publication while a local proposal is active', async () => {
		const initial = document('0', 1);
		const proposalResult = createDeferred<WorkbenchDocumentProposalResult>();
		const runtime = runtimeWith(initial);
		const session = sessionWith(
			bootstrapWith({ document: initial, kind: 'available' }, initial),
			() => proposalResult.promise,
		);
		const coordinator = new WorkbenchDocumentCoordinator(runtime, session);
		await coordinator.start();

		runtime.mutate(document('1', 2));
		await vi.waitFor(() => expect(session.propose).toHaveBeenCalledOnce());
		runtime.mutate(document('2', 3));
		const divergent = document('1', 9);
		session.publish({
			document: divergent,
			sequence: '1',
			type: 'shared-document-published',
		});
		proposalResult.resolve({ document: divergent, kind: 'stale' });

		await expect(coordinator.flush()).resolves.toEqual({ kind: 'flushed', revision: '1' });
		expect(runtime.applySharedDocument).toHaveBeenCalledOnce();
		expect(runtime.applySharedDocument).toHaveBeenCalledWith(divergent);
		expect(runtime.current).toEqual(divergent);
	});

	it('applies ordered document and terminal publications from another client', async () => {
		const current = document('0', 1);
		const incoming = document('1', 2);
		const runtime = runtimeWith(current);
		const session = sessionWith(
			bootstrapWith({ document: current, kind: 'available' }, current),
			vi.fn(),
		);
		const coordinator = new WorkbenchDocumentCoordinator(runtime, session);
		await coordinator.start();
		const event = {
			exitCode: 0,
			presentationId: terminalPresentationId('presentation-1'),
			signal: null,
			type: 'terminal-exited',
		} as const;

		session.publish({ document: incoming, sequence: '1', type: 'shared-document-published' });
		session.publish({
			event,
			sequence: '2',
			type: 'terminal-presentation-event-published',
		});

		expect(runtime.applySharedDocument).toHaveBeenCalledWith(incoming);
		expect(runtime.applyTerminalPresentationEvent).toHaveBeenCalledWith(event);
		expect(coordinator.failureReason).toBeNull();
	});
});

function runtimeWith(
	initial: WorkbenchSharedDocument,
	afterTerminalEvent?: WorkbenchSharedDocument | undefined,
) {
	let current = initial;
	let listener: (() => void) | undefined;
	const runtime = {
		applySharedDocument: vi.fn<(next: WorkbenchSharedDocument) => { readonly kind: 'unchanged' }>(
			(next) => {
				current = next;
				return { kind: 'unchanged' as const };
			},
		),
		applyTerminalPresentationEvent: vi.fn<
			(_event: TerminalPresentationEvent) => { readonly kind: 'applied' }
		>(() => {
			if (afterTerminalEvent !== undefined) {
				current = afterTerminalEvent;
				listener?.();
			}
			return { kind: 'applied' };
		}),
		get current() {
			return current;
		},
		getSharedDocument: vi.fn<
			(revision: WorkbenchSharedDocument['revision']) => WorkbenchSharedDocument
		>((revision) => document(revision, current.terminalContexts[0]?.nextTabSequence ?? 1)),
		mutate: (next: WorkbenchSharedDocument) => {
			current = next;
			listener?.();
		},
		subscribeDocument: vi.fn<(nextListener: () => void) => () => void>((nextListener) => {
			listener = nextListener;
			return () => {
				listener = undefined;
			};
		}),
	};
	return runtime;
}

function sessionWith(
	bootstrap: ReturnType<typeof bootstrapWith>,
	propose: (
		expectedRevision: WorkbenchSharedDocument['revision'] | null,
		document: WorkbenchSharedDocument,
	) => Promise<WorkbenchDocumentProposalResult>,
) {
	let listener:
		| ((
				publication: Exclude<
					WorkbenchClientSessionEvent,
					{ readonly type: 'shared-document-proposal-completed' }
				>,
		  ) => void)
		| undefined;
	return {
		bootstrap,
		propose: vi.fn<typeof propose>(propose),
		publish: (publication: unknown) => {
			const parsed = WorkbenchClientSessionEventSchema.parse(publication);
			if (parsed.type === 'shared-document-proposal-completed') {
				throw new Error('Expected a publication event.');
			}
			listener?.(parsed);
		},
		subscribe: vi.fn<(nextListener: NonNullable<typeof listener>) => () => void>((nextListener) => {
			listener = nextListener;
			return () => {
				listener = undefined;
			};
		}),
	};
}

function bootstrapWith(
	documentState:
		| { readonly document: WorkbenchSharedDocument; readonly kind: 'available' }
		| { readonly kind: 'missing' },
	initializationDocument: WorkbenchSharedDocument,
	terminalEvents: readonly TerminalPresentationEvent[] = [],
) {
	return WorkbenchClientSessionBootstrapSchema.parse({
		clientSessionId: 'client-1',
		defaultCwd: '/workspace',
		documentState,
		initialPresentationId: 'presentation-initial',
		initializationDocument,
		sequence: '0',
		terminalHostName: 'orchbee-host',
		terminalEvents,
		type: WORKBENCH_CLIENT_SESSION_BOOTSTRAP,
	});
}

function document(revision: string, nextTabSequence: number): WorkbenchSharedDocument {
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
				nextTabSequence,
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
}

function createDeferred<Value>(): {
	readonly promise: Promise<Value>;
	readonly resolve: (value: Value) => void;
} {
	let resolve = (_value: Value): void => undefined;
	const promise = new Promise<Value>((nextResolve) => {
		resolve = nextResolve;
	});
	return { promise, resolve };
}
