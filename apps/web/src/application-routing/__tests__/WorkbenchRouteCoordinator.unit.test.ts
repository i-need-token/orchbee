import {
	createWorkbenchPreviewTerminalPresentationComposition,
	terminalPresentationId,
	WorkbenchProjectWorkspaceRuntime,
} from '@orchbee/workbench';
import { describe, expect, it, vi } from 'vitest';

import { WorkbenchRouteCoordinator } from '../WorkbenchRouteCoordinator';

type WorkbenchRouteNavigation = ConstructorParameters<typeof WorkbenchRouteCoordinator>[1];

function createRuntime(): WorkbenchProjectWorkspaceRuntime {
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Standalone',
		terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
			terminalPresentationId('test:initial'),
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

describe('WorkbenchRouteCoordinator', () => {
	it('applies a route atomically without mirroring its own Client View writes', () => {
		const runtime = createRuntime();
		const navigate = vi.fn<WorkbenchRouteNavigation>(() => Promise.resolve());
		const coordinator = new WorkbenchRouteCoordinator(runtime, navigate);

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
		expect(navigate).not.toHaveBeenCalled();
		coordinator.dispose();
	});

	it('changes the URL before applying a location-navigation target', async () => {
		const runtime = createRuntime();
		const navigate = vi.fn<WorkbenchRouteNavigation>(() => Promise.resolve());
		const coordinator = new WorkbenchRouteCoordinator(runtime, navigate);
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
		const coordinator = new WorkbenchRouteCoordinator(runtime, navigate);
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
		const coordinator = new WorkbenchRouteCoordinator(runtime, navigate);
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
		const coordinator = new WorkbenchRouteCoordinator(runtime, navigate);
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
