import { describe, expect, it, vi } from 'vitest';

import type { WorkbenchFolderDirectory } from '../../../project-workspace';
import { workbenchSharedDocumentRevision } from '../../document';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '../../../terminal-presentation';
import type { WorkbenchTerminalPresentationCommand } from '../createWorkbenchPreviewTerminalPresentationComposition';
import { createWorkbenchPreviewTerminalPresentationComposition } from '../createWorkbenchPreviewTerminalPresentationComposition';
import { WorkbenchProjectWorkspaceRuntime } from '../WorkbenchProjectWorkspaceRuntime';

function createTestTerminalPresentationComposition(scope: string) {
	let sequence = 0;
	return createWorkbenchPreviewTerminalPresentationComposition(
		terminalPresentationId(`${scope}:initial`),
		() => terminalPresentationId(`${scope}:dynamic:${(sequence += 1)}`),
	);
}

const DIRECTORY: WorkbenchFolderDirectory = {
	available: true,
	git: null,
	id: 'directory-1',
	kind: 'folder',
	name: 'orchbee',
	path: '/projects/orchbee',
	placement: { kind: 'local' },
	source: 'selected',
};

const FEATURE_DIRECTORY: WorkbenchFolderDirectory = {
	...DIRECTORY,
	id: 'directory-2',
	name: 'feature',
	path: '/projects/feature',
};

const REMOTE_ENVIRONMENT_ID = 'environment:00000000000000000000000000000001';

const REMOTE_DIRECTORY: WorkbenchFolderDirectory = {
	...DIRECTORY,
	available: false,
	id: 'environment-directory:00000000000000000000000000000001',
	path: '/srv/orchbee',
	placement: {
		environmentId: REMOTE_ENVIRONMENT_ID,
		kind: 'environment',
	},
};

const REPOSITORY_DIRECTORY: WorkbenchFolderDirectory = {
	...DIRECTORY,
	git: {
		branch: 'main',
		head: 'abc123',
		repositoryId: 'repository-1',
		worktree: false,
	},
};

function createRuntime(): WorkbenchProjectWorkspaceRuntime {
	return new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Terminal 1',
		terminalPresentation: createTestTerminalPresentationComposition('runtime'),
	});
}

function createManagedRuntime(): WorkbenchProjectWorkspaceRuntime {
	let operationSequence = 1;
	let presentationSequence = 2;
	let removalSequence = 1;
	return new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Terminal 1',
		terminalPresentation: {
			createOperationId: () =>
				terminalPresentationOperationId(`managed:operation:${operationSequence++}`),
			createPresentationId: () =>
				terminalPresentationId(`managed:presentation:${presentationSequence++}`),
			createRemovalId: () => terminalPresentationRemovalId(`managed:removal:${removalSequence++}`),
			dispatchCommands: () => undefined,
			initialPresentationId: terminalPresentationId('managed:presentation:1'),
			kind: 'managed',
		},
	});
}

describe('WorkbenchProjectWorkspaceRuntime', () => {
	it('materializes an empty remote Workspace context without dispatching a terminal effect', () => {
		let operationSequence = 1;
		let presentationSequence = 2;
		const commands: WorkbenchTerminalPresentationCommand[] = [];
		const runtime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: {
				createOperationId: () =>
					terminalPresentationOperationId(`remote:operation:${operationSequence++}`),
				createPresentationId: () =>
					terminalPresentationId(`remote:presentation:${presentationSequence++}`),
				createRemovalId: () => terminalPresentationRemovalId('remote:removal:1'),
				dispatchCommands: (nextCommands) => commands.push(...nextCommands),
				initialPresentationId: terminalPresentationId('remote:presentation:1'),
				kind: 'managed',
			},
		});
		expect(commands).toHaveLength(1);
		expect(runtime.resolveActiveTerminalCreationRoute({ placement: { groupId: null } })).toEqual({
			kind: 'local',
			location: { kind: 'standalone' },
		});
		runtime.transition({
			directory: REMOTE_DIRECTORY,
			projectName: 'orchbee',
			type: 'environment-project-registered',
		});
		const result = runtime.transition({
			directoryId: REMOTE_DIRECTORY.id,
			projectId: 'project-1',
			type: 'environment-workspace-registered',
			workspaceName: 'orchbee',
		});
		runtime.transition({
			projectId: 'project-1',
			type: 'workspace-activated',
			workspaceId: 'workspace-1',
		});

		expect(result.outcome).toBe('applied');
		expect(commands).toHaveLength(1);
		expect(runtime.resolveActiveTerminalCreationRoute({ placement: { groupId: null } })).toEqual({
			kind: 'remote',
			projectId: 'project-1',
			workspaceId: 'workspace-1',
		});
		expect(
			runtime.resolveActiveTerminalCreationRoute({ placement: { groupId: 'group-1' } }),
		).toEqual({ kind: 'rejected', reason: 'remote-terminal-placement-unsupported' });
		expect(
			runtime.resolveActiveTerminalCreationRoute({ placement: { groupId: null, index: 0 } }),
		).toEqual({ kind: 'rejected', reason: 'remote-terminal-placement-unsupported' });
		expect(
			runtime.resolveActiveTerminalCreationRoute({
				directoryId: REMOTE_DIRECTORY.id,
				placement: { groupId: null },
			}),
		).toEqual({ kind: 'rejected', reason: 'remote-terminal-placement-unsupported' });
		expect(
			runtime
				.getSharedDocument(workbenchSharedDocumentRevision('1'))
				.terminalContexts.find((context) => context.terminalContextId === 'terminal-context-2'),
		).toMatchObject({
			nextTabSequence: 1,
			panelWorkspace: { pendingRemovals: [], tabs: [] },
			tabCollection: { groups: [], tabs: [] },
		});
		expect(
			runtime.createTerminalAtLocation({
				fallbackTitle: 'Remote Terminal',
				location: { kind: 'workspace', projectId: 'project-1', workspaceId: 'workspace-1' },
				placement: { groupId: null },
			}),
		).toEqual({ kind: 'rejected', reason: 'workspace-directory-remote' });
		expect(commands).toHaveLength(1);

		const clientViewBefore = runtime.getClientView();
		const created = runtime.createEnvironmentTerminalAtLocation({
			environmentId: REMOTE_ENVIRONMENT_ID,
			fallbackTitle: 'Remote Terminal',
			placement: { groupId: null },
			projectId: 'project-1',
			workspaceId: 'workspace-1',
		});
		expect(created).toMatchObject({
			intent: { kind: 'provision' },
			kind: 'created',
			terminalContextId: 'terminal-context-2',
		});
		if (created.kind !== 'created') throw new Error('Expected a created remote Terminal.');
		expect(
			runtime.hasPublishedTerminal({
				panelId: created.panelId,
				presentationId: created.presentationId,
				projectId: 'project-1',
				tabId: created.tabId,
				terminalContextId: created.terminalContextId,
				workspaceId: 'workspace-1',
			}),
		).toBe(true);
		expect(
			runtime.hasPublishedTerminal({
				panelId: created.panelId,
				presentationId: 'other-presentation',
				projectId: 'project-1',
				tabId: created.tabId,
				terminalContextId: created.terminalContextId,
				workspaceId: 'workspace-1',
			}),
		).toBe(false);
		expect(runtime.resolveActiveTerminalCreationRoute({ placement: { groupId: null } })).toEqual({
			kind: 'rejected',
			reason: 'remote-terminal-context-not-empty',
		});
		expect(runtime.getClientView()).toMatchObject({
			activeLocation: clientViewBefore.activeLocation,
			expandedProjectIds: clientViewBefore.expandedProjectIds,
		});
		expect(
			runtime
				.getSharedDocument(workbenchSharedDocumentRevision('2'))
				.terminalContexts.find((context) => context.terminalContextId === 'terminal-context-2')
				?.panelWorkspace.tabs[0]?.root,
		).toMatchObject({
			kind: 'panel',
			primaryPresentation: { launchCwd: '/srv/orchbee' },
		});
		expect(commands.at(-1)).toMatchObject({
			launchCwd: '/srv/orchbee',
			type: 'terminal-provision-requested',
		});
		expect(
			runtime.createEnvironmentTerminalAtLocation({
				environmentId: REMOTE_ENVIRONMENT_ID,
				fallbackTitle: 'Second Remote Terminal',
				placement: { groupId: null },
				projectId: 'project-1',
				workspaceId: 'workspace-1',
			}),
		).toEqual({ kind: 'rejected', reason: 'remote-terminal-context-not-empty' });
		expect(
			runtime.createEnvironmentTerminalAtLocation({
				environmentId: 'environment:00000000000000000000000000000002',
				fallbackTitle: 'Wrong Environment',
				placement: { groupId: null },
				projectId: 'project-1',
				workspaceId: 'workspace-1',
			}),
		).toEqual({ kind: 'rejected', reason: 'workspace-environment-mismatch' });
	});

	it('materializes the terminal context for an atomically created Worktree Workspace', () => {
		const runtime = createManagedRuntime();
		runtime.transition({
			directory: REPOSITORY_DIRECTORY,
			launchConfigurations: [],
			primaryWorkspaceLaunchConfigurationId: null,
			primaryWorkspaceName: 'Primary',
			projectName: 'orchbee',
			terminalTitle: 'Terminal 1',
			type: 'project-created',
		});
		const result = runtime.transition({
			createWorkspace: true,
			directory: {
				available: true,
				git: {
					branch: 'orchbee/workspace-1',
					head: 'abc123',
					repositoryId: 'repository-1',
					worktree: true,
				},
				id: 'managed-worktree-1',
				kind: 'worktree',
				name: 'orchbee/workspace-1',
				path: '/managed/worktrees/managed-worktree-1',
				placement: { kind: 'local' },
				source: 'created',
			},
			projectId: 'project-1',
			sourceDirectoryId: REPOSITORY_DIRECTORY.id,
			terminalTitle: 'Terminal 2',
			type: 'project-worktree-created',
			workspaceName: 'orchbee/workspace-1',
		});

		expect(result.outcome).toBe('applied');
		expect(runtime.getActiveDefaultCwd()).toBe('/managed/worktrees/managed-worktree-1');
		expect(runtime.getActiveTerminalContext().panelWorkspaceStore.getSnapshot()).toMatchObject({
			tabs: [
				{
					root: {
						kind: 'panel',
						primaryPresentation: {
							launchCwd: '/managed/worktrees/managed-worktree-1',
						},
					},
				},
			],
		});
		expect(
			runtime
				.getSharedDocument(workbenchSharedDocumentRevision('3'))
				.terminalContexts.find((context) => context.terminalContextId === 'terminal-context-3')
				?.panelWorkspace.tabs[0]?.root,
		).toMatchObject({
			kind: 'panel',
			primaryPresentation: { intent: { kind: 'provision' } },
		});
	});

	it('routes managed lifecycle results to recovered standalone presentations by opaque identity', () => {
		let operationSequence = 1;
		let removalSequence = 1;
		const commands: WorkbenchTerminalPresentationCommand[] = [];
		const runtime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standalonePresentations: [
				{
					cwd: '/first',
					presentationId: terminalPresentationId('managed:first'),
					title: 'Terminal 1',
				},
				{
					cwd: '/second',
					presentationId: terminalPresentationId('managed:second'),
					title: 'Terminal 2',
				},
			],
			standaloneTerminalTitle: 'Terminal',
			terminalPresentation: {
				createOperationId: () =>
					terminalPresentationOperationId(`managed:operation:${operationSequence++}`),
				createPresentationId: () => terminalPresentationId('managed:dynamic'),
				createRemovalId: () =>
					terminalPresentationRemovalId(`managed:removal:${removalSequence++}`),
				dispatchCommands: (nextCommands) => commands.push(...nextCommands),
				initialPresentationId: terminalPresentationId('managed:initial'),
				kind: 'managed',
			},
		});

		expect(commands).toEqual([
			expect.objectContaining({
				launchCwd: '/first',
				operationId: 'managed:operation:1',
				presentationId: 'managed:first',
				type: 'terminal-provision-requested',
			}),
			expect.objectContaining({
				launchCwd: '/second',
				operationId: 'managed:operation:2',
				presentationId: 'managed:second',
				type: 'terminal-provision-requested',
			}),
		]);
		expect(
			runtime.applyTerminalPresentationEvent({
				operationId: terminalPresentationOperationId('managed:operation:1'),
				presentationId: terminalPresentationId('managed:first'),
				type: 'terminal-provision-completed',
			}),
		).toEqual({ kind: 'applied' });
		expect(
			runtime.applyTerminalPresentationEvent({
				exitCode: 0,
				presentationId: terminalPresentationId('managed:first'),
				signal: null,
				type: 'terminal-exited',
			}),
		).toEqual({ kind: 'applied' });
		expect(commands.at(-1)).toEqual({
			disposition: 'remove',
			operationId: 'managed:operation:3',
			presentationId: 'managed:first',
			type: 'terminal-close-requested',
		});
		expect(
			runtime.getActiveTerminalContext().panelWorkspaceStore.getSnapshot().tabs[0]?.root,
		).toMatchObject({
			primaryPresentation: {
				lifecycle: {
					kind: 'closing',
					operationId: 'managed:operation:3',
					removalId: 'managed:removal:1',
				},
			},
		});
		expect(
			runtime.applyTerminalPresentationEvent({
				exitCode: 0,
				presentationId: terminalPresentationId('managed:first'),
				signal: null,
				type: 'terminal-exited',
			}),
		).toEqual({ kind: 'unchanged' });
		expect(commands).toHaveLength(3);
		expect(
			runtime.applyTerminalPresentationEvent({
				operationId: terminalPresentationOperationId('managed:operation:3'),
				presentationId: terminalPresentationId('managed:first'),
				type: 'terminal-close-completed',
			}),
		).toEqual({ kind: 'applied' });
		expect(
			runtime
				.getActiveTerminalContext()
				.tabModel.getSnapshot()
				.tabs.map((tab) => tab.id),
		).toEqual(['terminal-context-1-tab-2']);
		expect(
			runtime.applyTerminalPresentationEvent({
				operationId: terminalPresentationOperationId('managed:operation:missing'),
				presentationId: terminalPresentationId('managed:second'),
				type: 'terminal-provision-completed',
			}),
		).toEqual({ kind: 'rejected', reason: 'stale-operation' });
	});

	it('closes the final Tab after a signaled natural exit without closing the Workbench runtime', () => {
		let operationSequence = 1;
		const commands: WorkbenchTerminalPresentationCommand[] = [];
		const presentationId = terminalPresentationId('managed:natural-exit');
		const runtime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: {
				createOperationId: () =>
					terminalPresentationOperationId(`managed:natural-operation:${operationSequence++}`),
				createPresentationId: () => presentationId,
				createRemovalId: () => terminalPresentationRemovalId('managed:natural-removal'),
				dispatchCommands: (nextCommands) => commands.push(...nextCommands),
				initialPresentationId: presentationId,
				kind: 'managed',
			},
		});
		runtime.applyTerminalPresentationEvent({
			operationId: terminalPresentationOperationId('managed:natural-operation:1'),
			presentationId,
			type: 'terminal-provision-completed',
		});
		const documentListener = vi.fn<() => void>();
		runtime.subscribeDocument(documentListener);

		expect(
			runtime.applyTerminalPresentationEvent({
				exitCode: null,
				presentationId,
				signal: 'SIGTERM',
				type: 'terminal-exited',
			}),
		).toEqual({ kind: 'applied' });
		expect(documentListener).toHaveBeenCalledOnce();
		expect(runtime.getSharedDocument(workbenchSharedDocumentRevision('1'))).toMatchObject({
			terminalContexts: [
				{
					panelWorkspace: {
						pendingRemovals: [{ kind: 'tab', presentationIds: [presentationId] }],
						tabs: [
							{
								root: {
									primaryPresentation: { intent: { kind: 'close' } },
								},
							},
						],
					},
				},
			],
		});
		expect(commands.at(-1)).toEqual({
			disposition: 'remove',
			operationId: 'managed:natural-operation:2',
			presentationId,
			type: 'terminal-close-requested',
		});
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toHaveLength(1);

		expect(
			runtime.applyTerminalPresentationEvent({
				operationId: terminalPresentationOperationId('managed:natural-operation:2'),
				presentationId,
				type: 'terminal-close-completed',
			}),
		).toEqual({ kind: 'applied' });
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toEqual([]);
		expect(runtime.getActiveTerminalContext().panelWorkspaceStore.getSnapshot().tabs).toEqual([]);
	});

	it('keeps a managed Tab until its exact terminal close result completes', () => {
		let operationSequence = 1;
		const commands: WorkbenchTerminalPresentationCommand[] = [];
		const presentationId = terminalPresentationId('managed:close-target');
		const runtime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: {
				createOperationId: () =>
					terminalPresentationOperationId(`managed:close-operation:${operationSequence++}`),
				createPresentationId: () => presentationId,
				createRemovalId: () => terminalPresentationRemovalId('managed:close-removal'),
				dispatchCommands: (nextCommands) => commands.push(...nextCommands),
				initialPresentationId: presentationId,
				kind: 'managed',
			},
		});
		runtime.applyTerminalPresentationEvent({
			operationId: terminalPresentationOperationId('managed:close-operation:1'),
			presentationId,
			type: 'terminal-provision-completed',
		});

		expect(runtime.requestActiveTerminalTabsClose(['terminal-context-1-tab-1'])).toEqual({
			kind: 'applied',
		});
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toHaveLength(1);
		expect(commands.at(-1)).toEqual({
			disposition: 'remove',
			operationId: 'managed:close-operation:2',
			presentationId: 'managed:close-target',
			type: 'terminal-close-requested',
		});

		expect(
			runtime.applyTerminalPresentationEvent({
				operationId: terminalPresentationOperationId('managed:close-operation:2'),
				presentationId,
				type: 'terminal-close-completed',
			}),
		).toEqual({ kind: 'applied' });
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toHaveLength(0);
		expect(commands.some((command) => command.type === 'tab-close-ready')).toBe(false);
	});

	it('does not protect a fresh managed shell until activity becomes uncertain', () => {
		const runtime = createManagedRuntime();
		const context = runtime.getActiveTerminalContext();
		const tab = context.panelWorkspaceStore.getSnapshot().tabs[0];
		if (tab === undefined || tab.root.kind !== 'panel') {
			throw new Error('Expected one initial terminal panel.');
		}
		const presentationId = tab.root.primaryPresentation.presentationId;

		expect(runtime.getActiveTerminalTabsCloseConfirmationCount([tab.tabId])).toBe(0);
		expect(
			runtime.applyTerminalMetadataObservation(presentationId, {
				closeActivity: 'unknown',
				currentDirectory: { kind: 'ignored' },
				naturalTitle: null,
			}),
		).toEqual({ kind: 'applied' });
		expect(runtime.getActiveTerminalTabsCloseConfirmationCount([tab.tabId])).toBe(1);
		expect(
			runtime.applyTerminalMetadataObservation(presentationId, {
				closeActivity: 'idle',
				currentDirectory: { kind: 'ignored' },
				naturalTitle: null,
			}),
		).toEqual({ kind: 'applied' });
		expect(runtime.getActiveTerminalTabsCloseConfirmationCount([tab.tabId])).toBe(0);
		expect(
			runtime.applyTerminalPresentationEvent({
				operationId: terminalPresentationOperationId('managed:operation:1'),
				presentationId,
				type: 'terminal-provision-completed',
			}),
		).toEqual({ kind: 'applied' });
		expect(runtime.getActiveTerminalTabsCloseConfirmationCount([tab.tabId])).toBe(0);

		for (const [closeActivity, expectedCount] of [
			['unknown', 1],
			['idle', 0],
			['active', 1],
		] as const) {
			expect(
				runtime.applyTerminalMetadataObservation(presentationId, {
					closeActivity,
					currentDirectory: { kind: 'ignored' },
					naturalTitle: null,
				}),
			).toEqual({ kind: 'applied' });
			expect(runtime.getActiveTerminalTabsCloseConfirmationCount([tab.tabId])).toBe(expectedCount);
		}
	});

	it('counts close protection across every panel and child terminal in a Tab', () => {
		const runtime = createManagedRuntime();
		const context = runtime.getActiveTerminalContext();
		const tab = context.panelWorkspaceStore.getSnapshot().tabs[0];
		if (tab === undefined || tab.root.kind !== 'panel') {
			throw new Error('Expected one initial terminal panel.');
		}
		const tabId = tab.tabId;
		const initialPanelId = tab.root.id;
		const initialPresentationId = tab.root.primaryPresentation.presentationId;
		expect(
			runtime.applyTerminalPresentationEvent({
				operationId: terminalPresentationOperationId('managed:operation:1'),
				presentationId: initialPresentationId,
				type: 'terminal-provision-completed',
			}),
		).toEqual({ kind: 'applied' });

		const splitPresentation = context.panelWorkspaceStore.createTerminalPresentation();
		const childPresentation = context.panelWorkspaceStore.createTerminalPresentation();
		if (splitPresentation.kind !== 'managed' || childPresentation.kind !== 'managed') {
			throw new Error('Expected managed terminal presentations.');
		}
		expect(
			context.panelWorkspaceStore.transition({
				direction: 'right',
				newTerminalTitle: 'Terminal 2',
				panelId: initialPanelId,
				presentation: splitPresentation,
				tabId,
				type: 'panel-split',
			}).outcome,
		).toBe('applied');
		expect(
			context.panelWorkspaceStore.transition({
				newTerminalTitle: 'Terminal 3',
				panelId: initialPanelId,
				presentation: childPresentation,
				tabId,
				type: 'child-terminal-created',
			}).outcome,
		).toBe('applied');

		for (const presentation of [splitPresentation, childPresentation]) {
			expect(
				runtime.applyTerminalPresentationEvent({
					operationId: presentation.operationId,
					presentationId: presentation.presentationId,
					type: 'terminal-provision-completed',
				}),
			).toEqual({ kind: 'applied' });
		}
		for (const [presentationId, closeActivity, resultKind] of [
			[initialPresentationId, 'idle', 'unchanged'],
			[splitPresentation.presentationId, 'active', 'applied'],
			[childPresentation.presentationId, 'active', 'applied'],
		] as const) {
			expect(
				runtime.applyTerminalMetadataObservation(presentationId, {
					closeActivity,
					currentDirectory: { kind: 'ignored' },
					naturalTitle: null,
				}),
			).toEqual({ kind: resultKind });
		}

		expect(runtime.getActiveTerminalTabsCloseConfirmationCount([tabId])).toBe(2);
		expect(runtime.getActiveTerminalTabsCloseConfirmationCount([tabId, tabId])).toBe(2);
		expect(runtime.getActiveTerminalTabsCloseConfirmationCount(['missing-tab'])).toBe(0);
		expect(runtime.getTerminalSessionCloseConfirmationCount()).toBe(2);

		for (const presentationId of [
			splitPresentation.presentationId,
			childPresentation.presentationId,
		]) {
			expect(
				runtime.applyTerminalMetadataObservation(presentationId, {
					closeActivity: 'idle',
					currentDirectory: { kind: 'ignored' },
					naturalTitle: null,
				}),
			).toEqual({ kind: 'applied' });
		}
		expect(runtime.getTerminalSessionCloseConfirmationCount()).toBe(0);
	});

	it('starts with one standalone Tab, Panel, and launch CWD', () => {
		const runtime = createRuntime();
		const context = runtime.getActiveTerminalContext();
		const tab = context.tabModel.getSnapshot().tabs[0];
		const panelWorkspace = context.panelWorkspaceStore.getSnapshot().tabs[0];

		expect(runtime.getSnapshot().activeLocation).toEqual({ kind: 'standalone' });
		expect(tab).toMatchObject({ id: 'terminal-context-1-tab-1', title: 'system' });
		expect(panelWorkspace?.root).toMatchObject({
			kind: 'panel',
			primaryPresentation: {
				currentCwd: '/system',
				launchCwd: '/system',
				title: 'Terminal 1',
			},
		});
	});

	it('uses current or launch directory context until a natural title arrives', () => {
		const runtime = createManagedRuntime();
		const context = runtime.getActiveTerminalContext();
		const workspace = context.panelWorkspaceStore.getSnapshot().tabs[0];
		if (workspace === undefined || workspace.root.kind !== 'panel') {
			throw new Error('Expected one active terminal Tab.');
		}
		const presentationId = workspace.root.primaryPresentation.presentationId;

		expect(context.tabModel.getSnapshot().tabs[0]?.title).toBe('system');
		expect(
			runtime.applyTerminalMetadataObservation(presentationId, {
				closeActivity: 'idle',
				currentDirectory: { currentCwd: '/workspace/orchbee', kind: 'reported' },
				naturalTitle: null,
			}),
		).toEqual({ kind: 'applied' });
		expect(context.tabModel.getSnapshot().tabs[0]?.title).toBe('orchbee');

		runtime.applyTerminalMetadataObservation(presentationId, {
			closeActivity: 'active',
			currentDirectory: { kind: 'ignored' },
			naturalTitle: 'vim',
		});
		expect(context.tabModel.getSnapshot().tabs[0]?.title).toBe('vim');

		runtime.applyTerminalMetadataObservation(presentationId, {
			closeActivity: 'idle',
			currentDirectory: { currentCwd: 'C:\\Users\\liushuai\\orchbee', kind: 'reported' },
			naturalTitle: null,
		});
		expect(context.tabModel.getSnapshot().tabs[0]?.title).toBe('orchbee');

		runtime.applyTerminalMetadataObservation(presentationId, {
			closeActivity: 'idle',
			currentDirectory: { currentCwd: null, kind: 'reported' },
			naturalTitle: null,
		});
		expect(context.tabModel.getSnapshot().tabs[0]?.title).toBe('system');
	});

	it('keeps Warp-style natural and custom titles separate from launch metadata', () => {
		const runtime = createManagedRuntime();
		const context = runtime.getActiveTerminalContext();
		const tab = context.panelWorkspaceStore.getSnapshot().tabs[0];
		const tabId = context.tabModel.getSnapshot().activeTabId;
		if (tab === undefined || tabId === null || tab.root.kind !== 'panel') {
			throw new Error('Expected one active terminal Tab.');
		}
		const presentationId = tab.root.primaryPresentation.presentationId;

		expect(
			runtime.applyTerminalMetadataObservation(presentationId, {
				closeActivity: 'active',
				currentDirectory: { currentCwd: '/workspace/current', kind: 'reported' },
				naturalTitle: 'vim',
			}),
		).toEqual({ kind: 'applied' });
		expect(context.tabModel.getSnapshot().tabs[0]?.title).toBe('vim');
		expect(tab.root.primaryPresentation.launchCwd).toBe('/system');
		expect(context.panelWorkspaceStore.setTabCustomTitle(tabId, 'vim')).toEqual({
			kind: 'applied',
		});
		runtime.applyTerminalMetadataObservation(presentationId, {
			closeActivity: 'idle',
			currentDirectory: { kind: 'ignored' },
			naturalTitle: 'shell after equal override',
		});
		expect(context.tabModel.getSnapshot().tabs[0]?.title).toBe('vim');
		expect(context.panelWorkspaceStore.setTabCustomTitle(tabId, null)).toEqual({
			kind: 'applied',
		});
		expect(context.tabModel.getSnapshot().tabs[0]?.title).toBe('shell after equal override');

		expect(context.panelWorkspaceStore.setTabCustomTitle(tabId, '  Custom title  ')).toEqual({
			kind: 'applied',
		});
		expect(context.tabModel.getSnapshot().tabs[0]?.title).toBe('Custom title');
		runtime.applyTerminalMetadataObservation(presentationId, {
			closeActivity: 'idle',
			currentDirectory: { kind: 'ignored' },
			naturalTitle: 'ssh',
		});
		expect(context.tabModel.getSnapshot().tabs[0]?.title).toBe('Custom title');
		expect(context.panelWorkspaceStore.setTabCustomTitle(tabId, null)).toEqual({
			kind: 'applied',
		});
		expect(context.tabModel.getSnapshot().tabs[0]?.title).toBe('ssh');
		expect(context.panelWorkspaceStore.setTabCustomTitle(tabId, '   ')).toEqual({
			kind: 'unchanged',
		});

		context.tabModel.updateTab(tabId, { title: 'Persisted custom' });
		const document = runtime.getSharedDocument(workbenchSharedDocumentRevision('7'));
		expect(document.terminalContexts[0]?.tabCollection.tabs[0]).toMatchObject({
			customTitle: 'Persisted custom',
			title: 'Terminal 1',
		});
		expect(document.terminalContexts[0]?.panelWorkspace.tabs[0]?.root).toMatchObject({
			primaryPresentation: { title: 'Terminal 1' },
		});
		const restored = new WorkbenchProjectWorkspaceRuntime({
			initialDocument: document,
			standaloneCwd: '/fallback',
			standaloneTerminalTitle: 'Unused',
			terminalPresentation: createTestTerminalPresentationComposition('restored-title'),
		});
		const restoredContext = restored.getActiveTerminalContext();
		expect(restoredContext.tabModel.getSnapshot().tabs[0]?.title).toBe('Persisted custom');
		restored.applyTerminalMetadataObservation(presentationId, {
			closeActivity: 'idle',
			currentDirectory: { kind: 'ignored' },
			naturalTitle: 'restored shell',
		});
		expect(restoredContext.tabModel.getSnapshot().tabs[0]?.title).toBe('Persisted custom');
		expect(restoredContext.panelWorkspaceStore.setTabCustomTitle(tabId, null)).toEqual({
			kind: 'applied',
		});
		expect(restoredContext.tabModel.getSnapshot().tabs[0]?.title).toBe('restored shell');
	});

	it('preserves independent terminal contexts when switching locations', () => {
		const runtime = createRuntime();
		runtime.createTerminal({
			groupId: null,
			source: 'button',
			title: 'Terminal 2',
		});
		runtime.transition({
			directory: DIRECTORY,
			launchConfigurations: [],
			primaryWorkspaceName: 'Primary',
			primaryWorkspaceLaunchConfigurationId: null,
			projectName: 'orchbee',
			terminalTitle: 'Terminal 1',
			type: 'project-created',
		});
		const workspaceContext = runtime.getActiveTerminalContext();

		expect(workspaceContext.tabModel.getSnapshot().tabs).toHaveLength(1);
		expect(workspaceContext.panelWorkspaceStore.getSnapshot().tabs[0]?.root).toMatchObject({
			kind: 'panel',
			primaryPresentation: {
				currentCwd: '/projects/orchbee',
				launchCwd: '/projects/orchbee',
			},
		});

		runtime.transition({ type: 'standalone-activated' });
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toHaveLength(2);
	});

	it('notifies once after an applied domain transition and not after rejection', () => {
		const runtime = createRuntime();
		const listener = vi.fn<() => void>();
		runtime.subscribe(listener);

		runtime.transition({
			directory: DIRECTORY,
			launchConfigurations: [],
			primaryWorkspaceName: 'Primary',
			primaryWorkspaceLaunchConfigurationId: null,
			projectName: 'orchbee',
			terminalTitle: 'Terminal 1',
			type: 'project-created',
		});
		runtime.transition({
			projectId: 'missing',
			type: 'workspace-activated',
			workspaceId: 'missing',
		});

		expect(listener).toHaveBeenCalledTimes(1);
	});

	it('observes complete nested document projection and releases child subscriptions with the observer', () => {
		const runtime = createManagedRuntime();
		const context = runtime.getActiveTerminalContext();
		const listener = vi.fn<() => void>();
		const unsubscribe = runtime.subscribeDocument(listener);
		const activeTabId = context.tabModel.getSnapshot().activeTabId;
		if (activeTabId === null) throw new Error('Expected an active Tab.');

		context.tabModel.updateTab(activeTabId, { title: 'Renamed' });
		expect(listener).toHaveBeenCalled();
		listener.mockClear();
		unsubscribe();
		context.tabModel.updateTab(activeTabId, { title: 'Renamed again' });

		expect(listener).not.toHaveBeenCalled();
	});

	it('observes preview terminal topology without materializing a shared document', () => {
		const runtime = createRuntime();
		const context = runtime.getActiveTerminalContext();
		const listener = vi.fn<() => void>();
		const initialRevision = runtime.getTerminalTopologyRevision();
		const unsubscribe = runtime.subscribeTerminalTopology(listener);
		const activeTabId = context.tabModel.getSnapshot().activeTabId;
		if (activeTabId === null) throw new Error('Expected an active Tab.');

		context.tabModel.updateTab(activeTabId, { title: 'Renamed preview Tab' });

		expect(listener).toHaveBeenCalledOnce();
		expect(runtime.getTerminalTopologyRevision()).not.toBe(initialRevision);
		listener.mockClear();
		unsubscribe();
		context.tabModel.updateTab(activeTabId, { title: 'Renamed after unsubscribe' });

		expect(listener).not.toHaveBeenCalled();
	});

	it('notifies shared-document and Client View observers only for their owned projection', () => {
		const runtime = createManagedRuntime();
		runtime.createTerminal({ groupId: null, source: 'button', title: 'Terminal 2' });
		const context = runtime.getActiveTerminalContext();
		const documentListener = vi.fn<() => void>();
		const clientViewListener = vi.fn<() => void>();
		const unsubscribeDocument = runtime.subscribeDocument(documentListener);
		const unsubscribeClientView = runtime.subscribeClientView(clientViewListener);

		context.tabModel.selectTab('terminal-context-1-tab-1');

		expect(clientViewListener).toHaveBeenCalledOnce();
		expect(documentListener).not.toHaveBeenCalled();
		clientViewListener.mockClear();
		context.tabModel.updateTab('terminal-context-1-tab-1', { title: 'Renamed' });
		expect(documentListener).toHaveBeenCalledOnce();
		expect(clientViewListener).not.toHaveBeenCalled();
		unsubscribeClientView();
		unsubscribeDocument();
	});

	it('crosses the Action commit point before applying an authoritative transition', () => {
		const runtime = createRuntime();
		const initialState = runtime.getSnapshot();
		const beforeCommit = vi.fn<() => void>(() => {
			expect(runtime.getSnapshot()).toBe(initialState);
			expect(runtime.getTerminalContext('terminal-context-2')).toBeNull();
		});

		const result = runtime.transition(
			{
				directory: DIRECTORY,
				launchConfigurations: [],
				primaryWorkspaceName: 'Primary',
				primaryWorkspaceLaunchConfigurationId: null,
				projectName: 'orchbee',
				terminalTitle: 'Terminal 1',
				type: 'project-created',
			},
			beforeCommit,
		);

		expect(result.outcome).toBe('applied');
		expect(beforeCommit).toHaveBeenCalledOnce();
		expect(runtime.getSnapshot()).not.toBe(initialState);
		expect(runtime.getTerminalContext('terminal-context-2')).not.toBeNull();

		const unchangedCommit = vi.fn<() => void>();
		const rejectedCommit = vi.fn<() => void>();
		runtime.transition(
			{ name: 'orchbee', projectId: 'project-1', type: 'project-renamed' },
			unchangedCommit,
		);
		runtime.transition(
			{ name: 'Missing', projectId: 'missing', type: 'project-renamed' },
			rejectedCommit,
		);
		expect(unchangedCommit).not.toHaveBeenCalled();
		expect(rejectedCommit).not.toHaveBeenCalled();
	});

	it('isolates observer failures after an authoritative transition', () => {
		const runtime = createRuntime();
		const healthyListener = vi.fn<() => void>();
		runtime.subscribe(() => {
			throw new Error('observer failed');
		});
		runtime.subscribe(healthyListener);

		expect(() =>
			runtime.transition({
				directory: DIRECTORY,
				launchConfigurations: [],
				primaryWorkspaceName: 'Primary',
				primaryWorkspaceLaunchConfigurationId: null,
				projectName: 'orchbee',
				terminalTitle: 'Terminal 1',
				type: 'project-created',
			}),
		).not.toThrow();
		expect(healthyListener).toHaveBeenCalledOnce();
		expect(runtime.getSnapshot().projects).toHaveLength(1);
	});

	it('duplicates layout and launch CWD into fresh scoped identities', () => {
		const runtime = createRuntime();
		runtime.transition({
			directory: DIRECTORY,
			launchConfigurations: [],
			primaryWorkspaceName: 'Primary',
			primaryWorkspaceLaunchConfigurationId: null,
			projectName: 'orchbee',
			terminalTitle: 'Terminal 1',
			type: 'project-created',
		});
		const source = runtime.getActiveTerminalContext();
		const sourceTabId = source.tabModel.getSnapshot().activeTabId;
		const sourceTab = source.panelWorkspaceStore.getSnapshot().tabs[0];
		if (sourceTabId === null || sourceTab === undefined || sourceTab.root.kind !== 'panel') {
			throw new Error('Source terminal context is incomplete.');
		}
		source.panelWorkspaceStore.transition({
			direction: 'right',
			newTerminalTitle: 'Terminal 2',
			panelId: sourceTab.root.id,
			presentation: source.panelWorkspaceStore.createTerminalPresentation(),
			tabId: sourceTabId,
			type: 'panel-split',
		});
		source.panelWorkspaceStore.transition({
			currentCwd: '/projects/changed',
			presentationId: sourceTab.root.primaryPresentation.presentationId,
			tabId: sourceTabId,
			type: 'terminal-current-cwd-changed',
		});
		source.tabModel.updateTab(sourceTabId, {
			alertState: 'audio-playing',
			crashed: true,
			loading: true,
		});
		const sourceSplitTab = source.panelWorkspaceStore.getSnapshot().tabs[0];

		runtime.transition({
			copyName: 'Primary Copy',
			projectId: 'project-1',
			type: 'workspace-duplicated',
			workspaceId: 'workspace-1',
		});
		const copy = runtime.getActiveTerminalContext();
		const copiedTab = copy.panelWorkspaceStore.getSnapshot().tabs[0];

		expect(copy).not.toBe(source);
		expect(copy.tabModel.getSnapshot().tabs[0]).toEqual({
			group: null,
			id: 'terminal-context-3-tab-1',
			pinned: false,
			title: 'orchbee',
		});
		expect(copiedTab?.root).not.toBe(sourceSplitTab?.root);
		expect(copiedTab?.root).toMatchObject({
			kind: 'split',
			children: [
				{
					id: 'terminal-context-3-panel-slot-1',
					node: {
						id: 'terminal-context-3-panel-2',
						kind: 'panel',
						primaryPresentation: {
							currentCwd: '/projects/orchbee',
							launchCwd: '/projects/orchbee',
							lifecycle: { kind: 'preview' },
						},
					},
				},
				{
					id: 'terminal-context-3-panel-slot-2',
					node: {
						id: 'terminal-context-3-panel-1',
						kind: 'panel',
						primaryPresentation: {
							currentCwd: '/projects/orchbee',
							launchCwd: '/projects/orchbee',
							lifecycle: { kind: 'preview' },
						},
					},
				},
			],
			id: 'terminal-context-3-split-1',
		});
		if (copiedTab?.root.kind !== 'split' || sourceSplitTab?.root.kind !== 'split') {
			throw new Error('Expected source and copied split layouts.');
		}
		const sourcePresentationIds = new Set(
			sourceSplitTab.root.children.flatMap((child) =>
				child.node.kind === 'panel' ? [child.node.primaryPresentation.presentationId] : [],
			),
		);
		const copiedPresentationIds = copiedTab.root.children.flatMap((child) =>
			child.node.kind === 'panel' ? [child.node.primaryPresentation.presentationId] : [],
		);
		expect(new Set(copiedPresentationIds).size).toBe(2);
		expect(copiedPresentationIds.every((id) => !sourcePresentationIds.has(id))).toBe(true);
	});

	it('reconciles a new Tab and its requested CWD before the context is observed', () => {
		const runtime = createRuntime();
		runtime.transition({
			directory: DIRECTORY,
			launchConfigurations: [],
			primaryWorkspaceLaunchConfigurationId: null,
			primaryWorkspaceName: 'Primary',
			projectName: 'orchbee',
			terminalTitle: 'Terminal 1',
			type: 'project-created',
		});
		runtime.transition({
			directory: FEATURE_DIRECTORY,
			projectId: 'project-1',
			type: 'directory-added',
		});
		const creation = runtime.createTerminal({
			directoryId: FEATURE_DIRECTORY.id,
			groupId: null,
			index: 0,
			source: 'button',
			title: 'Feature',
		});
		if (creation.kind !== 'created') {
			throw new Error(`Terminal creation failed: ${creation.reason}`);
		}
		const context = runtime.getActiveTerminalContext();
		const panelWorkspace = context.panelWorkspaceStore
			.getSnapshot()
			.tabs.find((tab) => tab.tabId === creation.tabId);

		expect(panelWorkspace?.root).toMatchObject({
			kind: 'panel',
			primaryPresentation: {
				currentCwd: '/projects/feature',
				launchCwd: '/projects/feature',
				title: 'Feature',
			},
		});
	});

	it('creates a Tab from the exact displayed terminal CWD without changing ordinary defaults', () => {
		const runtime = createRuntime();
		const context = runtime.getActiveTerminalContext();
		const sourceTab = context.panelWorkspaceStore.getSnapshot().tabs[0];
		if (sourceTab?.root.kind !== 'panel') throw new Error('Expected the initial terminal Panel.');
		const sourcePresentationId = sourceTab.root.primaryPresentation.presentationId;
		runtime.applyTerminalMetadataObservation(sourcePresentationId, {
			closeActivity: 'idle',
			currentDirectory: { currentCwd: '/projects/current', kind: 'reported' },
			naturalTitle: null,
		});
		const resolution = runtime.resolveActiveTerminalCurrentDirectoryTarget();
		if (resolution.kind !== 'ready')
			throw new Error(`Expected a current CWD: ${resolution.reason}`);

		const creation = runtime.createTerminal({
			currentDirectoryTarget: resolution.target,
			groupId: null,
			source: 'keyboard',
			title: 'Current directory',
		});
		if (creation.kind !== 'created')
			throw new Error(`Terminal creation failed: ${creation.reason}`);
		expect(
			context.panelWorkspaceStore.getSnapshot().tabs.find((tab) => tab.tabId === creation.tabId)
				?.root,
		).toMatchObject({
			kind: 'panel',
			primaryPresentation: {
				currentCwd: '/projects/current',
				launchCwd: '/projects/current',
			},
		});
		expect(
			runtime.createTerminal({
				currentDirectoryTarget: resolution.target,
				groupId: null,
				source: 'keyboard',
				title: 'Stale',
			}),
		).toEqual({ kind: 'rejected', reason: 'terminal-target-stale' });

		const ordinaryCreation = runtime.createTerminal({
			groupId: null,
			source: 'button',
			title: 'Default directory',
		});
		if (ordinaryCreation.kind !== 'created') {
			throw new Error(`Terminal creation failed: ${ordinaryCreation.reason}`);
		}
		expect(
			context.panelWorkspaceStore
				.getSnapshot()
				.tabs.find((tab) => tab.tabId === ordinaryCreation.tabId)?.root,
		).toMatchObject({
			kind: 'panel',
			primaryPresentation: { launchCwd: '/system' },
		});
	});

	it('creates a terminal from an authoritative CWD without consulting client-local focus', () => {
		const runtime = createManagedRuntime();

		const creation = runtime.createTerminalAtCwd({
			cwd: '/projects/new-window',
			groupId: null,
			source: 'programmatic',
			title: 'New window',
		});

		expect(creation.kind).toBe('created');
		if (creation.kind !== 'created') return;
		const terminalContext = runtime.getActiveTerminalContext();
		expect(
			terminalContext.panelWorkspaceStore
				.getSnapshot()
				.tabs.find((tab) => tab.tabId === creation.tabId)?.root,
		).toMatchObject({
			kind: 'panel',
			primaryPresentation: {
				currentCwd: '/projects/new-window',
				launchCwd: '/projects/new-window',
				title: 'New window',
			},
		});
	});

	it('resolves explicit Directory membership and refreshed availability before creating a Tab', () => {
		const runtime = createRuntime();
		runtime.transition({
			directory: DIRECTORY,
			launchConfigurations: [],
			primaryWorkspaceLaunchConfigurationId: null,
			primaryWorkspaceName: 'Primary',
			projectName: 'orchbee',
			terminalTitle: 'Terminal 1',
			type: 'project-created',
		});
		runtime.transition({
			directory: FEATURE_DIRECTORY,
			projectId: 'project-1',
			type: 'directory-added',
		});
		runtime.transition({
			directoryId: FEATURE_DIRECTORY.id,
			projectId: 'project-1',
			type: 'project-primary-directory-set',
		});
		runtime.transition({
			directory: { ...DIRECTORY, available: false },
			projectId: 'project-1',
			type: 'directory-updated',
		});

		expect(runtime.resolveActiveTerminalCwd(DIRECTORY.id)).toEqual({
			kind: 'rejected',
			reason: 'explicit-directory-unavailable',
		});
		expect(runtime.resolveActiveTerminalCwd('missing')).toEqual({
			kind: 'rejected',
			reason: 'explicit-directory-not-found',
		});
		expect(runtime.getActiveDefaultCwd()).toBe('/projects/feature');
		expect(
			runtime.createTerminal({
				directoryId: DIRECTORY.id,
				groupId: null,
				source: 'button',
				title: 'Unavailable',
			}),
		).toEqual({ kind: 'rejected', reason: 'explicit-directory-unavailable' });
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toHaveLength(1);
	});

	it('disposes deleted Workspace contexts without changing standalone', () => {
		const runtime = createRuntime();
		const standalone = runtime.getActiveTerminalContext();
		runtime.transition({
			directory: DIRECTORY,
			launchConfigurations: [],
			primaryWorkspaceName: 'Primary',
			primaryWorkspaceLaunchConfigurationId: null,
			projectName: 'orchbee',
			terminalTitle: 'Terminal 1',
			type: 'project-created',
		});
		runtime.transition({
			projectId: 'project-1',
			type: 'workspace-deleted',
			workspaceId: 'workspace-1',
		});

		expect(runtime.getTerminalContext('terminal-context-2')).toBeNull();
		expect(runtime.getActiveTerminalContext()).toBe(standalone);
	});
});
