import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '../../../terminal-presentation';
import { workbenchTopologyRevision } from '../../topology';
import { describe, expect, it } from 'vitest';

import type { WorkbenchTerminalPresentationCommand } from '../createWorkbenchPreviewTerminalPresentationComposition';
import { WorkbenchProjectWorkspaceRuntime } from '../WorkbenchProjectWorkspaceRuntime';
import { hydrateWorkbenchTopology } from '../workbench-topology-runtime';

function managedComposition(commands: WorkbenchTerminalPresentationCommand[]) {
	let operationSequence = 1;
	let presentationSequence = 1;
	let removalSequence = 1;
	return {
		createOperationId: () =>
			terminalPresentationOperationId(`topology:operation:${operationSequence++}`),
		createPresentationId: () =>
			terminalPresentationId(`topology:presentation:${presentationSequence++}`),
		createRemovalId: () => terminalPresentationRemovalId(`topology:removal:${removalSequence++}`),
		dispatchCommands: (nextCommands: readonly WorkbenchTerminalPresentationCommand[]) => {
			commands.push(...nextCommands);
		},
		initialPresentationId: terminalPresentationId('topology:presentation:initial'),
		kind: 'managed' as const,
	};
}

function createManagedRuntime(commands: WorkbenchTerminalPresentationCommand[]) {
	return new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/workspace',
		standaloneTerminalTitle: 'Terminal 1',
		terminalPresentation: managedComposition(commands),
	});
}

describe('workbench topology runtime', () => {
	it('projects and hydrates managed topology without dispatching recovered effects', () => {
		const commands: WorkbenchTerminalPresentationCommand[] = [];
		const runtime = createManagedRuntime(commands);
		const provision = commands[0];
		if (provision?.type !== 'terminal-provision-requested') {
			throw new Error('Expected the initial provision command.');
		}
		runtime.applyTerminalPresentationEvent({
			operationId: provision.operationId,
			presentationId: provision.presentationId,
			type: 'terminal-provision-completed',
		});
		const context = runtime.getActiveTerminalContext();
		const tabId = context.tabModel.getSnapshot().activeTabId;
		if (tabId === null) throw new Error('Expected an active Tab.');
		context.panelWorkspaceStore.transition({
			currentCwd: '/workspace/changed',
			presentationId: provision.presentationId,
			tabId,
			type: 'terminal-current-cwd-changed',
		});
		const document = runtime.getTopologyDocument(workbenchTopologyRevision('7'));
		const hydrated = hydrateWorkbenchTopology(document);
		const restoredCommands: WorkbenchTerminalPresentationCommand[] = [];
		const restored = new WorkbenchProjectWorkspaceRuntime({
			initialTopology: document,
			standaloneCwd: '/fallback',
			standaloneTerminalTitle: 'Unused',
			terminalPresentation: managedComposition(restoredCommands),
		});

		expect(restoredCommands).toEqual([]);
		expect(restored.getTopologyDocument(workbenchTopologyRevision('7'))).toEqual(document);
		expect(hydrated.terminalContexts[0]?.panelWorkspace.tabs[0]?.root).toMatchObject({
			primaryPresentation: {
				currentCwd: null,
				launchCwd: '/workspace',
				lifecycle: { kind: 'restoring' },
			},
		});
	});

	it('preserves a durable close intent and its exact pending removal', () => {
		const commands: WorkbenchTerminalPresentationCommand[] = [];
		const runtime = createManagedRuntime(commands);
		const provision = commands[0];
		if (provision?.type !== 'terminal-provision-requested') {
			throw new Error('Expected the initial provision command.');
		}
		runtime.applyTerminalPresentationEvent({
			operationId: provision.operationId,
			presentationId: provision.presentationId,
			type: 'terminal-provision-completed',
		});
		const tabId = runtime.getActiveTerminalContext().tabModel.getSnapshot().activeTabId;
		if (tabId === null) throw new Error('Expected an active Tab.');
		runtime.requestActiveTerminalTabsClose([tabId]);

		const document = runtime.getTopologyDocument(workbenchTopologyRevision('8'));
		const pendingRemoval = document.terminalContexts[0]?.panelWorkspace.pendingRemovals[0];
		const presentation = document.terminalContexts[0]?.panelWorkspace.tabs[0]?.root;

		expect(pendingRemoval).toMatchObject({
			kind: 'tab',
			presentationIds: [provision.presentationId],
		});
		expect(presentation).toMatchObject({
			primaryPresentation: {
				intent: {
					kind: 'close',
					removalId: pendingRemoval?.removalId,
				},
			},
		});
		const restoredCommands: WorkbenchTerminalPresentationCommand[] = [];
		const restored = new WorkbenchProjectWorkspaceRuntime({
			initialTopology: document,
			standaloneCwd: '/fallback',
			standaloneTerminalTitle: 'Unused',
			terminalPresentation: managedComposition(restoredCommands),
		});
		expect(restored.getTopologyDocument(workbenchTopologyRevision('8'))).toEqual(document);
		expect(restored.reconcileDurableTerminalPresentations()).toEqual({
			commandCount: 1,
			kind: 'started',
		});
		expect(restoredCommands).toContainEqual({
			operationId: expect.any(String),
			presentationId: provision.presentationId,
			type: 'terminal-close-requested',
		});
	});

	it('replays a persisted provision intent with its original operation identity', () => {
		const initialCommands: WorkbenchTerminalPresentationCommand[] = [];
		const initial = createManagedRuntime(initialCommands);
		const provision = initialCommands[0];
		if (provision?.type !== 'terminal-provision-requested') {
			throw new Error('Expected the initial provision command.');
		}
		const document = initial.getTopologyDocument(workbenchTopologyRevision('3'));
		const replayedCommands: WorkbenchTerminalPresentationCommand[] = [];
		const replayed = new WorkbenchProjectWorkspaceRuntime({
			initialTopology: document,
			standaloneCwd: '/fallback',
			standaloneTerminalTitle: 'Unused',
			terminalPresentation: managedComposition(replayedCommands),
		});

		replayed.reconcileDurableTerminalPresentations();

		expect(replayedCommands).toEqual([provision]);
		expect(replayed.getTopologyDocument(workbenchTopologyRevision('3'))).toEqual(document);
	});

	it('reconciles retain intent through restore-only commands without changing durable intent', () => {
		const initialCommands: WorkbenchTerminalPresentationCommand[] = [];
		const initial = createManagedRuntime(initialCommands);
		const provision = initialCommands[0];
		if (provision?.type !== 'terminal-provision-requested') {
			throw new Error('Expected the initial provision command.');
		}
		initial.applyTerminalPresentationEvent({
			operationId: provision.operationId,
			presentationId: provision.presentationId,
			type: 'terminal-provision-completed',
		});
		const document = initial.getTopologyDocument(workbenchTopologyRevision('4'));
		const restoredCommands: WorkbenchTerminalPresentationCommand[] = [];
		const restored = new WorkbenchProjectWorkspaceRuntime({
			initialTopology: document,
			standaloneCwd: '/fallback',
			standaloneTerminalTitle: 'Unused',
			terminalPresentation: managedComposition(restoredCommands),
		});

		expect(restored.reconcileDurableTerminalPresentations()).toEqual({
			commandCount: 1,
			kind: 'started',
		});
		expect(restoredCommands).toEqual([
			{
				launchCwd: '/workspace',
				operationId: 'topology:operation:1',
				presentationId: provision.presentationId,
				type: 'terminal-restore-requested',
			},
		]);
		expect(restored.getTopologyDocument(workbenchTopologyRevision('4'))).toEqual(document);
		expect(
			restored.applyTerminalPresentationEvent({
				operationId: terminalPresentationOperationId('topology:operation:1'),
				presentationId: provision.presentationId,
				type: 'terminal-restore-completed',
			}),
		).toEqual({ kind: 'applied' });
		expect(restored.getTopologyDocument(workbenchTopologyRevision('5'))).toEqual({
			...document,
			revision: '5',
		});
		expect(restored.reconcileDurableTerminalPresentations()).toEqual({
			commandCount: 0,
			kind: 'unchanged',
		});
	});

	it('keeps failed restoration as retain intent instead of replacement provision intent', () => {
		const initialCommands: WorkbenchTerminalPresentationCommand[] = [];
		const initial = createManagedRuntime(initialCommands);
		const provision = initialCommands[0];
		if (provision?.type !== 'terminal-provision-requested') {
			throw new Error('Expected the initial provision command.');
		}
		initial.applyTerminalPresentationEvent({
			operationId: provision.operationId,
			presentationId: provision.presentationId,
			type: 'terminal-provision-completed',
		});
		const document = initial.getTopologyDocument(workbenchTopologyRevision('2'));
		const restored = new WorkbenchProjectWorkspaceRuntime({
			initialTopology: document,
			standaloneCwd: '/fallback',
			standaloneTerminalTitle: 'Unused',
			terminalPresentation: managedComposition([]),
		});
		restored.reconcileDurableTerminalPresentations();

		expect(
			restored.applyTerminalPresentationEvent({
				failure: { code: 'terminal-identity-unavailable', retryable: false },
				operationId: terminalPresentationOperationId('topology:operation:1'),
				presentationId: provision.presentationId,
				type: 'terminal-restore-failed',
			}),
		).toEqual({ kind: 'applied' });
		expect(restored.getTopologyDocument(workbenchTopologyRevision('2'))).toEqual(document);
	});

	it('refuses to project preview-only terminal presentations as durable process intent', () => {
		const runtime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/workspace',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: {
				createPresentationId: () => terminalPresentationId('preview:next'),
				initialPresentationId: terminalPresentationId('preview:initial'),
				kind: 'preview',
			},
		});

		expect(() => runtime.getTopologyDocument(workbenchTopologyRevision('0'))).toThrow(
			'Preview terminal presentations cannot enter durable topology.',
		);
	});
});
