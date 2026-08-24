import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '../../../terminal-presentation';
import { workbenchSharedDocumentRevision } from '../../document';
import { describe, expect, it } from 'vitest';

import type { WorkbenchTerminalPresentationCommand } from '../createWorkbenchPreviewTerminalPresentationComposition';
import { WorkbenchProjectWorkspaceRuntime } from '../WorkbenchProjectWorkspaceRuntime';
import { hydrateWorkbenchDocuments } from '../workbench-document-runtime';

function managedComposition(commands: WorkbenchTerminalPresentationCommand[]) {
	let operationSequence = 1;
	let presentationSequence = 1;
	let removalSequence = 1;
	return {
		createOperationId: () =>
			terminalPresentationOperationId(`document:operation:${operationSequence++}`),
		createPresentationId: () =>
			terminalPresentationId(`document:presentation:${presentationSequence++}`),
		createRemovalId: () => terminalPresentationRemovalId(`document:removal:${removalSequence++}`),
		dispatchCommands: (nextCommands: readonly WorkbenchTerminalPresentationCommand[]) => {
			commands.push(...nextCommands);
		},
		initialPresentationId: terminalPresentationId('document:presentation:initial'),
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

describe('Workbench document runtime', () => {
	it('applies a shared document without effects and reconciles the current Client View', () => {
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
		const initialDocument = runtime.getSharedDocument(workbenchSharedDocumentRevision('0'));
		runtime.createTerminal({ groupId: null, source: 'button', title: 'Terminal 2' });
		expect(runtime.getClientView().terminalContexts[0]?.tabCollection.activeTabId).toBe(
			'terminal-context-1-tab-2',
		);
		const commandCount = commands.length;

		const result = runtime.applySharedDocument(initialDocument);

		expect(result).toMatchObject({ kind: 'applied' });
		expect(runtime.getClientView().terminalContexts[0]?.tabCollection.activeTabId).toBe(
			'terminal-context-1-tab-1',
		);
		expect(runtime.getSharedDocument(workbenchSharedDocumentRevision('0'))).toEqual(
			initialDocument,
		);
		expect(
			runtime.getActiveTerminalContext().panelWorkspaceStore.getSnapshot().tabs[0]?.root,
		).toMatchObject({ primaryPresentation: { lifecycle: { kind: 'running' } } });
		expect(commands).toHaveLength(commandCount);
	});

	it('preserves live Panel focus history across shared-document application without serializing it', () => {
		const commands: WorkbenchTerminalPresentationCommand[] = [];
		const runtime = createManagedRuntime(commands);
		const initialProvision = commands[0];
		if (initialProvision?.type !== 'terminal-provision-requested') {
			throw new Error('Expected the initial provision command.');
		}
		runtime.applyTerminalPresentationEvent({
			operationId: initialProvision.operationId,
			presentationId: initialProvision.presentationId,
			type: 'terminal-provision-completed',
		});
		const context = runtime.getActiveTerminalContext();
		const initialTab = context.panelWorkspaceStore.getSnapshot().tabs[0];
		if (initialTab === undefined) throw new Error('Expected an initial Tab.');
		for (const panelId of ['panel-1', 'panel-2']) {
			const presentation = context.panelWorkspaceStore.createTerminalPresentation();
			if (presentation.kind !== 'managed') throw new Error('Expected a managed presentation.');
			context.panelWorkspaceStore.transition({
				direction: 'right',
				newTerminalTitle: `Terminal ${panelId === 'panel-1' ? '2' : '3'}`,
				panelId,
				presentation,
				tabId: initialTab.tabId,
				type: 'panel-split',
			});
			runtime.applyTerminalPresentationEvent({
				operationId: presentation.operationId,
				presentationId: presentation.presentationId,
				type: 'terminal-provision-completed',
			});
		}
		for (const panelId of ['panel-1', 'panel-3', 'panel-2']) {
			context.panelWorkspaceStore.transition({
				panelId,
				tabId: initialTab.tabId,
				type: 'panel-focused',
			});
		}
		const clientView = runtime.getClientView();
		const document = runtime.getSharedDocument(workbenchSharedDocumentRevision('11'));

		expect(context.panelWorkspaceStore.getSnapshot().tabs[0]?.panelFocusHistory).toEqual([
			'panel-1',
			'panel-3',
			'panel-2',
		]);
		expect(JSON.stringify(document)).not.toContain('panelFocusHistory');
		expect(
			hydrateWorkbenchDocuments(document, clientView).terminalContexts[0]?.panelWorkspace.tabs[0]
				?.panelFocusHistory,
		).toEqual(['panel-2']);

		runtime.createTerminal({ groupId: null, source: 'button', title: 'Terminal 4' });
		expect(runtime.applySharedDocument(document)).toMatchObject({ kind: 'applied' });
		expect(
			runtime.getActiveTerminalContext().panelWorkspaceStore.getSnapshot().tabs[0]
				?.panelFocusHistory,
		).toEqual(['panel-1', 'panel-3', 'panel-2']);
	});

	it('projects and hydrates managed shared document without dispatching recovered effects', () => {
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
		const document = runtime.getSharedDocument(workbenchSharedDocumentRevision('7'));
		const hydrated = hydrateWorkbenchDocuments(document);
		const restoredCommands: WorkbenchTerminalPresentationCommand[] = [];
		const restored = new WorkbenchProjectWorkspaceRuntime({
			initialDocument: document,
			standaloneCwd: '/fallback',
			standaloneTerminalTitle: 'Unused',
			terminalPresentation: managedComposition(restoredCommands),
		});

		expect(restoredCommands).toEqual([]);
		expect(restored.getSharedDocument(workbenchSharedDocumentRevision('7'))).toEqual(document);
		expect(hydrated.terminalContexts[0]?.panelWorkspace.tabs[0]?.root).toMatchObject({
			primaryPresentation: {
				currentCwd: null,
				launchCwd: '/workspace',
				lifecycle: { kind: 'restoring' },
			},
		});

		const restoreOperationId = terminalPresentationOperationId('main:restore:1');
		expect(
			restored.applyTerminalPresentationEvent({
				operationId: restoreOperationId,
				presentationId: provision.presentationId,
				type: 'terminal-restore-started',
			}),
		).toEqual({ kind: 'applied' });
		expect(restoredCommands).toEqual([]);
		expect(
			restored.applyTerminalPresentationEvent({
				operationId: restoreOperationId,
				presentationId: provision.presentationId,
				type: 'terminal-restore-completed',
			}),
		).toEqual({ kind: 'applied' });
		expect(
			restored.getActiveTerminalContext().panelWorkspaceStore.getSnapshot().tabs[0]?.root,
		).toMatchObject({ primaryPresentation: { lifecycle: { kind: 'running' } } });
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

		const document = runtime.getSharedDocument(workbenchSharedDocumentRevision('8'));
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
			initialDocument: document,
			standaloneCwd: '/fallback',
			standaloneTerminalTitle: 'Unused',
			terminalPresentation: managedComposition(restoredCommands),
		});
		expect(restored.getSharedDocument(workbenchSharedDocumentRevision('8'))).toEqual(document);
		expect(restored.reconcileDurableTerminalPresentations()).toEqual({
			commandCount: 1,
			kind: 'started',
		});
		expect(restoredCommands).toContainEqual({
			disposition: 'remove',
			operationId: expect.any(String),
			presentationId: provision.presentationId,
			type: 'terminal-close-requested',
		});
	});

	it('persists a primary-only natural-exit removal while retaining child presentations', () => {
		const commands: WorkbenchTerminalPresentationCommand[] = [];
		const runtime = createManagedRuntime(commands);
		const primaryProvision = commands[0];
		if (primaryProvision?.type !== 'terminal-provision-requested') {
			throw new Error('Expected the initial provision command.');
		}
		runtime.applyTerminalPresentationEvent({
			operationId: primaryProvision.operationId,
			presentationId: primaryProvision.presentationId,
			type: 'terminal-provision-completed',
		});
		const context = runtime.getActiveTerminalContext();
		const tab = context.panelWorkspaceStore.getSnapshot().tabs[0];
		if (tab === undefined || tab.root.kind !== 'panel') {
			throw new Error('Expected an initial Panel.');
		}
		const child = context.panelWorkspaceStore.createTerminalPresentation();
		if (child.kind !== 'managed') throw new Error('Expected a managed child presentation.');
		context.panelWorkspaceStore.transition({
			newTerminalTitle: 'Child',
			panelId: tab.root.id,
			presentation: child,
			tabId: tab.tabId,
			type: 'child-terminal-created',
		});
		runtime.applyTerminalPresentationEvent({
			operationId: child.operationId,
			presentationId: child.presentationId,
			type: 'terminal-provision-completed',
		});

		expect(
			runtime.applyTerminalPresentationEvent({
				exitCode: 0,
				presentationId: primaryProvision.presentationId,
				signal: null,
				type: 'terminal-exited',
			}),
		).toEqual({ kind: 'applied' });
		const document = runtime.getSharedDocument(workbenchSharedDocumentRevision('9'));
		const terminalContext = document.terminalContexts[0];
		expect(terminalContext?.panelWorkspace.pendingRemovals).toEqual([
			expect.objectContaining({
				kind: 'primary',
				presentationIds: [primaryProvision.presentationId],
			}),
		]);
		expect(terminalContext?.panelWorkspace.tabs[0]?.root).toMatchObject({
			childPresentations: [
				{
					intent: { kind: 'retain' },
					presentationId: child.presentationId,
				},
			],
			primaryPresentation: {
				intent: { kind: 'close' },
				presentationId: primaryProvision.presentationId,
			},
		});

		const restoredCommands: WorkbenchTerminalPresentationCommand[] = [];
		const restored = new WorkbenchProjectWorkspaceRuntime({
			initialDocument: document,
			standaloneCwd: '/fallback',
			standaloneTerminalTitle: 'Unused',
			terminalPresentation: managedComposition(restoredCommands),
		});
		restored.reconcileDurableTerminalPresentations();
		expect(restoredCommands).toContainEqual({
			disposition: 'remove',
			operationId: expect.any(String),
			presentationId: primaryProvision.presentationId,
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
		const document = initial.getSharedDocument(workbenchSharedDocumentRevision('3'));
		const replayedCommands: WorkbenchTerminalPresentationCommand[] = [];
		const replayed = new WorkbenchProjectWorkspaceRuntime({
			initialDocument: document,
			standaloneCwd: '/fallback',
			standaloneTerminalTitle: 'Unused',
			terminalPresentation: managedComposition(replayedCommands),
		});

		replayed.reconcileDurableTerminalPresentations();

		expect(replayedCommands).toEqual([provision]);
		expect(replayed.getSharedDocument(workbenchSharedDocumentRevision('3'))).toEqual(document);
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
		const document = initial.getSharedDocument(workbenchSharedDocumentRevision('4'));
		const restoredCommands: WorkbenchTerminalPresentationCommand[] = [];
		const restored = new WorkbenchProjectWorkspaceRuntime({
			initialDocument: document,
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
				operationId: 'document:operation:1',
				presentationId: provision.presentationId,
				type: 'terminal-restore-requested',
			},
		]);
		expect(restored.getSharedDocument(workbenchSharedDocumentRevision('4'))).toEqual(document);
		expect(
			restored.applyTerminalPresentationEvent({
				operationId: terminalPresentationOperationId('document:operation:1'),
				presentationId: provision.presentationId,
				type: 'terminal-restore-completed',
			}),
		).toEqual({ kind: 'applied' });
		expect(restored.getSharedDocument(workbenchSharedDocumentRevision('5'))).toEqual({
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
		const document = initial.getSharedDocument(workbenchSharedDocumentRevision('2'));
		const restored = new WorkbenchProjectWorkspaceRuntime({
			initialDocument: document,
			standaloneCwd: '/fallback',
			standaloneTerminalTitle: 'Unused',
			terminalPresentation: managedComposition([]),
		});
		restored.reconcileDurableTerminalPresentations();

		expect(
			restored.applyTerminalPresentationEvent({
				failure: { code: 'terminal-identity-unavailable', retryable: false },
				operationId: terminalPresentationOperationId('document:operation:1'),
				presentationId: provision.presentationId,
				type: 'terminal-restore-failed',
			}),
		).toEqual({ kind: 'applied' });
		expect(restored.getSharedDocument(workbenchSharedDocumentRevision('2'))).toEqual(document);
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

		expect(() => runtime.getSharedDocument(workbenchSharedDocumentRevision('0'))).toThrow(
			'Preview terminal presentations cannot enter a shared document.',
		);
	});
});
