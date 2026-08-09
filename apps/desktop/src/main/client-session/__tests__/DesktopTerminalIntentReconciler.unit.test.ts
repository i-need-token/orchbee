import {
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
		execute,
		getTerminalOutcome,
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
					tabs: [{ group: null, id: 'tab-1', pinned: false, title: 'Terminal' }],
				},
				terminalContextId: 'terminal-context-1',
			},
		],
	});
}
