import {
	createEmptyWorkbenchSharedTerminalContext,
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
	type WorkbenchSharedPresentation,
	type WorkbenchSharedPresentationIntent,
} from '@orchbee/workbench/document';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	type TerminalPresentationCommand,
	type TerminalPresentationEvent,
} from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import { DesktopTerminalIntentReconciler } from '../DesktopTerminalIntentReconciler';

describe('DesktopTerminalIntentReconciler', () => {
	it('executes one durable operation once and caches its exact publication', async () => {
		const execute = vi
			.fn<(command: TerminalPresentationCommand) => Promise<TerminalPresentationEvent>>()
			.mockImplementation(async (command) => ({
				operationId: command.operationId,
				presentationId: command.presentationId,
				type: 'terminal-provision-completed',
			}));
		const reconciler = reconcilerWith(execute);
		const candidate = document('0', provisionIntent('operation-1'));
		const published: TerminalPresentationEvent[] = [];

		expect(reconciler.validateDocument(candidate)).toBe(true);
		await reconciler.reconcileDocument(candidate, (event) => published.push(event));
		await reconciler.reconcileDocument(candidate, (event) => published.push(event));

		expect(execute).toHaveBeenCalledOnce();
		expect(published).toEqual([
			{
				operationId: 'operation-1',
				presentationId: 'presentation-1',
				type: 'terminal-provision-completed',
			},
		]);
		expect(reconciler.getTerminalEvents()).toEqual(published);
	});

	it('starts independent presentation effects without waiting for earlier completions', async () => {
		const primaryResult = createDeferred<TerminalPresentationEvent>();
		const childResult = createDeferred<TerminalPresentationEvent>();
		const execute = vi.fn<
			(command: TerminalPresentationCommand) => Promise<TerminalPresentationEvent>
		>((command) =>
			command.presentationId === terminalPresentationId('presentation-1')
				? primaryResult.promise
				: childResult.promise,
		);
		const reconciler = reconcilerWith(execute);
		const published: TerminalPresentationEvent[] = [];
		const reconciliation = reconciler.reconcileDocument(
			document('0', provisionIntent('operation-1'), {
				child: presentation('presentation-2', provisionIntent('operation-2'), '/workspace'),
			}),
			(event) => published.push(event),
		);

		expect(execute.mock.calls.map(([command]) => command.presentationId)).toEqual([
			'presentation-1',
			'presentation-2',
		]);
		childResult.resolve({
			operationId: terminalPresentationOperationId('operation-2'),
			presentationId: terminalPresentationId('presentation-2'),
			type: 'terminal-provision-completed',
		});
		await vi.waitFor(() => expect(published).toHaveLength(1));
		primaryResult.resolve({
			operationId: terminalPresentationOperationId('operation-1'),
			presentationId: terminalPresentationId('presentation-1'),
			type: 'terminal-provision-completed',
		});
		await reconciliation;

		expect(published.map((event) => event.presentationId)).toEqual([
			'presentation-2',
			'presentation-1',
		]);
	});

	it('rejects operation identity reuse with different durable content', async () => {
		const execute = vi
			.fn<(command: TerminalPresentationCommand) => Promise<TerminalPresentationEvent>>()
			.mockImplementation(async (command) => ({
				operationId: command.operationId,
				presentationId: command.presentationId,
				type: 'terminal-provision-completed',
			}));
		const reconciler = reconcilerWith(execute);
		const initial = document('0', provisionIntent('same-operation'));
		await reconciler.reconcileDocument(initial, () => undefined);

		expect(
			reconciler.validateDocument(
				document('1', provisionIntent('same-operation'), { launchCwd: '/different' }),
			),
		).toBe(false);
		expect(
			reconciler.validateDocument(
				document('1', provisionIntent('same-operation'), {
					child: presentation('presentation-2', provisionIntent('same-operation'), '/workspace'),
				}),
			),
		).toBe(false);
	});

	it('publishes restore start before the matching result and never restores retain twice', async () => {
		const execute = vi
			.fn<(command: TerminalPresentationCommand) => Promise<TerminalPresentationEvent>>()
			.mockImplementation(async (command) => ({
				operationId: command.operationId,
				presentationId: command.presentationId,
				type: 'terminal-restore-completed',
			}));
		const reconciler = reconcilerWith(execute);
		const retained = document('4', { kind: 'retain' });
		const published: TerminalPresentationEvent[] = [];

		await reconciler.reconcileDocument(retained, (event) => published.push(event));
		await reconciler.reconcileDocument(retained, (event) => published.push(event));

		expect(execute).toHaveBeenCalledOnce();
		expect(published.map((event) => event.type)).toEqual([
			'terminal-restore-started',
			'terminal-restore-completed',
		]);
		expect(reconciler.getTerminalEvents()).toEqual(published);
	});

	it('projects a completed provision into retain bootstrap without executing a redundant restore', async () => {
		const execute = vi
			.fn<(command: TerminalPresentationCommand) => Promise<TerminalPresentationEvent>>()
			.mockImplementation(async (command) => ({
				operationId: command.operationId,
				presentationId: command.presentationId,
				type: 'terminal-provision-completed',
			}));
		const reconciler = reconcilerWith(execute);
		await reconciler.reconcileDocument(
			document('0', provisionIntent('provision-operation')),
			() => undefined,
		);
		const published: TerminalPresentationEvent[] = [];

		await reconciler.reconcileDocument(document('1', { kind: 'retain' }), (event) =>
			published.push(event),
		);

		expect(execute).toHaveBeenCalledOnce();
		expect(published).toEqual([]);
		expect(reconciler.getTerminalEvents().map((event) => event.type)).toEqual([
			'terminal-restore-started',
			'terminal-restore-completed',
		]);
	});

	it('maps a mismatched runtime result to one correlated non-retryable failure', async () => {
		const execute = vi
			.fn<(command: TerminalPresentationCommand) => Promise<TerminalPresentationEvent>>()
			.mockResolvedValue({
				exitCode: 0,
				presentationId: terminalPresentationId('presentation-1'),
				signal: null,
				type: 'terminal-exited',
			});
		const reconciler = reconcilerWith(execute);
		const published: TerminalPresentationEvent[] = [];

		await reconciler.reconcileDocument(document('0', provisionIntent('operation-1')), (event) =>
			published.push(event),
		);

		expect(published).toEqual([
			{
				failure: { code: 'terminal-effect-result-mismatch', retryable: false },
				operationId: 'operation-1',
				presentationId: 'presentation-1',
				type: 'terminal-provision-failed',
			},
		]);
	});

	it('records Host outcomes only for presentations in the current canonical document', async () => {
		const reconciler = reconcilerWith(async (command) => ({
			operationId: command.operationId,
			presentationId: command.presentationId,
			type: 'terminal-restore-completed',
		}));
		const knownExit = {
			exitCode: 0,
			presentationId: terminalPresentationId('presentation-1'),
			signal: null,
			type: 'terminal-exited',
		} as const;
		const unknownExit = {
			...knownExit,
			presentationId: terminalPresentationId('presentation-unreferenced'),
		};

		expect(reconciler.recordExternalEvent(unknownExit)).toBe(false);
		await reconciler.reconcileDocument(document('4', { kind: 'retain' }), () => undefined);
		expect(reconciler.reserveExternalEvent(knownExit)).toBe(true);
		expect(reconciler.getTerminalEvents()).not.toContainEqual(knownExit);
		expect(reconciler.commitExternalEvent(knownExit)).toBe(true);
		expect(reconciler.recordExternalEvent(knownExit)).toBe(false);
		expect(reconciler.recordExternalEvent(unknownExit)).toBe(false);
		expect(reconciler.getTerminalEvents()).toContainEqual(knownExit);
		const conflictingFailure = {
			failure: { code: 'terminal-runtime-pty-read-read-output', retryable: false },
			presentationId: knownExit.presentationId,
			type: 'terminal-failed',
		} as const;
		expect(reconciler.recordExternalEvent(conflictingFailure)).toBe(false);
	});

	it('publishes a recovered runtime failure immediately after restore completes', async () => {
		const failure = {
			failure: { code: 'terminal-runtime-pty-write-write-input', retryable: false },
			presentationId: terminalPresentationId('presentation-1'),
			type: 'terminal-failed',
		} as const;
		const reconciler = reconcilerWith(
			async (command) => ({
				operationId: command.operationId,
				presentationId: command.presentationId,
				type: 'terminal-restore-completed',
			}),
			() => failure,
		);
		const published: TerminalPresentationEvent[] = [];

		await reconciler.reconcileDocument(document('5', { kind: 'retain' }), (event) =>
			published.push(event),
		);

		expect(published.map((event) => event.type)).toEqual([
			'terminal-restore-started',
			'terminal-restore-completed',
			'terminal-failed',
		]);
		expect(reconciler.getTerminalEvents()).toContainEqual(failure);
	});

	it('routes a remote Workspace intent only to its durable Environment target', async () => {
		const execute = vi
			.fn<ConstructorParameters<typeof DesktopTerminalIntentReconciler>[0]['execute']>()
			.mockImplementation(async (command) => ({
				event: {
					operationId: command.operationId,
					presentationId: command.presentationId,
					type: 'terminal-provision-completed',
				},
				kind: 'settled',
			}));
		const reconciler = new DesktopTerminalIntentReconciler({
			createOperationId: () => terminalPresentationOperationId('restore-operation'),
			execute,
			getTerminalOutcome: () => null,
		});

		await reconciler.reconcileDocument(
			document('0', provisionIntent('remote-operation'), {
				targetEnvironmentId: 'environment:00000000000000000000000000000001',
			}),
			() => undefined,
		);

		expect(execute).toHaveBeenCalledWith(expect.anything(), {
			environmentId: 'environment:00000000000000000000000000000001',
			kind: 'ssh-environment',
		});
	});

	it('keeps an unavailable remote intent pending until an explicit reconciliation settles it', async () => {
		const execute = vi
			.fn<ConstructorParameters<typeof DesktopTerminalIntentReconciler>[0]['execute']>()
			.mockResolvedValueOnce({ kind: 'deferred' })
			.mockImplementationOnce(async (command) => ({
				event: {
					operationId: command.operationId,
					presentationId: command.presentationId,
					type: 'terminal-provision-completed',
				},
				kind: 'settled',
			}));
		const reconciler = new DesktopTerminalIntentReconciler({
			createOperationId: () => terminalPresentationOperationId('restore-operation'),
			execute,
			getTerminalOutcome: () => null,
		});
		const remote = document('0', provisionIntent('remote-operation'), {
			targetEnvironmentId: 'environment:00000000000000000000000000000001',
		});
		const published: TerminalPresentationEvent[] = [];

		await reconciler.reconcileDocument(remote, (event) => published.push(event));
		expect(published).toEqual([]);
		expect(reconciler.getTerminalEvents()).toEqual([]);

		await reconciler.reconcileDocument(remote, (event) => published.push(event));
		expect(execute).toHaveBeenCalledTimes(2);
		expect(published.map((event) => event.type)).toEqual(['terminal-provision-completed']);
	});

	it('rejects reuse of a durable operation identity after its Environment target changes', async () => {
		const execute = vi
			.fn<ConstructorParameters<typeof DesktopTerminalIntentReconciler>[0]['execute']>()
			.mockImplementation(async (command) => ({
				event: {
					operationId: command.operationId,
					presentationId: command.presentationId,
					type: 'terminal-provision-completed',
				},
				kind: 'settled',
			}));
		const reconciler = new DesktopTerminalIntentReconciler({
			createOperationId: () => terminalPresentationOperationId('restore-operation'),
			execute,
			getTerminalOutcome: () => null,
		});
		await reconciler.reconcileDocument(
			document('0', provisionIntent('same-operation'), {
				targetEnvironmentId: 'environment:00000000000000000000000000000001',
			}),
			() => undefined,
		);

		expect(
			reconciler.validateDocument(
				document('1', provisionIntent('same-operation'), {
					targetEnvironmentId: 'environment:00000000000000000000000000000002',
				}),
			),
		).toBe(false);
	});
});

function reconcilerWith(
	execute: (command: TerminalPresentationCommand) => Promise<TerminalPresentationEvent>,
	getTerminalOutcome: ConstructorParameters<
		typeof DesktopTerminalIntentReconciler
	>[0]['getTerminalOutcome'] = () => null,
): DesktopTerminalIntentReconciler {
	let operationSequence = 0;
	return new DesktopTerminalIntentReconciler({
		createOperationId: () => terminalPresentationOperationId(`main:restore:${++operationSequence}`),
		execute: async (command) => ({ event: await execute(command), kind: 'settled' }),
		getTerminalOutcome: (presentationId) => getTerminalOutcome(presentationId, { kind: 'local' }),
	});
}

function provisionIntent(operationId: string): WorkbenchSharedPresentationIntent {
	return {
		kind: 'provision',
		operationId: terminalPresentationOperationId(operationId),
	};
}

function presentation(
	presentationId: string,
	intent: WorkbenchSharedPresentationIntent,
	launchCwd: string,
): WorkbenchSharedPresentation {
	return {
		intent,
		launchCwd,
		presentationId: terminalPresentationId(presentationId),
		title: presentationId,
	};
}

function document(
	revision: string,
	intent: WorkbenchSharedPresentationIntent,
	options: {
		readonly child?: WorkbenchSharedPresentation;
		readonly launchCwd?: string;
		readonly targetEnvironmentId?: string;
	} = {},
): WorkbenchSharedDocument {
	const remote = options.targetEnvironmentId !== undefined;
	return createWorkbenchSharedDocument({
		projectWorkspace: {
			nextProjectSequence: 1,
			nextTerminalContextSequence: remote ? 3 : 2,
			nextWorkspaceSequence: remote ? 2 : 1,
			projects: remote
				? [
						{
							directories: [
								{
									available: false,
									git: null,
									id: 'remote-directory-1',
									kind: 'folder',
									name: 'workspace',
									path: '/workspace',
									placement: {
										environmentId: options.targetEnvironmentId,
										kind: 'environment',
									},
									source: 'selected',
								},
							],
							id: 'project-1',
							launchConfigurations: [],
							name: 'workspace',
							primaryDirectoryId: 'remote-directory-1',
							workspaces: [
								{
									id: 'workspace-1',
									launchConfigurationId: null,
									name: 'workspace',
									primaryDirectoryId: 'remote-directory-1',
									terminalContextId: 'terminal-context-1',
								},
							],
						},
					]
				: [],
			standaloneTerminalContextId: remote ? 'terminal-context-2' : 'terminal-context-1',
		},
		revision: workbenchSharedDocumentRevision(revision),
		terminalContexts: [
			{
				nextTabSequence: 2,
				panelWorkspace: {
					nextPanelSequence: 2,
					nextPanelSlotSequence: 1,
					nextSplitSequence: 1,
					nextTerminalSequence: 3,
					pendingRemovals: [],
					tabs: [
						{
							root: {
								childPresentations: options.child === undefined ? [] : [options.child],
								id: 'panel-1',
								kind: 'panel',
								primaryPresentation: presentation(
									'presentation-1',
									intent,
									options.launchCwd ?? '/workspace',
								),
							},
							tabId: 'tab-1',
						},
					],
				},
				tabCollection: {
					groups: [],
					tabs: [{ customTitle: null, group: null, id: 'tab-1', pinned: false, title: 'Terminal' }],
				},
				terminalContextId: 'terminal-context-1',
			},
			...(remote ? [createEmptyWorkbenchSharedTerminalContext('terminal-context-2')] : []),
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
