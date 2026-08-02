import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
	WorkbenchProjectWorkspaceRuntime,
	type TerminalPresentationCommand,
	type WorkbenchTerminalPresentationCommand,
} from '@orchbee/workbench';
import {
	createWorkbenchTopologyDocument,
	workbenchTopologyRevision,
	type WorkbenchTopologyDocument,
	type WorkbenchTopologyRevision,
} from '@orchbee/workbench/topology';
import { describe, expect, it, vi } from 'vitest';

import { DesktopWorkbenchTopologyCoordinator } from '../DesktopWorkbenchTopologyCoordinator';

type TopologyCommit = (
	expectedRevision: WorkbenchTopologyRevision | null,
	document: WorkbenchTopologyDocument,
) => Promise<
	| { readonly kind: 'committed'; readonly revision: WorkbenchTopologyRevision }
	| { readonly kind: 'unavailable'; readonly reason: 'write-failed' }
>;

class TestTopologyRuntime {
	readonly #listeners = new Set<() => void>();
	#sequence = 1;

	getTopologyDocument(revision: WorkbenchTopologyRevision): WorkbenchTopologyDocument {
		return topology(revision, this.#sequence);
	}

	subscribeTopology(listener: () => void): () => void {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	change(sequence: number): void {
		this.#sequence = sequence;
		for (const listener of this.#listeners) listener();
	}
}

function topology(
	revision: WorkbenchTopologyRevision,
	projectSequence = 1,
): WorkbenchTopologyDocument {
	return createWorkbenchTopologyDocument({
		projectWorkspace: {
			activeLocation: { kind: 'standalone' },
			expandedProjectIds: [],
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
					activeTabId: null,
					groups: [],
					mostRecentGroupId: null,
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

describe('DesktopWorkbenchTopologyCoordinator', () => {
	it('commits missing revision zero before dispatching initial provision', async () => {
		const runtime = new TestTopologyRuntime();
		const commit = deferred<{
			readonly kind: 'committed';
			readonly revision: WorkbenchTopologyRevision;
		}>();
		const topologyPort = { commit: vi.fn<TopologyCommit>(() => commit.promise) };
		const commandPort = {
			dispatch: vi.fn<(commands: readonly TerminalPresentationCommand[]) => void>(),
		};
		const coordinator = new DesktopWorkbenchTopologyCoordinator(runtime, topologyPort, commandPort);

		const start = coordinator.start({ kind: 'missing' }, [provisionCommand()]);
		expect(commandPort.dispatch).not.toHaveBeenCalled();
		expect(topologyPort.commit).toHaveBeenCalledWith(
			null,
			topology(workbenchTopologyRevision('0')),
		);
		commit.resolve({ kind: 'committed', revision: workbenchTopologyRevision('0') });

		await expect(start).resolves.toEqual({ kind: 'ready', revision: '0' });
		await vi.waitFor(() => expect(commandPort.dispatch).toHaveBeenCalledWith([provisionCommand()]));
	});

	it('dispatches restore against already committed retain topology without a redundant write', async () => {
		const runtime = new TestTopologyRuntime();
		const topologyPort = { commit: vi.fn<TopologyCommit>() };
		const commandPort = {
			dispatch: vi.fn<(commands: readonly TerminalPresentationCommand[]) => void>(),
		};
		const coordinator = new DesktopWorkbenchTopologyCoordinator(runtime, topologyPort, commandPort);

		await coordinator.start(
			{ document: topology(workbenchTopologyRevision('7')), kind: 'available' },
			[restoreCommand()],
		);

		await vi.waitFor(() => expect(commandPort.dispatch).toHaveBeenCalledWith([restoreCommand()]));
		expect(topologyPort.commit).not.toHaveBeenCalled();
	});

	it('flushes only after the current topology is durable and its prepared commands are released', async () => {
		const runtime = new TestTopologyRuntime();
		const commit = deferred<{
			readonly kind: 'committed';
			readonly revision: WorkbenchTopologyRevision;
		}>();
		const topologyPort = { commit: vi.fn<TopologyCommit>(() => commit.promise) };
		const commandPort = {
			dispatch: vi.fn<(commands: readonly TerminalPresentationCommand[]) => void>(),
		};
		const coordinator = new DesktopWorkbenchTopologyCoordinator(runtime, topologyPort, commandPort);
		await coordinator.start({
			document: topology(workbenchTopologyRevision('7')),
			kind: 'available',
		});
		runtime.change(2);
		coordinator.dispatchCommands([closeCommand()]);
		const flushed = coordinator.flush();
		let settled = false;
		void flushed.then(() => {
			settled = true;
		});
		await vi.waitFor(() => expect(topologyPort.commit).toHaveBeenCalledOnce());
		expect(settled).toBe(false);
		expect(commandPort.dispatch).not.toHaveBeenCalled();

		commit.resolve({ kind: 'committed', revision: workbenchTopologyRevision('8') });
		await expect(flushed).resolves.toEqual({ kind: 'flushed', revision: '8' });
		expect(commandPort.dispatch).toHaveBeenCalledWith([closeCommand()]);
	});

	it('binds each command batch to the complete snapshot captured before its commit', async () => {
		const runtime = new TestTopologyRuntime();
		const first = deferred<{
			readonly kind: 'committed';
			readonly revision: WorkbenchTopologyRevision;
		}>();
		const second = deferred<{
			readonly kind: 'committed';
			readonly revision: WorkbenchTopologyRevision;
		}>();
		const topologyPort = {
			commit: vi
				.fn<TopologyCommit>()
				.mockReturnValueOnce(first.promise)
				.mockReturnValueOnce(second.promise),
		};
		const commandPort = {
			dispatch: vi.fn<(commands: readonly TerminalPresentationCommand[]) => void>(),
		};
		const coordinator = new DesktopWorkbenchTopologyCoordinator(runtime, topologyPort, commandPort);
		await coordinator.start({
			document: topology(workbenchTopologyRevision('7')),
			kind: 'available',
		});

		runtime.change(2);
		coordinator.dispatchCommands([provisionCommand()]);
		await vi.waitFor(() => expect(topologyPort.commit).toHaveBeenCalledTimes(1));
		runtime.change(3);
		coordinator.dispatchCommands([closeCommand()]);
		first.resolve({ kind: 'committed', revision: workbenchTopologyRevision('8') });

		await vi.waitFor(() => {
			expect(commandPort.dispatch).toHaveBeenNthCalledWith(1, [provisionCommand()]);
			expect(topologyPort.commit).toHaveBeenCalledTimes(2);
		});
		expect(topologyPort.commit).toHaveBeenNthCalledWith(
			2,
			workbenchTopologyRevision('8'),
			topology(workbenchTopologyRevision('9'), 3),
		);
		expect(commandPort.dispatch).toHaveBeenCalledTimes(1);
		second.resolve({ kind: 'committed', revision: workbenchTopologyRevision('9') });
		await vi.waitFor(() =>
			expect(commandPort.dispatch).toHaveBeenNthCalledWith(2, [closeCommand()]),
		);
	});

	it('fails closed after CAS failure and never releases the affected command', async () => {
		const runtime = new TestTopologyRuntime();
		const topologyPort = {
			commit: vi
				.fn<TopologyCommit>()
				.mockResolvedValue({ kind: 'unavailable', reason: 'write-failed' }),
		};
		const commandPort = {
			dispatch: vi.fn<(commands: readonly TerminalPresentationCommand[]) => void>(),
		};
		const coordinator = new DesktopWorkbenchTopologyCoordinator(runtime, topologyPort, commandPort);
		await coordinator.start({
			document: topology(workbenchTopologyRevision('7')),
			kind: 'available',
		});

		runtime.change(2);
		coordinator.dispatchCommands([provisionCommand()]);

		await vi.waitFor(() => expect(coordinator.failureReason).toBe('write-failed'));
		expect(commandPort.dispatch).not.toHaveBeenCalled();
		runtime.change(3);
		coordinator.dispatchCommands([closeCommand()]);
		await Promise.resolve();
		expect(topologyPort.commit).toHaveBeenCalledOnce();
		expect(commandPort.dispatch).not.toHaveBeenCalled();
	});

	it('persists managed close intent before the effect and commits container cleanup afterward', async () => {
		let operationSequence = 1;
		const bufferedCommands: TerminalPresentationCommand[] = [];
		let coordinator: DesktopWorkbenchTopologyCoordinator | undefined;
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
		const committedDocuments: WorkbenchTopologyDocument[] = [];
		const dispatchedCommands: TerminalPresentationCommand[] = [];
		const topologyPort = {
			commit: vi.fn<TopologyCommit>(
				async (
					_expected: WorkbenchTopologyRevision | null,
					document: WorkbenchTopologyDocument,
				) => {
					committedDocuments.push(document);
					timeline.push(`commit:${intentIn(document)}`);
					return { kind: 'committed' as const, revision: document.revision };
				},
			),
		};
		const commandPort = {
			dispatch: (commands: readonly TerminalPresentationCommand[]) => {
				dispatchedCommands.push(...commands);
				for (const command of commands) timeline.push(`dispatch:${command.type}`);
			},
		};
		coordinator = new DesktopWorkbenchTopologyCoordinator(runtime, topologyPort, commandPort);

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

function intentIn(document: WorkbenchTopologyDocument | undefined): string {
	const root = document?.terminalContexts[0]?.panelWorkspace.tabs[0]?.root;
	if (root?.kind !== 'panel') return 'absent';
	return root.primaryPresentation.intent.kind;
}
