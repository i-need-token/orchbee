// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import * as React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import { WORKBENCH_ACTION_IDS } from '../../actions/workbench-action-ids';
import { createWorkbenchPreviewTerminalPresentationComposition } from '../../model/createWorkbenchPreviewTerminalPresentationComposition';
import type {
	WorkbenchNavigationPort,
	WorkbenchTerminalFontSizeChangeCallback,
	WorkbenchTerminalCreateCallback,
} from '../../types/workbench-host-actions';
import { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import { WORKBENCH_SHORTCUTS } from '../../utils/workbench-shortcuts';
import { useWorkbenchActions } from '../useWorkbenchActions';

const unavailableFocusRouting = {
	beginTerminalHandover: () => ({ complete: () => undefined, held: () => true }),
	focus: () => ({ kind: 'unavailable', reason: 'no-available-region' }) as const,
	getResolution: () => ({ kind: 'unavailable', reason: 'no-available-region' }) as const,
};
const worktreeTestLabel = (ordinal: number) => `Worktree ${ordinal}`;

function createTestTerminalPresentationComposition(scope: string) {
	let sequence = 0;
	return createWorkbenchPreviewTerminalPresentationComposition(
		terminalPresentationId(`${scope}:initial`),
		() => terminalPresentationId(`${scope}:dynamic:${(sequence += 1)}`),
	);
}

interface WorkbenchActionsHarnessProps {
	readonly captureActions?: ((actions: ReturnType<typeof useWorkbenchActions>) => void) | undefined;
	readonly captureScope?: ((scope: ReturnType<ActionRuntime['createScope']>) => void) | undefined;
	readonly focusRouting?: Parameters<typeof useWorkbenchActions>[0]['focusRouting'] | undefined;
	readonly onNewTerminal?: WorkbenchTerminalCreateCallback | undefined;
	readonly onTerminalFontSizeChange?: WorkbenchTerminalFontSizeChangeCallback | undefined;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly projectWorkspaceRuntime?: WorkbenchProjectWorkspaceRuntime | undefined;
	readonly runtime: ActionRuntime;
}

function WorkbenchActionsHarness({
	captureActions,
	captureScope,
	focusRouting = unavailableFocusRouting,
	navigation,
	onNewTerminal,
	onTerminalFontSizeChange,
	projectWorkspaceRuntime: providedProjectWorkspaceRuntime,
	runtime,
}: WorkbenchActionsHarnessProps): React.ReactNode {
	const [isSidebarVisible, setIsSidebarVisible] = React.useState(true);
	const [projectWorkspaceRuntime] = React.useState(
		() =>
			providedProjectWorkspaceRuntime ??
			new WorkbenchProjectWorkspaceRuntime({
				standaloneCwd: '/system',
				standaloneTerminalTitle: 'Terminal 1',
				terminalPresentation: createTestTerminalPresentationComposition('action-harness'),
			}),
	);
	const projectWorkspaceState = React.useSyncExternalStore(
		projectWorkspaceRuntime.subscribe,
		projectWorkspaceRuntime.getSnapshot,
		projectWorkspaceRuntime.getSnapshot,
	);
	const actions = useWorkbenchActions({
		focusRouting,
		isSidebarVisible,
		navigation,
		onNewTerminal,
		onSettings: undefined,
		onTerminalFontSizeChange,
		openSearch: () => undefined,
		openTabTitleEditor: () => undefined,
		defaultWorkspaceName: 'Workspace',
		primaryWorkspaceName: 'Primary',
		projectWorkspaceRuntime,
		projectWorkspaceState,
		runtime,
		setTabStripOrientation: () => undefined,
		tabStripOrientation: 'horizontal',
		toggleSidebar: () => setIsSidebarVisible((visible) => !visible),
		workspaceCopySuffix: 'Copy',
		workspaceTerminalTitle: 'Terminal 1',
		worktreeLabel: worktreeTestLabel,
	});
	captureActions?.(actions);
	captureScope?.(actions.scope);

	return React.createElement(
		React.Fragment,
		null,
		React.createElement(
			'button',
			{
				onClick: () =>
					actions.invoke(WORKBENCH_ACTION_IDS.createTerminal, { groupId: null }, 'button'),
				type: 'button',
			},
			'create',
		),
		React.createElement(
			'button',
			{
				onClick: () => actions.invoke(WORKBENCH_ACTION_IDS.toggleSidebar, undefined, 'button'),
				type: 'button',
			},
			'toggle',
		),
		React.createElement('output', null, isSidebarVisible ? 'visible' : 'hidden'),
	);
}

afterEach(() => cleanup());

describe('useWorkbenchActions', () => {
	it('delegates location navigation without mutating the Workbench first', async () => {
		const runtime = new ActionRuntime();
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createTestTerminalPresentationComposition('route-navigation'),
		});
		projectWorkspaceRuntime.transition({
			directory: {
				available: true,
				git: null,
				id: 'directory-1',
				kind: 'folder',
				name: 'orchbee',
				path: '/workspace/orchbee',
				placement: { kind: 'local' },
				source: 'selected',
			},
			launchConfigurations: [],
			primaryWorkspaceLaunchConfigurationId: null,
			primaryWorkspaceName: 'Primary',
			projectName: 'orchbee',
			terminalTitle: 'Terminal 1',
			type: 'project-created',
		});
		const navigate = vi.fn<WorkbenchNavigationPort['navigate']>((context) => {
			context.markCommitted();
			return { kind: 'completed' };
		});
		const navigation: WorkbenchNavigationPort = {
			navigate,
			selectTab: () => ({ kind: 'unchanged' }),
		};
		let actionScope: ReturnType<ActionRuntime['createScope']> | undefined;
		render(
			React.createElement(WorkbenchActionsHarness, {
				captureScope: (scope) => {
					actionScope = scope;
				},
				navigation,
				projectWorkspaceRuntime,
				runtime,
			}),
		);
		if (actionScope === undefined) throw new Error('Expected the Workbench action scope.');

		await runtime.execute(WORKBENCH_ACTION_IDS.activateStandalone, undefined, {
			scope: actionScope,
			source: 'button',
		}).completion;

		expect(navigate).toHaveBeenCalledWith(expect.anything(), { kind: 'standalone' });
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
			kind: 'workspace',

			workspaceId: 'workspace-1',
		});
	});

	it('registers enumerable actions and routes product controls through their handlers', async () => {
		const runtime = new ActionRuntime();
		const onNewTerminal = vi.fn<WorkbenchTerminalCreateCallback>(() => ({
			kind: 'completed',
		}));
		let actionScope: ReturnType<ActionRuntime['createScope']> | undefined;
		render(
			React.createElement(WorkbenchActionsHarness, {
				captureScope: (scope) => {
					actionScope = scope;
				},
				onNewTerminal,
				runtime,
			}),
		);
		expect(actionScope).toBeDefined();
		if (actionScope === undefined) {
			throw new Error('Expected the Workbench action scope to be captured');
		}
		expect(
			new Set(
				runtime
					.list(
						{
							scope: actionScope,
							source: 'command-surface',
						},
						{
							[WORKBENCH_ACTION_IDS.setWorkspaceProject]: {
								workspaceId: 'missing',
								projectId: null,
							},
							[WORKBENCH_ACTION_IDS.createWorkspaceProject]: {
								workspaceId: 'missing',
								name: 'Group',
							},
							[WORKBENCH_ACTION_IDS.closeProjectWorkspaces]: { projectId: 'missing' },
							[WORKBENCH_ACTION_IDS.addResourceFolder]: {
								path: '/workspace/ghostty',
								resourceScopeId: 'missing-project',
							},
							[WORKBENCH_ACTION_IDS.openSearch]: {
								filter: 'all',
							},
							[WORKBENCH_ACTION_IDS.activateWorkspace]: {
								projectId: 'missing-project',
								workspaceId: 'missing-workspace',
							},
							[WORKBENCH_ACTION_IDS.createWorkspace]: {
								directoryId: 'missing-directory',
								projectId: 'missing-project',
								resourceScopeId: 'missing-scope',
							},
							[WORKBENCH_ACTION_IDS.createProject]: {
								path: '/workspace/orchbee',
							},
							[WORKBENCH_ACTION_IDS.deleteWorkspace]: {
								projectId: 'missing-project',
								workspaceId: 'missing-workspace',
							},
							[WORKBENCH_ACTION_IDS.duplicateWorkspace]: {
								projectId: 'missing-project',
								workspaceId: 'missing-workspace',
							},
							[WORKBENCH_ACTION_IDS.removeProject]: {
								projectId: 'missing-project',
							},
							[WORKBENCH_ACTION_IDS.reorderProject]: {
								beforeProjectId: null,
								projectId: 'missing-project',
							},
							[WORKBENCH_ACTION_IDS.reorderWorkspace]: {
								beforeWorkspaceId: null,
								projectId: 'missing-project',
								workspaceId: 'missing-workspace',
							},
							[WORKBENCH_ACTION_IDS.removeResourceDirectory]: {
								directoryId: 'missing-directory',
								resourceScopeId: 'missing-project',
							},
							[WORKBENCH_ACTION_IDS.renameProject]: {
								name: 'Missing Project',
								projectId: 'missing-project',
							},
							[WORKBENCH_ACTION_IDS.setTabStripOrientation]: {
								orientation: 'horizontal',
							},
							[WORKBENCH_ACTION_IDS.focusActivePanelByPosition]: {
								position: 1,
							},
							[WORKBENCH_ACTION_IDS.selectTabByPosition]: {
								position: 1,
							},
							[WORKBENCH_ACTION_IDS.setResourcePrimaryDirectory]: {
								directoryId: 'missing-directory',
								resourceScopeId: 'missing-project',
							},
							[WORKBENCH_ACTION_IDS.toggleProjectExpansion]: {
								projectId: 'missing-project',
							},
							[WORKBENCH_ACTION_IDS.updateWorkspace]: {
								name: 'Missing Workspace',
								primaryDirectoryId: 'missing-directory',
								projectId: 'missing-project',
								workspaceId: 'missing-workspace',
							},
						},
					)
					.map((action) => action.descriptor.id),
			),
		).toEqual(
			new Set(
				Object.values(WORKBENCH_ACTION_IDS).filter(
					(actionId) =>
						actionId !== WORKBENCH_ACTION_IDS.navigateAgent &&
						actionId !== WORKBENCH_ACTION_IDS.closeWorkspaceConfirmed &&
						// Worktree Actions exist only where an authority reaches Git.
						!actionId.startsWith('workbench.worktree.'),
				),
			),
		);

		fireEvent.click(screen.getByRole('button', { name: 'create' }));
		fireEvent.click(screen.getByRole('button', { name: 'toggle' }));

		expect(onNewTerminal).toHaveBeenCalledOnce();
		expect(onNewTerminal).toHaveBeenCalledWith(expect.any(Object), { groupId: null });
		expect(screen.getByText('hidden')).toBeDefined();
		await act(async () => undefined);
	});

	it('captures the exact displayed terminal when creating a Tab in its current directory', async () => {
		const runtime = new ActionRuntime();
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createTestTerminalPresentationComposition('current-directory-action'),
		});
		const targetResolution = projectWorkspaceRuntime.resolveActiveTerminalCurrentDirectoryTarget();
		if (targetResolution.kind !== 'ready') throw new Error('Expected a current-directory target.');
		const onNewTerminal = vi.fn<WorkbenchTerminalCreateCallback>(() => ({ kind: 'completed' }));
		let actionScope: ReturnType<ActionRuntime['createScope']> | undefined;
		render(
			React.createElement(WorkbenchActionsHarness, {
				captureScope: (scope) => {
					actionScope = scope;
				},
				onNewTerminal,
				projectWorkspaceRuntime,
				runtime,
			}),
		);
		if (actionScope === undefined) throw new Error('Expected the Workbench action scope.');
		const invocation = {
			scope: actionScope,
			source: 'context-menu' as const,
			target: { id: targetResolution.target.panelId, type: 'panel' as const },
		};
		expect(
			runtime.resolveAction(
				WORKBENCH_ACTION_IDS.createTerminalInCurrentDirectory,
				undefined,
				invocation,
			)?.availability,
		).toEqual({ kind: 'enabled' });

		await runtime.execute(
			WORKBENCH_ACTION_IDS.createTerminalInCurrentDirectory,
			undefined,
			invocation,
		).completion;

		expect(onNewTerminal).toHaveBeenCalledWith(expect.any(Object), {
			currentDirectoryTarget: targetResolution.target,
			groupId: null,
		});
	});

	it('keeps missing host capabilities disabled for shortcut resolution', () => {
		const runtime = new ActionRuntime();
		let capturedScope: ReturnType<typeof runtime.createScope> | undefined;
		const CapturingHarness = () => {
			const [projectWorkspaceRuntime] = React.useState(
				() =>
					new WorkbenchProjectWorkspaceRuntime({
						standaloneCwd: '/system',
						standaloneTerminalTitle: 'Terminal 1',
						terminalPresentation: createTestTerminalPresentationComposition('action-disabled'),
					}),
			);
			const actions = useWorkbenchActions({
				focusRouting: unavailableFocusRouting,
				isSidebarVisible: true,
				onNewTerminal: undefined,
				onSettings: undefined,
				openSearch: () => undefined,
				openTabTitleEditor: () => undefined,
				defaultWorkspaceName: 'Workspace',
				primaryWorkspaceName: 'Primary',
				projectWorkspaceRuntime,
				projectWorkspaceState: projectWorkspaceRuntime.getSnapshot(),
				runtime,
				setTabStripOrientation: () => undefined,
				tabStripOrientation: 'horizontal',
				toggleSidebar: () => undefined,
				workspaceCopySuffix: 'Copy',
				workspaceTerminalTitle: 'Terminal 1',
				worktreeLabel: worktreeTestLabel,
			});
			capturedScope = actions.scope;
			return null;
		};
		render(React.createElement(CapturingHarness));
		expect(capturedScope).toBeDefined();
		if (capturedScope === undefined) {
			throw new Error('Expected the Workbench action scope to be captured');
		}

		expect(
			runtime.resolveAction(WORKBENCH_ACTION_IDS.createTerminal, undefined, {
				scope: capturedScope,
				source: 'keyboard',
			}),
		).toMatchObject({
			availability: { kind: 'disabled' },
		});
	});

	it('consumes region shortcuts only for resolved targets and preserves direct-focus no-ops', async () => {
		const runtime = new ActionRuntime();
		type TestFocusRouting = Parameters<typeof useWorkbenchActions>[0]['focusRouting'];
		const focusRouting: TestFocusRouting = {
			beginTerminalHandover: unavailableFocusRouting.beginTerminalHandover,
			focus: vi.fn<TestFocusRouting['focus']>((intent) =>
				intent.kind === 'cycle' && intent.direction === 'next'
					? ({ kind: 'target', region: 'tab-strip' } as const)
					: ({ kind: 'unavailable', reason: 'target-unavailable' } as const),
			),
			getResolution: vi.fn<TestFocusRouting['getResolution']>((intent) => {
				if (intent.kind === 'direct' && intent.region === 'terminal') {
					return { kind: 'unchanged', region: 'terminal' } as const;
				}
				return intent.kind === 'cycle' && intent.direction === 'next'
					? ({ kind: 'target', region: 'tab-strip' } as const)
					: ({ kind: 'unavailable', reason: 'no-different-region' } as const);
			}),
		};
		let actions: ReturnType<typeof useWorkbenchActions> | undefined;
		render(
			React.createElement(WorkbenchActionsHarness, {
				captureActions: (candidate) => {
					actions = candidate;
				},
				focusRouting,
				runtime,
			}),
		);
		if (actions === undefined) throw new Error('Expected Workbench actions to be captured.');
		const scope = actions.scope;

		expect(
			runtime.resolveAction(WORKBENCH_ACTION_IDS.focusNextRegion, undefined, {
				scope,
				source: 'keyboard',
			})?.availability,
		).toEqual({ kind: 'enabled' });
		expect(
			runtime.resolveAction(WORKBENCH_ACTION_IDS.focusPreviousRegion, undefined, {
				scope,
				source: 'keyboard',
			})?.availability,
		).toMatchObject({ kind: 'disabled' });

		await expect(
			runtime.execute(WORKBENCH_ACTION_IDS.focusNextRegion, undefined, {
				scope,
				source: 'keyboard',
			}).completion,
		).resolves.toEqual({ kind: 'completed' });
		expect(focusRouting.focus).toHaveBeenCalledWith({ direction: 'next', kind: 'cycle' });

		await expect(
			runtime.execute(WORKBENCH_ACTION_IDS.focusActiveTerminal, undefined, {
				scope,
				source: 'command-surface',
			}).completion,
		).resolves.toEqual({ kind: 'unchanged' });
		expect(focusRouting.focus).toHaveBeenCalledOnce();

		expect(
			actions.invokeShortcut(
				{
					action: 'focusPreviousRegion',
					chord: WORKBENCH_SHORTCUTS.focusPreviousRegion.defaults.standard,
					shortcutId: WORKBENCH_SHORTCUTS.focusPreviousRegion.shortcutId,
				},
				'keyboard',
			),
		).toBe(false);
		expect(
			actions.invokeShortcut(
				{
					action: 'focusNextRegion',
					chord: WORKBENCH_SHORTCUTS.focusNextRegion.defaults.standard,
					shortcutId: WORKBENCH_SHORTCUTS.focusNextRegion.shortcutId,
				},
				'keyboard',
			),
		).toBe(true);
		await act(async () => undefined);
		expect(focusRouting.focus).toHaveBeenCalledTimes(2);
	});

	it('routes the three terminal font-size actions through one host capability', async () => {
		const runtime = new ActionRuntime();
		const changes: string[] = [];
		const onTerminalFontSizeChange = vi.fn<WorkbenchTerminalFontSizeChangeCallback>(
			(context, change) => {
				changes.push(change);
				context.markCommitted();
				return { kind: 'completed' };
			},
		);
		let actionScope: ReturnType<ActionRuntime['createScope']> | undefined;
		render(
			React.createElement(WorkbenchActionsHarness, {
				captureScope: (scope) => {
					actionScope = scope;
				},
				onTerminalFontSizeChange,
				runtime,
			}),
		);
		if (actionScope === undefined) throw new Error('Expected the Workbench action scope.');

		for (const actionId of [
			WORKBENCH_ACTION_IDS.increaseTerminalFontSize,
			WORKBENCH_ACTION_IDS.decreaseTerminalFontSize,
			WORKBENCH_ACTION_IDS.resetTerminalFontSize,
		]) {
			await runtime.execute(actionId, undefined, {
				scope: actionScope,
				source: 'keyboard',
			}).completion;
		}

		expect(changes).toEqual(['increase', 'decrease', 'reset']);
		expect(onTerminalFontSizeChange).toHaveBeenCalledTimes(3);
	});

	it('routes Project and Workspace intents through stable actions', async () => {
		const runtime = new ActionRuntime({ confirm: () => true });
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createTestTerminalPresentationComposition('action-confirmation'),
		});
		projectWorkspaceRuntime.transition({
			directory: {
				available: true,
				git: null,
				id: 'directory-1',
				kind: 'folder',
				name: 'orchbee',
				path: '/workspace/orchbee',
				placement: { kind: 'local' },
				source: 'selected',
			},
			launchConfigurations: [],
			primaryWorkspaceLaunchConfigurationId: null,
			primaryWorkspaceName: 'Primary',
			projectName: 'orchbee',
			terminalTitle: 'Terminal 1',
			type: 'project-created',
		});
		projectWorkspaceRuntime.transition({
			directory: {
				available: true,
				git: null,
				id: 'directory-2',
				kind: 'folder',
				name: 'xterm.js',
				path: '/workspace/xterm.js',
				placement: { kind: 'local' },
				source: 'selected',
			},
			resourceScopeId: 'resource-scope-1',
			type: 'directory-added',
		});
		let actionScope: ReturnType<ActionRuntime['createScope']> | undefined;
		render(
			React.createElement(WorkbenchActionsHarness, {
				captureScope: (scope) => {
					actionScope = scope;
				},
				projectWorkspaceRuntime,
				runtime,
			}),
		);
		if (actionScope === undefined) {
			throw new Error('Expected the Workbench action scope to be captured');
		}
		const projectActionScope = actionScope;

		await act(async () => {
			await runtime.execute(WORKBENCH_ACTION_IDS.activateStandalone, undefined, {
				scope: projectActionScope,
				source: 'button',
			}).completion;
		});
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({ kind: 'standalone' });

		await act(async () => {
			await runtime.execute(
				WORKBENCH_ACTION_IDS.removeResourceDirectory,
				{ directoryId: 'directory-1', resourceScopeId: 'resource-scope-1' },
				{ scope: projectActionScope, source: 'button' },
			).completion;
		});
		expect(projectWorkspaceRuntime.getSnapshot().projects[0]).toMatchObject({ name: 'orchbee' });
		expect(projectWorkspaceRuntime.getSnapshot().resourceScopes[0]).toMatchObject({
			primaryDirectoryId: 'directory-2',
		});
		expect(
			projectWorkspaceRuntime
				.getSnapshot()
				.workspaces.filter(
					(workspace) =>
						workspace.projectId === projectWorkspaceRuntime.getSnapshot().projects[0]?.id,
				),
		).toMatchObject([{ primaryDirectoryId: 'directory-2' }]);

		await act(async () => {
			await runtime.execute(
				WORKBENCH_ACTION_IDS.renameProject,
				{ name: 'Orchbee Project', projectId: 'project-1' },
				{ scope: projectActionScope, source: 'button' },
			).completion;
		});
		expect(projectWorkspaceRuntime.getSnapshot().projects[0]?.name).toBe('Orchbee Project');

		await act(async () => {
			await runtime.execute(
				WORKBENCH_ACTION_IDS.createWorkspace,
				{ directoryId: 'directory-2', resourceScopeId: 'resource-scope-1', projectId: 'project-1' },
				{ scope: projectActionScope, source: 'menu' },
			).completion;
		});
		expect(
			projectWorkspaceRuntime
				.getSnapshot()
				.workspaces.filter(
					(workspace) =>
						workspace.projectId === projectWorkspaceRuntime.getSnapshot().projects[0]?.id,
				),
		).toHaveLength(2);
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toMatchObject({
			kind: 'workspace',
			workspaceId: 'workspace-2',
		});

		await act(async () => {
			await runtime.execute(
				WORKBENCH_ACTION_IDS.updateWorkspace,
				{
					name: 'Feature',
					primaryDirectoryId: 'directory-2',

					workspaceId: 'workspace-2',
				},
				{ scope: projectActionScope, source: 'button' },
			).completion;
		});

		await act(async () => {
			await runtime.execute(
				WORKBENCH_ACTION_IDS.duplicateWorkspace,
				{ workspaceId: 'workspace-2' },
				{ scope: projectActionScope, source: 'menu' },
			).completion;
		});
		expect(
			projectWorkspaceRuntime
				.getSnapshot()
				.workspaces.filter(
					(workspace) =>
						workspace.projectId === projectWorkspaceRuntime.getSnapshot().projects[0]?.id,
				)[2]?.name,
		).toBe('Feature Copy');

		await act(async () => {
			await runtime.execute(
				WORKBENCH_ACTION_IDS.deleteWorkspace,
				{ workspaceId: 'workspace-3' },
				{ scope: projectActionScope, source: 'menu' },
			).completion;
		});
		expect(
			projectWorkspaceRuntime
				.getSnapshot()
				.workspaces.filter(
					(workspace) =>
						workspace.projectId === projectWorkspaceRuntime.getSnapshot().projects[0]?.id,
				),
		).toHaveLength(2);
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({ kind: 'standalone' });
	});

	it('passes cancellation and terminal-result ownership to an asynchronous host operation', async () => {
		const runtime = new ActionRuntime();
		let actionScope: ReturnType<ActionRuntime['createScope']> | undefined;
		let context: Parameters<WorkbenchTerminalCreateCallback>[0] | undefined;
		let settle:
			| ((result: Awaited<ReturnType<WorkbenchTerminalCreateCallback>>) => void)
			| undefined;
		const onNewTerminal: WorkbenchTerminalCreateCallback = (candidateContext) => {
			context = candidateContext;
			return new Promise((resolve) => {
				settle = resolve;
			});
		};
		render(
			React.createElement(WorkbenchActionsHarness, {
				captureScope: (scope) => {
					actionScope = scope;
				},
				onNewTerminal,
				runtime,
			}),
		);
		if (actionScope === undefined) {
			throw new Error('Expected the Workbench action scope to be captured');
		}

		const execution = runtime.execute(
			WORKBENCH_ACTION_IDS.createTerminal,
			{ groupId: null },
			{ scope: actionScope, source: 'button' },
		);
		expect(execution.getSnapshot().state).toBe('running');
		await expect(
			runtime.execute(
				WORKBENCH_ACTION_IDS.createTerminal,
				{ groupId: null },
				{ scope: actionScope, source: 'keyboard' },
			).completion,
		).resolves.toEqual({ code: 'duplicate-action-execution', kind: 'blocked' });
		execution.cancel();

		expect(context?.signal.aborted).toBe(true);
		expect(execution.getSnapshot().state).toBe('cancelling');
		settle?.({ kind: 'cancelled' });
		await expect(execution.completion).resolves.toEqual({ kind: 'cancelled' });
	});

	it('preserves a rejected host operation as the terminal Action failure', async () => {
		const runtime = new ActionRuntime();
		let actionScope: ReturnType<ActionRuntime['createScope']> | undefined;
		render(
			React.createElement(WorkbenchActionsHarness, {
				captureScope: (scope) => {
					actionScope = scope;
				},
				onNewTerminal: () => Promise.reject(new Error('terminal creation failed')),
				runtime,
			}),
		);
		if (actionScope === undefined) {
			throw new Error('Expected the Workbench action scope to be captured');
		}

		await expect(
			runtime.execute(WORKBENCH_ACTION_IDS.createTerminal, undefined, {
				scope: actionScope,
				source: 'button',
			}).completion,
		).resolves.toEqual({
			failure: { code: 'unexpected-error' },
			kind: 'failed',
		});
	});
});
