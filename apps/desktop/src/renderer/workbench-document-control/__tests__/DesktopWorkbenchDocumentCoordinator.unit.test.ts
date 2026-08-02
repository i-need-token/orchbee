import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
	WorkbenchProjectWorkspaceRuntime,
	type TerminalPresentationCommand,
	type WorkbenchTerminalPresentationCommand,
} from '@orchbee/workbench';
import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
	type WorkbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';
import { describe, expect, it, vi } from 'vitest';

import { DesktopWorkbenchDocumentCoordinator } from '../DesktopWorkbenchDocumentCoordinator';

type DocumentCommit = (
	expectedRevision: WorkbenchSharedDocumentRevision | null,
	document: WorkbenchSharedDocument,
) => Promise<
	| { readonly kind: 'committed'; readonly revision: WorkbenchSharedDocumentRevision }
	| { readonly kind: 'unavailable'; readonly reason: 'write-failed' }
>;

class TestDocumentRuntime {
	readonly #listeners = new Set<() => void>();
	#sequence = 1;

	getSharedDocument(revision: WorkbenchSharedDocumentRevision): WorkbenchSharedDocument {
		return sharedDocument(revision, this.#sequence);
	}

	subscribeDocument(listener: () => void): () => void {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	change(sequence: number): void {
		this.#sequence = sequence;
		for (const listener of this.#listeners) listener();
	}

	notifyClientViewChange(): void {
		for (const listener of this.#listeners) listener();
	}
}

function sharedDocument(
	revision: WorkbenchSharedDocumentRevision,
	projectSequence = 1,
): WorkbenchSharedDocument {
	return createWorkbenchSharedDocument({
		projectWorkspace: {
			nextProjectSequence: projectSequence,
			nextTerminalContextSequence: 2,
			nextWorkspaceSequence: 1,
			projects: [],
			standaloneTerminalContextId: 'terminal-context-1',
		},
		revision,
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
					groups: [],
					tabs: [],
				},
				terminalContextId: 'terminal-context-1',
			},
		],
	});
}

function provisionCommand(operation = 'provision-1'): TerminalPresentationCommand {
	return {
		launchCwd: '/workspace',
		operationId: terminalPresentationOperationId(operation),
		presentationId: terminalPresentationId('presentation-1'),
		type: 'terminal-provision-requested',
	};
}

function closeCommand(operation = 'close-1'): TerminalPresentationCommand {
	return {
		operationId: terminalPresentationOperationId(operation),
		presentationId: terminalPresentationId('presentation-1'),
		type: 'terminal-close-requested',
	};
}

function restoreCommand(): TerminalPresentationCommand {
	return {
		launchCwd: '/workspace',
		operationId: terminalPresentationOperationId('restore-1'),
		presentationId: terminalPresentationId('presentation-1'),
		type: 'terminal-restore-requested',
	};
}

function deferred<Value>() {
	let resolve = (_value: Value): void => undefined;
	const promise = new Promise<Value>((nextResolve) => {
		resolve = nextResolve;
	});
	return { promise, resolve };
}

describe('DesktopWorkbenchDocumentCoordinator', () => {
	it('commits missing revision zero before dispatching initial provision', async () => {
		const runtime = new TestDocumentRuntime();
		const commit = deferred<{
			readonly kind: 'committed';
			readonly revision: WorkbenchSharedDocumentRevision;
		}>();
		const documentPort = { commit: vi.fn<DocumentCommit>(() => commit.promise) };
		const commandPort = {
			dispatch: vi.fn<(commands: readonly TerminalPresentationCommand[]) => void>(),
		};
		const coordinator = new DesktopWorkbenchDocumentCoordinator(runtime, documentPort, commandPort);

		const start = coordinator.start({ kind: 'missing' }, [provisionCommand()]);
		expect(commandPort.dispatch).not.toHaveBeenCalled();
		expect(documentPort.commit).toHaveBeenCalledWith(
			null,
			sharedDocument(workbenchSharedDocumentRevision('0')),
		);
		commit.resolve({ kind: 'committed', revision: workbenchSharedDocumentRevision('0') });

		await expect(start).resolves.toEqual({ kind: 'ready', revision: '0' });
		await vi.waitFor(() => expect(commandPort.dispatch).toHaveBeenCalledWith([provisionCommand()]));
	});

	it('dispatches restore against already committed retain document without a redundant write', async () => {
		const runtime = new TestDocumentRuntime();
		const documentPort = { commit: vi.fn<DocumentCommit>() };
		const commandPort = {
			dispatch: vi.fn<(commands: readonly TerminalPresentationCommand[]) => void>(),
		};
		const coordinator = new DesktopWorkbenchDocumentCoordinator(runtime, documentPort, commandPort);

		await coordinator.start(
			{ document: sharedDocument(workbenchSharedDocumentRevision('7')), kind: 'available' },
			[restoreCommand()],
		);

		await vi.waitFor(() => expect(commandPort.dispatch).toHaveBeenCalledWith([restoreCommand()]));
		expect(documentPort.commit).not.toHaveBeenCalled();
	});

	it('does not commit when a Client View notification leaves the shared fingerprint unchanged', async () => {
		const runtime = new TestDocumentRuntime();
		const documentPort = { commit: vi.fn<DocumentCommit>() };
		const coordinator = new DesktopWorkbenchDocumentCoordinator(runtime, documentPort, {
			dispatch: vi.fn<(commands: readonly TerminalPresentationCommand[]) => void>(),
		});
		await coordinator.start({
			document: sharedDocument(workbenchSharedDocumentRevision('7')),
			kind: 'available',
		});

		runtime.notifyClientViewChange();

		await expect(coordinator.flush()).resolves.toEqual({ kind: 'flushed', revision: '7' });
		expect(documentPort.commit).not.toHaveBeenCalled();
	});

	it('flushes only after the current document is durable and its prepared commands are released', async () => {
		const runtime = new TestDocumentRuntime();
		const commit = deferred<{
			readonly kind: 'committed';
			readonly revision: WorkbenchSharedDocumentRevision;
		}>();
		const documentPort = { commit: vi.fn<DocumentCommit>(() => commit.promise) };
		const commandPort = {
			dispatch: vi.fn<(commands: readonly TerminalPresentationCommand[]) => void>(),
		};
		const coordinator = new DesktopWorkbenchDocumentCoordinator(runtime, documentPort, commandPort);
		await coordinator.start({
			document: sharedDocument(workbenchSharedDocumentRevision('7')),
			kind: 'available',
		});
		runtime.change(2);
		coordinator.dispatchCommands([closeCommand()]);
		const flushed = coordinator.flush();
		let settled = false;
		void flushed.then(() => {
			settled = true;
		});
		await vi.waitFor(() => expect(documentPort.commit).toHaveBeenCalledOnce());
		expect(settled).toBe(false);
		expect(commandPort.dispatch).not.toHaveBeenCalled();

		commit.resolve({ kind: 'committed', revision: workbenchSharedDocumentRevision('8') });
		await expect(flushed).resolves.toEqual({ kind: 'flushed', revision: '8' });
		expect(commandPort.dispatch).toHaveBeenCalledWith([closeCommand()]);
	});

	it('binds each command batch to the complete snapshot captured before its commit', async () => {
		const runtime = new TestDocumentRuntime();
		const first = deferred<{
			readonly kind: 'committed';
			readonly revision: WorkbenchSharedDocumentRevision;
		}>();
		const second = deferred<{
			readonly kind: 'committed';
			readonly revision: WorkbenchSharedDocumentRevision;
		}>();
		const documentPort = {
			commit: vi
				.fn<DocumentCommit>()
				.mockReturnValueOnce(first.promise)
				.mockReturnValueOnce(second.promise),
		};
		const commandPort = {
			dispatch: vi.fn<(commands: readonly TerminalPresentationCommand[]) => void>(),
		};
		const coordinator = new DesktopWorkbenchDocumentCoordinator(runtime, documentPort, commandPort);
		await coordinator.start({
			document: sharedDocument(workbenchSharedDocumentRevision('7')),
			kind: 'available',
		});

		runtime.change(2);
		coordinator.dispatchCommands([provisionCommand()]);
		await vi.waitFor(() => expect(documentPort.commit).toHaveBeenCalledTimes(1));
		runtime.change(3);
		coordinator.dispatchCommands([closeCommand()]);
		first.resolve({ kind: 'committed', revision: workbenchSharedDocumentRevision('8') });

		await vi.waitFor(() => {
			expect(commandPort.dispatch).toHaveBeenNthCalledWith(1, [provisionCommand()]);
			expect(documentPort.commit).toHaveBeenCalledTimes(2);
		});
		expect(documentPort.commit).toHaveBeenNthCalledWith(
			2,
			workbenchSharedDocumentRevision('8'),
			sharedDocument(workbenchSharedDocumentRevision('9'), 3),
		);
		expect(commandPort.dispatch).toHaveBeenCalledTimes(1);
		second.resolve({ kind: 'committed', revision: workbenchSharedDocumentRevision('9') });
		await vi.waitFor(() =>
			expect(commandPort.dispatch).toHaveBeenNthCalledWith(2, [closeCommand()]),
		);
	});

	it('fails closed after CAS failure and never releases the affected command', async () => {
		const runtime = new TestDocumentRuntime();
		const documentPort = {
			commit: vi
				.fn<DocumentCommit>()
				.mockResolvedValue({ kind: 'unavailable', reason: 'write-failed' }),
		};
		const commandPort = {
			dispatch: vi.fn<(commands: readonly TerminalPresentationCommand[]) => void>(),
		};
		const coordinator = new DesktopWorkbenchDocumentCoordinator(runtime, documentPort, commandPort);
		await coordinator.start({
			document: sharedDocument(workbenchSharedDocumentRevision('7')),
			kind: 'available',
		});

		runtime.change(2);
		coordinator.dispatchCommands([provisionCommand()]);

		await vi.waitFor(() => expect(coordinator.failureReason).toBe('write-failed'));
		expect(commandPort.dispatch).not.toHaveBeenCalled();
		runtime.change(3);
		coordinator.dispatchCommands([closeCommand()]);
		await Promise.resolve();
		expect(documentPort.commit).toHaveBeenCalledOnce();
		expect(commandPort.dispatch).not.toHaveBeenCalled();
	});

	it('persists managed close intent before the effect and commits container cleanup afterward', async () => {
		let operationSequence = 1;
		const bufferedCommands: TerminalPresentationCommand[] = [];
		let coordinator: DesktopWorkbenchDocumentCoordinator | undefined;
		const runtime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/workspace',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: {
				createOperationId: () =>
					terminalPresentationOperationId(`operation-${operationSequence++}`),
				createPresentationId: () => terminalPresentationId('presentation-dynamic'),
				createRemovalId: () => terminalPresentationRemovalId('removal-1'),
				dispatchCommands: (commands: readonly WorkbenchTerminalPresentationCommand[]) => {
					const terminalCommands = commands.filter(
						(command): command is TerminalPresentationCommand => command.type !== 'tab-close-ready',
					);
					if (coordinator === undefined) bufferedCommands.push(...terminalCommands);
					else coordinator.dispatchCommands(terminalCommands);
				},
				initialPresentationId: terminalPresentationId('presentation-1'),
				kind: 'managed',
			},
		});
		const timeline: string[] = [];
		const committedDocuments: WorkbenchSharedDocument[] = [];
		const dispatchedCommands: TerminalPresentationCommand[] = [];
		const documentPort = {
			commit: vi.fn<DocumentCommit>(
				async (
					_expected: WorkbenchSharedDocumentRevision | null,
					candidate: WorkbenchSharedDocument,
				) => {
					committedDocuments.push(candidate);
					timeline.push(`commit:${intentIn(candidate)}`);
					return { kind: 'committed' as const, revision: candidate.revision };
				},
			),
		};
		const commandPort = {
			dispatch: (commands: readonly TerminalPresentationCommand[]) => {
				dispatchedCommands.push(...commands);
				for (const command of commands) timeline.push(`dispatch:${command.type}`);
			},
		};
		coordinator = new DesktopWorkbenchDocumentCoordinator(runtime, documentPort, commandPort);

		await coordinator.start({ kind: 'missing' }, bufferedCommands.splice(0));
		await vi.waitFor(() => expect(dispatchedCommands).toHaveLength(1));
		const provision = dispatchedCommands[0];
		if (provision?.type !== 'terminal-provision-requested') {
			throw new Error('Expected initial provision.');
		}
		runtime.applyTerminalPresentationEvent({
			operationId: provision.operationId,
			presentationId: provision.presentationId,
			type: 'terminal-provision-completed',
		});
		await vi.waitFor(() => expect(intentIn(committedDocuments.at(-1))).toBe('retain'));

		runtime.requestActiveTerminalTabsClose(['terminal-context-1-tab-1']);
		await vi.waitFor(() => expect(dispatchedCommands).toHaveLength(2));
		const close = dispatchedCommands[1];
		if (close?.type !== 'terminal-close-requested') throw new Error('Expected close.');
		const closeCommitIndex = timeline.indexOf('commit:close');
		const closeDispatchIndex = timeline.indexOf('dispatch:terminal-close-requested');
		expect(closeCommitIndex).toBeGreaterThan(-1);
		expect(closeDispatchIndex).toBeGreaterThan(closeCommitIndex);

		runtime.applyTerminalPresentationEvent({
			operationId: close.operationId,
			presentationId: close.presentationId,
			type: 'terminal-close-completed',
		});
		await vi.waitFor(() =>
			expect(committedDocuments.at(-1)?.terminalContexts[0]?.tabCollection.tabs).toEqual([]),
		);
		expect(timeline.at(-1)).toBe('commit:absent');
		coordinator.dispose();
	});
});

function intentIn(candidateDocument: WorkbenchSharedDocument | undefined): string {
	const root = candidateDocument?.terminalContexts[0]?.panelWorkspace.tabs[0]?.root;
	if (root?.kind !== 'panel') return 'absent';
	return root.primaryPresentation.intent.kind;
}
