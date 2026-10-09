import type { ActionExecutionContext } from '@orchbee/actions';
import {
	createWorkbenchPreviewTerminalPresentationComposition,
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
	WorkbenchProjectWorkspaceRuntime,
	type WorkbenchTerminalPresentationCommand,
} from '@orchbee/workbench';
import { describe, expect, it, vi } from 'vitest';

import { WorkbenchRouteCoordinator } from '../WorkbenchRouteCoordinator';

type WorkbenchRouteNavigation = ConstructorParameters<typeof WorkbenchRouteCoordinator>[1];

function createRuntime(): WorkbenchProjectWorkspaceRuntime {
	let presentationSequence = 0;
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Standalone',
		terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
			terminalPresentationId('test:initial'),
			() => terminalPresentationId(`test:dynamic:${(presentationSequence += 1)}`),
		),
	});
	runtime.transition({
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
		projectName: 'Orchbee',
		terminalTitle: 'Workspace',
		type: 'project-created',
	});
	runtime.createTerminal({ groupId: null, source: 'button', title: 'Workspace 2' });
	return runtime;
}

function createSealedEmptyRuntime(): WorkbenchProjectWorkspaceRuntime {
	let operationSequence = 1;
	let presentationSequence = 2;
	let removalSequence = 1;
	const commands: WorkbenchTerminalPresentationCommand[] = [];
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Standalone',
		terminalPresentation: {
			createOperationId: () =>
				terminalPresentationOperationId(`route:operation:${operationSequence++}`),
			createPresentationId: () =>
				terminalPresentationId(`route:presentation:${presentationSequence++}`),
			createRemovalId: () => terminalPresentationRemovalId(`route:removal:${removalSequence++}`),
			dispatchCommands: (nextCommands) => commands.push(...nextCommands),
			initialPresentationId: terminalPresentationId('route:presentation:1'),
			kind: 'managed',
		},
	});
	const provision = commands.find((command) => command.type === 'terminal-provision-requested');
	if (provision?.type !== 'terminal-provision-requested') {
		throw new Error('Expected the initial terminal provision request.');
	}
	runtime.applyTerminalPresentationEvent({
		operationId: provision.operationId,
		presentationId: provision.presentationId,
		type: 'terminal-provision-completed',
	});
	runtime.requestActiveTerminalTabsClose(['terminal-context-1-tab-1']);
	const close = commands.find((command) => command.type === 'terminal-close-requested');
	if (close?.type !== 'terminal-close-requested') {
		throw new Error('Expected the initial terminal close request.');
	}
	runtime.applyTerminalPresentationEvent({
		operationId: close.operationId,
		presentationId: close.presentationId,
		type: 'terminal-close-completed',
	});
	runtime.requestAllTerminalSessionsStop();
	return runtime;
}

describe('WorkbenchRouteCoordinator', () => {
	it('keeps created-Project activation pending until its exact delayed prepare is accepted', async () => {
		const fixture = createProjectCreationRoute();
		const settled = vi.fn();
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
				else if (change === 'replace') fixture.coordinator.navigateTo({ kind: 'standalone' });
				else if (change === 'dispose') fixture.coordinator.dispose();
				else if (change === 'timeout') await vi.advanceTimersByTimeAsync(10_000);
				else {
					fixture.runtime.transition({
						type: 'project-removed',
						projectId: fixture.receipt.projectId,
					});
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
		const coordinator = new WorkbenchRouteCoordinator(runtime, vi.fn(), () => 'Terminal');
		const target = {
			kind: 'workspace',
			projectId: 'project-1',
			workspaceId: 'workspace-1',
		} as const;
		expect(runtime.getSnapshot().activeLocation).toEqual(target);
		expect(coordinator.navigation.isActive?.(target)).toBe(false);
		coordinator.prepare({ kind: 'standalone', tabId: 'terminal-context-1-tab-1' });
		coordinator.navigateTo(target);
		expect(coordinator.navigation.isActive?.(target)).toBe(false);
		coordinator.prepare({ ...target, tabId: 'terminal-context-2-tab-1' });
		expect(coordinator.navigation.isActive?.(target)).toBe(true);
		expect(coordinator.navigation.isActive?.({ ...target, projectId: 'other' })).toBe(false);
		coordinator.dispose();
		expect(coordinator.navigation.isActive?.(target)).toBe(false);
	});

	it('applies a route atomically without mirroring its own Client View writes', () => {
		const runtime = createRuntime();
		const navigate = vi.fn<WorkbenchRouteNavigation>(() => Promise.resolve());
		const coordinator = new WorkbenchRouteCoordinator(runtime, navigate, () => 'Terminal');

		expect(
			coordinator.prepare({
				kind: 'workspace',
				projectId: 'project-1',
				tabId: 'terminal-context-2-tab-1',
				workspaceId: 'workspace-1',
			}),
		).toMatchObject({ kind: 'ready' });

		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().activeTabId).toBe(
			'terminal-context-2-tab-1',
		);
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toHaveLength(2);
		expect(navigate).not.toHaveBeenCalled();
		coordinator.dispose();
	});

	it('creates one localized terminal in an initially empty standalone route and never recreates it', () => {
		const runtime = createRuntime();
		runtime.transition({ type: 'standalone-activated' });
		runtime.requestActiveTerminalTabsClose(['terminal-context-1-tab-1']);
		const createTerminal = vi.spyOn(runtime, 'createTerminal');
		const navigate = vi.fn<WorkbenchRouteNavigation>(() => Promise.resolve());
		const coordinator = new WorkbenchRouteCoordinator(
			runtime,
			navigate,
			() => 'Localized Terminal',
		);

		expect(coordinator.prepare({ kind: 'standalone' })).toEqual({
			kind: 'redirect',
			location: { kind: 'standalone', tabId: 'terminal-context-1-tab-2' },
		});
		expect(createTerminal).toHaveBeenCalledExactlyOnceWith({
			groupId: null,
			source: 'programmatic',
			title: 'Localized Terminal',
		});
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toEqual([
			expect.objectContaining({ id: 'terminal-context-1-tab-2' }),
		]);

		coordinator.prepare({ kind: 'standalone', tabId: 'terminal-context-1-tab-2' });
		runtime.requestActiveTerminalTabsClose(['terminal-context-1-tab-2']);
		expect(coordinator.prepare({ kind: 'standalone' })).toEqual({
			kind: 'ready',
			location: { kind: 'standalone', tabId: null },
		});
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toEqual([]);
		coordinator.dispose();
	});

	it('creates the initial terminal in the Workspace selected by the first ready route', () => {
		const runtime = createRuntime();
		runtime.requestActiveTerminalTabsClose([
			'terminal-context-2-tab-1',
			'terminal-context-2-tab-2',
		]);
		const createTerminal = vi.spyOn(runtime, 'createTerminal');
		const navigate = vi.fn<WorkbenchRouteNavigation>(() => Promise.resolve());
		const coordinator = new WorkbenchRouteCoordinator(
			runtime,
			navigate,
			() => 'Localized Terminal',
		);

		expect(
			coordinator.prepare({
				kind: 'workspace',
				projectId: 'project-1',
				workspaceId: 'workspace-1',
			}),
		).toEqual({
			kind: 'redirect',
			location: {
				kind: 'workspace',
				projectId: 'project-1',
				tabId: 'terminal-context-2-tab-3',
				workspaceId: 'workspace-1',
			},
		});
		expect(createTerminal).toHaveBeenCalledExactlyOnceWith({
			groupId: null,
			source: 'programmatic',
			title: 'Localized Terminal',
		});
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toEqual([
			expect.objectContaining({ id: 'terminal-context-2-tab-3' }),
		]);
		coordinator.dispose();
	});

	it('keeps an undecided context eligible after an unavailable route', () => {
		const runtime = createRuntime();
		runtime.requestActiveTerminalTabsClose([
			'terminal-context-2-tab-1',
			'terminal-context-2-tab-2',
		]);
		const navigate = vi.fn<WorkbenchRouteNavigation>(() => Promise.resolve());
		const coordinator = new WorkbenchRouteCoordinator(runtime, navigate, () => 'Terminal');

		expect(
			coordinator.prepare({
				kind: 'workspace',
				projectId: 'project-1',
				tabId: 'missing-tab',
				workspaceId: 'workspace-1',
			}),
		).toEqual({ kind: 'unavailable', reason: 'tab-not-found' });
		expect(
			coordinator.prepare({
				kind: 'workspace',
				projectId: 'project-1',
				workspaceId: 'workspace-1',
			}),
		).toMatchObject({ kind: 'redirect' });
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toHaveLength(1);
		coordinator.dispose();
	});

	it('creates a terminal on the first empty Workspace visit after standalone was already displayed', () => {
		const runtime = createRuntime();
		runtime.requestActiveTerminalTabsClose([
			'terminal-context-2-tab-1',
			'terminal-context-2-tab-2',
		]);
		const createTerminal = vi.spyOn(runtime, 'createTerminal');
		const navigate = vi.fn<WorkbenchRouteNavigation>(() => Promise.resolve());
		let localizedTitle = 'Terminal';
		const coordinator = new WorkbenchRouteCoordinator(runtime, navigate, () => localizedTitle);

		expect(
			coordinator.prepare({ kind: 'standalone', tabId: 'terminal-context-1-tab-1' }),
		).toMatchObject({ kind: 'ready' });
		localizedTitle = '终端';
		expect(
			coordinator.prepare({
				kind: 'workspace',
				projectId: 'project-1',
				workspaceId: 'workspace-1',
			}),
		).toMatchObject({ kind: 'redirect' });

		expect(createTerminal).toHaveBeenCalledExactlyOnceWith({
			groupId: null,
			source: 'programmatic',
			title: '终端',
		});
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toHaveLength(1);
		coordinator.dispose();
	});

	it('leaves an initially empty route blank and does not retry when creation is rejected', () => {
		const runtime = createSealedEmptyRuntime();
		const createTerminal = vi.spyOn(runtime, 'createTerminal');
		const navigate = vi.fn<WorkbenchRouteNavigation>(() => Promise.resolve());
		const coordinator = new WorkbenchRouteCoordinator(runtime, navigate, () => 'Terminal');

		expect(coordinator.prepare({ kind: 'standalone' })).toEqual({
			kind: 'ready',
			location: { kind: 'standalone', tabId: null },
		});
		expect(coordinator.prepare({ kind: 'standalone' })).toMatchObject({ kind: 'ready' });
		expect(createTerminal).toHaveBeenCalledOnce();
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toEqual([]);
		coordinator.dispose();
	});

	it('changes the URL before applying a location-navigation target', async () => {
		const runtime = createRuntime();
		const navigate = vi.fn<WorkbenchRouteNavigation>(() => Promise.resolve());
		const coordinator = new WorkbenchRouteCoordinator(runtime, navigate, () => 'Terminal');
		coordinator.prepare({
			kind: 'workspace',
			projectId: 'project-1',
			tabId: 'terminal-context-2-tab-2',
			workspaceId: 'workspace-1',
		});

		await coordinator.navigateTo({ kind: 'standalone' });

		expect(navigate).toHaveBeenCalledWith(
			{ kind: 'standalone', tabId: 'terminal-context-1-tab-1' },
			{ replace: false },
		);
		expect(runtime.getSnapshot().activeLocation).toMatchObject({ kind: 'workspace' });
		coordinator.prepare({ kind: 'standalone', tabId: 'terminal-context-1-tab-1' });
		expect(runtime.getSnapshot().activeLocation).toEqual({ kind: 'standalone' });
		coordinator.dispose();
	});

	it('preserves a delegated multi-selection mode after the target route is observed', async () => {
		const runtime = createRuntime();
		const navigate = vi.fn<WorkbenchRouteNavigation>(() => Promise.resolve());
		const coordinator = new WorkbenchRouteCoordinator(runtime, navigate, () => 'Terminal');
		coordinator.prepare({
			kind: 'workspace',
			projectId: 'project-1',
			tabId: 'terminal-context-2-tab-1',
			workspaceId: 'workspace-1',
		});

		await coordinator.navigateToTab({ mode: 'extend', tabId: 'terminal-context-2-tab-2' });
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().activeTabId).toBe(
			'terminal-context-2-tab-1',
		);
		coordinator.prepare({
			kind: 'workspace',
			projectId: 'project-1',
			tabId: 'terminal-context-2-tab-2',
			workspaceId: 'workspace-1',
		});

		const selection = runtime.getActiveTerminalContext().tabModel.getSnapshot();
		expect(selection.activeTabId).toBe('terminal-context-2-tab-2');
		expect(selection.selectedTabIds).toEqual(
			new Set(['terminal-context-2-tab-1', 'terminal-context-2-tab-2']),
		);
		coordinator.dispose();
	});

	it('keeps the active Tab route when toggling a different selected Tab off', () => {
		const runtime = createRuntime();
		const navigate = vi.fn<WorkbenchRouteNavigation>(() => Promise.resolve());
		const coordinator = new WorkbenchRouteCoordinator(runtime, navigate, () => 'Terminal');
		const tabModel = runtime.getActiveTerminalContext().tabModel;
		coordinator.prepare({
			kind: 'workspace',
			projectId: 'project-1',
			tabId: 'terminal-context-2-tab-2',
			workspaceId: 'workspace-1',
		});
		tabModel.setSelection(
			['terminal-context-2-tab-1', 'terminal-context-2-tab-2'],
			'terminal-context-2-tab-2',
		);

		expect(coordinator.navigateToTab({ mode: 'toggle', tabId: 'terminal-context-2-tab-1' })).toBe(
			'updated',
		);

		const selection = tabModel.getSnapshot();
		expect(selection.activeTabId).toBe('terminal-context-2-tab-2');
		expect(selection.selectedTabIds).toEqual(new Set(['terminal-context-2-tab-2']));
		expect(navigate).not.toHaveBeenCalled();
		coordinator.dispose();
	});

	it('pushes unported active navigation and replaces an invalidated displayed target', () => {
		const runtime = createRuntime();
		const navigate = vi.fn<WorkbenchRouteNavigation>(() => Promise.resolve());
		const coordinator = new WorkbenchRouteCoordinator(runtime, navigate, () => 'Terminal');
		coordinator.prepare({
			kind: 'workspace',
			projectId: 'project-1',
			tabId: 'terminal-context-2-tab-1',
			workspaceId: 'workspace-1',
		});
		const tabModel = runtime.getActiveTerminalContext().tabModel;

		tabModel.selectTab('terminal-context-2-tab-2');
		expect(navigate).toHaveBeenLastCalledWith(
			{
				kind: 'workspace',
				projectId: 'project-1',
				tabId: 'terminal-context-2-tab-2',
				workspaceId: 'workspace-1',
			},
			{ replace: false },
		);

		coordinator.prepare({
			kind: 'workspace',
			projectId: 'project-1',
			tabId: 'terminal-context-2-tab-2',
			workspaceId: 'workspace-1',
		});
		tabModel.closeTab('terminal-context-2-tab-2');
		expect(navigate).toHaveBeenLastCalledWith(
			{
				kind: 'workspace',
				projectId: 'project-1',
				tabId: 'terminal-context-2-tab-1',
				workspaceId: 'workspace-1',
			},
			{ replace: true },
		);
		coordinator.dispose();
	});
});

function createProjectCreationRoute() {
	const runtime = createRuntime();
	const creation = runtime.transition({
		directory: {
			available: true,
			git: null,
			id: 'new-folder',
			kind: 'folder',
			name: 'new',
			path: '/workspace/new',
			placement: { kind: 'local' },
			source: 'selected',
		},
		launchConfigurations: [],
		primaryWorkspaceLaunchConfigurationId: null,
		primaryWorkspaceName: 'Primary',
		projectName: 'new',
		terminalTitle: 'Terminal',
		type: 'project-created',
	});
	const receipt = creation.projectCreationReceipt;
	if (receipt === undefined) throw new Error('Expected an exact creation receipt.');
	const tabId = runtime.getTerminalContext(receipt.terminalContextId)?.tabModel.getSnapshot()
		.activeTabId;
	if (tabId === undefined || tabId === null) throw new Error('Expected the first terminal Tab.');
	const navigate = vi.fn<WorkbenchRouteNavigation>();
	const coordinator = new WorkbenchRouteCoordinator(runtime, navigate, () => 'Terminal');
	coordinator.prepare({ kind: 'standalone', tabId: 'terminal-context-1-tab-1' });
	const lifetime = new AbortController();
	const context: ActionExecutionContext = {
		executionId: 'project-creation-route',
		markCommitted: vi.fn(),
		reportProgress: vi.fn(),
		scope: { id: 'test', parent: null },
		signal: lifetime.signal,
		source: 'button',
	};
	const target = {
		kind: 'workspace',
		projectId: receipt.projectId,
		workspaceId: receipt.workspaceId,
	} as const;
	return {
		context,
		coordinator,
		lifetime,
		location: { ...target, tabId },
		navigate,
		receipt,
		runtime,
		target,
	};
}
