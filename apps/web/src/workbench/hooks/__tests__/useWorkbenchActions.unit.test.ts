// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import * as React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	createSnapshotRuntime,
	fixtureDirectoryId,
	fixturePresentationId,
	fixtureProjectId,
	fixtureWorkspaceId,
	oneWorkspaceSnapshot,
	snapshotContext,
	snapshotDirectory,
	snapshotProject,
	snapshotTab,
	snapshotWorkspace,
	workbenchSnapshot,
} from '../../../__tests__/server-workbench-snapshot';
import { inertWorkbenchAuthority } from '../../../__tests__/inertWorkbenchAuthority';
import { WORKBENCH_ACTION_IDS } from '../../actions/workbench-action-ids';
import type { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchAuthority } from '../../types/workbench-authority';
import type {
	WorkbenchNavigationPort,
	WorkbenchTerminalFontSizeChangeCallback,
	WorkbenchTerminalCreateCallback,
} from '../../types/workbench-host-actions';
import { WORKBENCH_SHORTCUTS } from '../../utils/workbench-shortcuts';
import { useWorkbenchActions } from '../useWorkbenchActions';

const unavailableFocusRouting = {
	beginTerminalHandover: () => ({ complete: () => undefined, held: () => true }),
	focus: () => ({ kind: 'unavailable', reason: 'no-available-region' }) as const,
	getResolution: () => ({ kind: 'unavailable', reason: 'no-available-region' }) as const,
};
const worktreeTestLabel = (ordinal: number) => `Worktree ${ordinal}`;
const closeWarningTestText = () => 'Ends its terminals.';

function createTerminalRuntime(): WorkbenchProjectWorkspaceRuntime {
	return createSnapshotRuntime(oneWorkspaceSnapshot(1, [snapshotTab(1)]));
}

/** Workspace 1 of Project 1 in Directory 1. */
function createProjectRuntime(): WorkbenchProjectWorkspaceRuntime {
	return createSnapshotRuntime(
		workbenchSnapshot(
			1,
			{
				directories: [snapshotDirectory(1)],
				projects: [snapshotProject(1, 'orchbee')],
				workspaces: [snapshotWorkspace(1, { label: 'Primary', projectId: fixtureProjectId(1) })],
			},
			[snapshotContext(1, [snapshotTab(1)])],
		),
	);
}

interface WorkbenchActionsHarnessProps {
	readonly authority?: WorkbenchAuthority | undefined;
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
	authority = inertWorkbenchAuthority(),
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
		() => providedProjectWorkspaceRuntime ?? createTerminalRuntime(),
	);
	const projectWorkspaceState = React.useSyncExternalStore(
		projectWorkspaceRuntime.subscribe,
		projectWorkspaceRuntime.getSnapshot,
		projectWorkspaceRuntime.getSnapshot,
	);
	const actions = useWorkbenchActions({
		authority,
		focusRouting,
		formatCloseWarning: closeWarningTestText,
		isSidebarVisible,
		navigation,
		onNewTerminal,
		onSettings: undefined,
		onTerminalFontSizeChange,
		openAgentBoard: () => undefined,
		openSearch: () => undefined,
		openTabTitleEditor: () => undefined,
		projectWorkspaceRuntime,
		projectWorkspaceState,
		runtime,
		setTabStripOrientation: () => undefined,
		tabStripOrientation: 'horizontal',
		toggleSidebar: () => setIsSidebarVisible((visible) => !visible),
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
		const projectWorkspaceRuntime = createSnapshotRuntime(
			workbenchSnapshot(
				1,
				{
					directories: [snapshotDirectory(1)],
					workspaces: [
						snapshotWorkspace(1),
						snapshotWorkspace(2, { primaryDirectoryId: fixtureDirectoryId(1) }),
					],
				},
				[snapshotContext(1, [snapshotTab(1)]), snapshotContext(2, [snapshotTab(2)])],
			),
		);
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

		await runtime.execute(
			WORKBENCH_ACTION_IDS.activateWorkspace,
			{ workspaceId: fixtureWorkspaceId(2) },
			{ scope: actionScope, source: 'button' },
		).completion;

		expect(navigate).toHaveBeenCalledWith(expect.anything(), {
			kind: 'workspace',
			workspaceId: fixtureWorkspaceId(2),
		});
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
			kind: 'workspace',
			workspaceId: fixtureWorkspaceId(1),
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
							[WORKBENCH_ACTION_IDS.toggleProjectExpansion]: {
								projectId: 'missing-project',
							},
							[WORKBENCH_ACTION_IDS.updateWorkspace]: {
								name: 'Missing Workspace',
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
						// Only the authority's answer names the Tab this asks about.
						actionId !== WORKBENCH_ACTION_IDS.closeTabConfirmed &&
						// Only an authority's close can stop where Resume continues it.
						actionId !== WORKBENCH_ACTION_IDS.resumeTabClose &&
						// Only an authority that reads the Server's Agent sessions has a board.
						actionId !== WORKBENCH_ACTION_IDS.openAgentBoard &&
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
		const projectWorkspaceRuntime = createTerminalRuntime();
		projectWorkspaceRuntime.applyTerminalMetadataObservation(
			terminalPresentationId(fixturePresentationId(1)),
			{
				currentDirectory: { currentCwd: '/workspace/orchbee', kind: 'reported' },
				naturalTitle: null,
			},
		);
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
			const [projectWorkspaceRuntime] = React.useState(createTerminalRuntime);
			const actions = useWorkbenchActions({
				authority: inertWorkbenchAuthority(),
				focusRouting: unavailableFocusRouting,
				formatCloseWarning: closeWarningTestText,
				isSidebarVisible: true,
				onNewTerminal: undefined,
				onSettings: undefined,
				openAgentBoard: () => undefined,
				openSearch: () => undefined,
				openTabTitleEditor: () => undefined,
				projectWorkspaceRuntime,
				projectWorkspaceState: projectWorkspaceRuntime.getSnapshot(),
				runtime,
				setTabStripOrientation: () => undefined,
				tabStripOrientation: 'horizontal',
				toggleSidebar: () => undefined,
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
		const projectWorkspaceRuntime = createProjectRuntime();
		const before = projectWorkspaceRuntime.getSnapshot();
		const workspace = before.workspaces[0];
		const directory = workspace?.directory;
		if (workspace === undefined || directory === undefined || directory === null) {
			throw new Error('Expected one Workspace in a Folder.');
		}
		const change = vi.fn<WorkbenchAuthority['change']>(() =>
			Promise.resolve({ kind: 'completed' }),
		);
		const close = vi.fn<WorkbenchAuthority['close']>(() => Promise.resolve({ kind: 'completed' }));
		const create = vi.fn<WorkbenchAuthority['create']>(() =>
			Promise.resolve({ kind: 'cancelled' }),
		);
		let actionScope: ReturnType<ActionRuntime['createScope']> | undefined;
		render(
			React.createElement(WorkbenchActionsHarness, {
				authority: inertWorkbenchAuthority({ change, close, create }),
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
		const execute = async (actionId: string, input: unknown) => {
			await act(async () => {
				await runtime.execute(actionId, input, { scope: projectActionScope, source: 'button' })
					.completion;
			});
		};

		await execute(WORKBENCH_ACTION_IDS.renameProject, {
			name: 'Orchbee Project',
			projectId: fixtureProjectId(1),
		});
		await execute(WORKBENCH_ACTION_IDS.createWorkspace, {
			directoryId: directory.id,
			projectId: fixtureProjectId(1),
		});
		await execute(WORKBENCH_ACTION_IDS.updateWorkspace, {
			name: 'Feature',
			workspaceId: workspace.id,
		});
		await execute(WORKBENCH_ACTION_IDS.deleteWorkspace, { workspaceId: workspace.id });

		expect(change.mock.calls.map(([, request]) => request)).toEqual([
			{ name: 'Orchbee Project', projectId: fixtureProjectId(1), type: 'project-renamed' },
			{ name: 'Feature', type: 'workspace-updated', workspaceId: workspace.id },
		]);
		expect(create).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			directoryId: directory.id,
			projectId: fixtureProjectId(1),
			type: 'workspace-created',
		});
		expect(close).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			type: 'workspace-close-if-idle',
			workspaceId: workspace.id,
		});
		// Every change shows once the authority publishes it.
		expect(projectWorkspaceRuntime.getSnapshot()).toBe(before);
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
