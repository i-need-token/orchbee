import type { ActionExecutionContext, ActionResult } from '@orchbee/actions';
import { describe, expect, it, vi } from 'vitest';

import {
	createSnapshotRuntime,
	fixtureContextId,
	fixturePresentationId,
	fixtureProjectId,
	fixtureTabId,
	fixtureWorkspaceId,
	publishSnapshot,
	snapshotContext,
	snapshotDirectory,
	snapshotProject,
	snapshotTab,
	snapshotWorkspace,
	workbenchSnapshot,
} from '../../__tests__/server-workbench-snapshot';
import type { WorkbenchProjectCreationReceipt } from '../../workbench';
import { WorkbenchRouteCoordinator } from '../WorkbenchRouteCoordinator';

type WorkbenchRouteNavigation = ConstructorParameters<typeof WorkbenchRouteCoordinator>[1];

const WORKSPACE_1 = fixtureWorkspaceId(1);
const WORKSPACE_2 = fixtureWorkspaceId(2);
const TAB_1 = fixtureTabId(1);
const TAB_2 = fixtureTabId(2);
const TAB_3 = fixtureTabId(3);

/** Workspace 1 holds Tabs 1 and 2; Workspace 2, in Project 1, holds Tab 3. */
function routeSnapshot(revision: number, options: { readonly withoutTab2?: boolean } = {}) {
	return workbenchSnapshot(
		revision,
		{
			directories: [snapshotDirectory(1), snapshotDirectory(2)],
			projects: [snapshotProject(1)],
			workspaces: [snapshotWorkspace(1), snapshotWorkspace(2, { projectId: fixtureProjectId(1) })],
		},
		[
			snapshotContext(
				1,
				options.withoutTab2 === true ? [snapshotTab(1)] : [snapshotTab(1), snapshotTab(2)],
			),
			snapshotContext(2, [snapshotTab(3)]),
		],
	);
}

function createRuntime() {
	return createSnapshotRuntime(routeSnapshot(1));
}

function createActionContext(signal = new AbortController().signal): ActionExecutionContext {
	return {
		executionId: 'route-action',
		markCommitted: vi.fn<ActionExecutionContext['markCommitted']>(),
		reportProgress: vi.fn<ActionExecutionContext['reportProgress']>(),
		scope: { id: 'test', parent: null },
		signal,
		source: 'button',
	};
}

describe('WorkbenchRouteCoordinator', () => {
	it('keeps created-Project activation pending until its exact delayed prepare is accepted', async () => {
		const fixture = createProjectCreationRoute();
		const settled = vi.fn<(result: ActionResult<void>) => void>();
		const result = Promise.resolve(
			fixture.coordinator.navigation.navigate(fixture.context, fixture.target, fixture.receipt),
		);
		void result.then(settled);
		await Promise.resolve();
		expect(fixture.context.markCommitted).toHaveBeenCalledOnce();
		expect(fixture.navigate).toHaveBeenCalledExactlyOnceWith(fixture.location, {
			replace: false,
		});
		expect(settled).not.toHaveBeenCalled();
		expect(fixture.coordinator.navigation.isActive?.(fixture.target)).toBe(false);
		fixture.coordinator.prepare(fixture.location);
		await expect(result).resolves.toEqual({ kind: 'completed' });
		expect(fixture.coordinator.navigation.isActive?.(fixture.target)).toBe(true);
		fixture.coordinator.dispose();
	});

	it.each(['retire', 'replace', 'dispose', 'timeout', 'remove-target'] as const)(
		'releases created-Project acceptance on %s and never relabels a late prepare as its success',
		async (change) => {
			vi.useFakeTimers();
			const fixture = createProjectCreationRoute();
			try {
				const result = Promise.resolve(
					fixture.coordinator.navigation.navigate(fixture.context, fixture.target, fixture.receipt),
				);
				if (change === 'retire') fixture.lifetime.abort();
				else if (change === 'replace')
					fixture.coordinator.navigation.navigate(createActionContext(), { kind: 'standalone' });
				else if (change === 'dispose') fixture.coordinator.dispose();
				else if (change === 'timeout') await vi.advanceTimersByTimeAsync(10_000);
				else {
					publishSnapshot(
						fixture.runtime,
						workbenchSnapshot(
							2,
							{ directories: [snapshotDirectory(1)], workspaces: [snapshotWorkspace(1)] },
							[snapshotContext(1, [snapshotTab(1), snapshotTab(2)])],
						),
					);
					fixture.coordinator.prepare(fixture.location);
				}
				await expect(result).resolves.toMatchObject({ kind: 'blocked' });
				expect(vi.getTimerCount()).toBe(0);
				if (change !== 'dispose') fixture.coordinator.prepare(fixture.location);
				await expect(result).resolves.toMatchObject({ kind: 'blocked' });
			} finally {
				fixture.coordinator.dispose();
				vi.useRealTimers();
			}
		},
	);

	it('reports only an accepted exact route, not the current model or a pending navigation request', () => {
		const runtime = createRuntime();
		const coordinator = new WorkbenchRouteCoordinator(runtime, vi.fn<WorkbenchRouteNavigation>());
		const target = { kind: 'workspace', workspaceId: WORKSPACE_2 } as const;
		expect(runtime.getSnapshot().activeLocation).toEqual(target);
		expect(coordinator.navigation.isActive?.(target)).toBe(false);
		coordinator.prepare({ kind: 'standalone' });
		coordinator.navigation.navigate(createActionContext(), target);
		expect(coordinator.navigation.isActive?.(target)).toBe(false);
		coordinator.prepare({ ...target, tabId: TAB_3 });
		expect(coordinator.navigation.isActive?.(target)).toBe(true);
		expect(coordinator.navigation.isActive?.({ ...target, workspaceId: WORKSPACE_1 })).toBe(false);
		coordinator.dispose();
		expect(coordinator.navigation.isActive?.(target)).toBe(false);
	});

	it('applies a route atomically without mirroring its own Client View writes', () => {
		const runtime = createRuntime();
		const navigate = vi.fn<WorkbenchRouteNavigation>();
		const coordinator = new WorkbenchRouteCoordinator(runtime, navigate);

		expect(
			coordinator.prepare({ kind: 'workspace', tabId: TAB_2, workspaceId: WORKSPACE_1 }),
		).toMatchObject({ kind: 'ready' });

		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().activeTabId).toBe(TAB_2);
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toHaveLength(2);
		expect(navigate).not.toHaveBeenCalled();
		coordinator.dispose();
	});

	it('changes the URL before applying a location-navigation target', () => {
		const runtime = createRuntime();
		const navigate = vi.fn<WorkbenchRouteNavigation>();
		const coordinator = new WorkbenchRouteCoordinator(runtime, navigate);
		coordinator.prepare({ kind: 'workspace', tabId: TAB_2, workspaceId: WORKSPACE_1 });

		expect(coordinator.navigation.navigate(createActionContext(), { kind: 'standalone' })).toEqual({
			kind: 'completed',
		});

		expect(navigate).toHaveBeenCalledWith({ kind: 'standalone', tabId: null }, { replace: false });
		expect(runtime.getSnapshot().activeLocation).toMatchObject({ kind: 'workspace' });
		coordinator.prepare({ kind: 'standalone' });
		expect(runtime.getSnapshot().activeLocation).toEqual({ kind: 'standalone' });
		coordinator.dispose();
	});

	it('preserves a delegated multi-selection mode after the target route is observed', () => {
		const runtime = createRuntime();
		const navigate = vi.fn<WorkbenchRouteNavigation>();
		const coordinator = new WorkbenchRouteCoordinator(runtime, navigate);
		coordinator.prepare({ kind: 'workspace', tabId: TAB_1, workspaceId: WORKSPACE_1 });

		coordinator.navigation.selectTab(createActionContext(), { mode: 'extend', tabId: TAB_2 });
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().activeTabId).toBe(TAB_1);
		coordinator.prepare({ kind: 'workspace', tabId: TAB_2, workspaceId: WORKSPACE_1 });

		const selection = runtime.getActiveTerminalContext().tabModel.getSnapshot();
		expect(selection.activeTabId).toBe(TAB_2);
		expect(selection.selectedTabIds).toEqual(new Set([TAB_1, TAB_2]));
		coordinator.dispose();
	});

	it('keeps the active Tab route when toggling a different selected Tab off', () => {
		const runtime = createRuntime();
		const navigate = vi.fn<WorkbenchRouteNavigation>();
		const coordinator = new WorkbenchRouteCoordinator(runtime, navigate);
		coordinator.prepare({ kind: 'workspace', tabId: TAB_2, workspaceId: WORKSPACE_1 });
		const tabModel = runtime.getActiveTerminalContext().tabModel;
		tabModel.setSelection([TAB_1, TAB_2], TAB_2);

		expect(
			coordinator.navigation.selectTab(createActionContext(), { mode: 'toggle', tabId: TAB_1 }),
		).toEqual({ kind: 'completed' });

		const selection = tabModel.getSnapshot();
		expect(selection.activeTabId).toBe(TAB_2);
		expect(selection.selectedTabIds).toEqual(new Set([TAB_2]));
		expect(navigate).not.toHaveBeenCalled();
		coordinator.dispose();
	});

	it('pushes unported active navigation and replaces an invalidated displayed target', () => {
		const runtime = createRuntime();
		const navigate = vi.fn<WorkbenchRouteNavigation>();
		const coordinator = new WorkbenchRouteCoordinator(runtime, navigate);
		coordinator.prepare({ kind: 'workspace', tabId: TAB_1, workspaceId: WORKSPACE_1 });

		runtime.getActiveTerminalContext().tabModel.selectTab(TAB_2);
		expect(navigate).toHaveBeenLastCalledWith(
			{ kind: 'workspace', tabId: TAB_2, workspaceId: WORKSPACE_1 },
			{ replace: false },
		);

		coordinator.prepare({ kind: 'workspace', tabId: TAB_2, workspaceId: WORKSPACE_1 });
		publishSnapshot(runtime, routeSnapshot(2, { withoutTab2: true }));
		expect(navigate).toHaveBeenLastCalledWith(
			{ kind: 'workspace', tabId: TAB_1, workspaceId: WORKSPACE_1 },
			{ replace: true },
		);
		coordinator.dispose();
	});
});

/** Workspace 1 is displayed while Workspace 2 is the Project the Server just created. */
function createProjectCreationRoute() {
	const runtime = createRuntime();
	const workspace = runtime.getSnapshot().workspaces.find((each) => each.id === WORKSPACE_2);
	const scope = runtime
		.getSnapshot()
		.resourceScopes.find((each) => each.id === workspace?.resourceScopeId);
	const directory = scope?.directories[0];
	if (workspace === undefined || directory === undefined)
		throw new Error('Expected the created Workspace and its Directory.');
	const receipt: WorkbenchProjectCreationReceipt = {
		directoryId: directory.id,
		path: directory.path,
		presentationId: fixturePresentationId(3),
		projectId: fixtureProjectId(1),
		resourceScopeId: workspace.resourceScopeId,
		terminalContextId: fixtureContextId(2),
		workspaceId: WORKSPACE_2,
	};
	const navigate = vi.fn<WorkbenchRouteNavigation>();
	const coordinator = new WorkbenchRouteCoordinator(runtime, navigate);
	coordinator.prepare({ kind: 'workspace', tabId: TAB_1, workspaceId: WORKSPACE_1 });
	const lifetime = new AbortController();
	const context = createActionContext(lifetime.signal);
	const target = { kind: 'workspace', workspaceId: WORKSPACE_2 } as const;
	return {
		context,
		coordinator,
		lifetime,
		location: { ...target, tabId: TAB_3 },
		navigate,
		receipt,
		runtime,
		target,
	};
}
