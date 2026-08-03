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
	DESKTOP_CLIENT_SESSION_IPC_CHANNEL,
	DesktopClientSessionBootstrapSchema,
	DesktopClientSessionEventSchema,
	type DesktopClientSessionEvent,
	type DesktopDocumentProposalResult,
} from '../../../shared';
import { DesktopWorkbenchDocumentCoordinator } from '../DesktopWorkbenchDocumentCoordinator';

describe('DesktopWorkbenchDocumentCoordinator', () => {
	it('proposes the common initialization document for a missing canonical store', async () => {
		const initial = document('0', 1);
		const runtime = runtimeWith(initial);
		const session = sessionWith(
			bootstrapWith({ kind: 'missing' }, initial),
			vi.fn().mockResolvedValue({ document: initial, kind: 'committed' }),
		);
		const coordinator = new DesktopWorkbenchDocumentCoordinator(runtime, session);

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
		const coordinator = new DesktopWorkbenchDocumentCoordinator(runtime, session);

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
				) => Promise<DesktopDocumentProposalResult>
			>()
			.mockResolvedValue({ document: canonical, kind: 'stale' });
		const session = sessionWith(
			bootstrapWith({ document: canonical, kind: 'available' }, document('0', 1)),
			propose,
		);
		const coordinator = new DesktopWorkbenchDocumentCoordinator(runtime, session);
		await coordinator.start();

		runtime.mutate(local);
		expect(await coordinator.flush()).toEqual({ kind: 'flushed', revision: '3' });

		expect(propose).toHaveBeenCalledOnce();
		expect(runtime.applySharedDocument).toHaveBeenCalledWith(canonical);
		expect(runtime.current).toEqual(canonical);
	});

	it('applies ordered document and terminal publications from another client', async () => {
		const current = document('0', 1);
		const incoming = document('1', 2);
		const runtime = runtimeWith(current);
		const session = sessionWith(
			bootstrapWith({ document: current, kind: 'available' }, current),
			vi.fn(),
		);
		const coordinator = new DesktopWorkbenchDocumentCoordinator(runtime, session);
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

function runtimeWith(initial: WorkbenchSharedDocument) {
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
		>(() => ({ kind: 'applied' })),
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
	) => Promise<DesktopDocumentProposalResult>,
) {
	let listener:
		| ((
				publication: Exclude<
					DesktopClientSessionEvent,
					{ readonly type: 'shared-document-proposal-completed' }
				>,
		  ) => void)
		| undefined;
	return {
		bootstrap,
		propose: vi.fn<typeof propose>(propose),
		publish: (publication: unknown) => {
			const parsed = DesktopClientSessionEventSchema.parse(publication);
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
	return DesktopClientSessionBootstrapSchema.parse({
		clientSessionId: 'client-1',
		defaultCwd: '/workspace',
		documentState,
		initialPresentationId: 'presentation-initial',
		initializationDocument,
		sequence: '0',
		terminalEvents,
		type: DESKTOP_CLIENT_SESSION_IPC_CHANNEL,
	});
}

function document(revision: string, nextTabSequence: number): WorkbenchSharedDocument {
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
