// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import * as React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { terminalPresentationId } from '../../../terminal-presentation';
import { WORKBENCH_ACTION_IDS } from '../../actions/workbench-action-ids';
import { createWorkbenchPreviewTerminalPresentationComposition } from '../../model/createWorkbenchPreviewTerminalPresentationComposition';
import type {
	WorkbenchNavigationPort,
	WorkbenchTerminalCreateCallback,
} from '../../types/workbench-host-actions';
import { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import { useWorkbenchActions } from '../useWorkbenchActions';

function createTestTerminalPresentationComposition(scope: string) {
	let sequence = 0;
	return createWorkbenchPreviewTerminalPresentationComposition(
		terminalPresentationId(`${scope}:initial`),
		() => terminalPresentationId(`${scope}:dynamic:${(sequence += 1)}`),
	);
}

interface WorkbenchActionsHarnessProps {
	readonly captureScope?: ((scope: ReturnType<ActionRuntime['createScope']>) => void) | undefined;
	readonly onNewTerminal?: WorkbenchTerminalCreateCallback | undefined;
	readonly navigation?: WorkbenchNavigationPort | undefined;
	readonly projectWorkspaceRuntime?: WorkbenchProjectWorkspaceRuntime | undefined;
	readonly runtime: ActionRuntime;
}

function WorkbenchActionsHarness({
	captureScope,
	navigation,
	onNewTerminal,
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
		isSidebarVisible,
		navigation,
		onNewTerminal,
		onSettings: undefined,
		openSearch: () => undefined,
		primaryWorkspaceName: 'Primary',
		projectWorkspaceRuntime,
		projectWorkspaceState,
		runtime,
		setTabStripOrientation: () => undefined,
		tabStripOrientation: 'horizontal',
		toggleSidebar: () => setIsSidebarVisible((visible) => !visible),
		workspaceCopySuffix: 'Copy',
		workspaceTerminalTitle: 'Terminal 1',
	});
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
			projectId: 'project-1',
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
							[WORKBENCH_ACTION_IDS.addProjectFolder]: {
								path: '/workspace/ghostty',
								projectId: 'missing-project',
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
							[WORKBENCH_ACTION_IDS.removeProjectDirectory]: {
								directoryId: 'missing-directory',
								projectId: 'missing-project',
							},
							[WORKBENCH_ACTION_IDS.renameProject]: {
								name: 'Missing Project',
								projectId: 'missing-project',
							},
							[WORKBENCH_ACTION_IDS.setTabStripOrientation]: {
								orientation: 'horizontal',
							},
							[WORKBENCH_ACTION_IDS.selectTabByPosition]: {
								position: 1,
							},
							[WORKBENCH_ACTION_IDS.setProjectPrimaryDirectory]: {
								directoryId: 'missing-directory',
								projectId: 'missing-project',
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
		).toEqual(new Set(Object.values(WORKBENCH_ACTION_IDS)));

		fireEvent.click(screen.getByRole('button', { name: 'create' }));
		fireEvent.click(screen.getByRole('button', { name: 'toggle' }));

		expect(onNewTerminal).toHaveBeenCalledOnce();
		expect(onNewTerminal).toHaveBeenCalledWith(expect.any(Object), { groupId: null });
		expect(screen.getByText('hidden')).toBeDefined();
		await act(async () => undefined);
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
				isSidebarVisible: true,
				onNewTerminal: undefined,
				onSettings: undefined,
				openSearch: () => undefined,
				primaryWorkspaceName: 'Primary',
				projectWorkspaceRuntime,
				projectWorkspaceState: projectWorkspaceRuntime.getSnapshot(),
				runtime,
				setTabStripOrientation: () => undefined,
				tabStripOrientation: 'horizontal',
				toggleSidebar: () => undefined,
				workspaceCopySuffix: 'Copy',
				workspaceTerminalTitle: 'Terminal 1',
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
				source: 'selected',
			},
			projectId: 'project-1',
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
				WORKBENCH_ACTION_IDS.removeProjectDirectory,
				{ directoryId: 'directory-1', projectId: 'project-1' },
				{ scope: projectActionScope, source: 'button' },
			).completion;
		});
		expect(projectWorkspaceRuntime.getSnapshot().projects[0]).toMatchObject({
			name: 'orchbee',
			primaryDirectoryId: 'directory-2',
			workspaces: [{ primaryDirectoryId: 'directory-2' }],
		});

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
				{ directoryId: 'directory-2', projectId: 'project-1' },
				{ scope: projectActionScope, source: 'menu' },
			).completion;
		});
		expect(projectWorkspaceRuntime.getSnapshot().projects[0]?.workspaces).toHaveLength(2);
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
					projectId: 'project-1',
					workspaceId: 'workspace-2',
				},
				{ scope: projectActionScope, source: 'button' },
			).completion;
		});

		await act(async () => {
			await runtime.execute(
				WORKBENCH_ACTION_IDS.duplicateWorkspace,
				{ projectId: 'project-1', workspaceId: 'workspace-2' },
				{ scope: projectActionScope, source: 'menu' },
			).completion;
		});
		expect(projectWorkspaceRuntime.getSnapshot().projects[0]?.workspaces[2]?.name).toBe(
			'Feature Copy',
		);

		await act(async () => {
			await runtime.execute(
				WORKBENCH_ACTION_IDS.deleteWorkspace,
				{ projectId: 'project-1', workspaceId: 'workspace-3' },
				{ scope: projectActionScope, source: 'menu' },
			).completion;
		});
		expect(projectWorkspaceRuntime.getSnapshot().projects[0]?.workspaces).toHaveLength(2);
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
