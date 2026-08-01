import { describe, expect, it, vi } from 'vitest';

import type { WorkbenchDirectory } from '../../../project-workspace';
import { WorkbenchProjectWorkspaceRuntime } from '../WorkbenchProjectWorkspaceRuntime';

const DIRECTORY: WorkbenchDirectory = {
	available: true,
	git: null,
	id: 'directory-1',
	kind: 'folder',
	name: 'orchbee',
	path: '/projects/orchbee',
	source: 'selected',
};

const FEATURE_DIRECTORY: WorkbenchDirectory = {
	...DIRECTORY,
	id: 'directory-2',
	name: 'feature',
	path: '/projects/feature',
};

function createRuntime(): WorkbenchProjectWorkspaceRuntime {
	return new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Terminal 1',
	});
}

describe('WorkbenchProjectWorkspaceRuntime', () => {
	it('starts with one standalone Tab, Panel, and launch CWD', () => {
		const runtime = createRuntime();
		const context = runtime.getActiveTerminalContext();
		const tab = context.tabModel.getSnapshot().tabs[0];
		const panelWorkspace = context.panelWorkspaceStore.getSnapshot().tabs[0];

		expect(runtime.getSnapshot().activeLocation).toEqual({ kind: 'standalone' });
		expect(tab).toMatchObject({ id: 'terminal-context-1-tab-1', title: 'Terminal 1' });
		expect(panelWorkspace?.root).toMatchObject({
			kind: 'panel',
			primaryTerminal: {
				currentCwd: '/system',
				launchCwd: '/system',
				title: 'Terminal 1',
			},
		});
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
			primaryTerminal: {
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
			tabId: sourceTabId,
			type: 'panel-split',
		});
		source.panelWorkspaceStore.transition({
			currentCwd: '/projects/changed',
			tabId: sourceTabId,
			terminalId: 'terminal-1',
			type: 'terminal-current-cwd-changed',
		});
		source.panelWorkspaceStore.transition({
			phase: 'failed',
			tabId: sourceTabId,
			terminalId: 'terminal-1',
			type: 'terminal-phase-changed',
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
			title: 'Terminal 1',
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
						primaryTerminal: {
							currentCwd: '/projects/orchbee',
							id: 'terminal-context-3-terminal-1',
							launchCwd: '/projects/orchbee',
							phase: 'running',
						},
					},
				},
				{
					id: 'terminal-context-3-panel-slot-2',
					node: {
						id: 'terminal-context-3-panel-1',
						kind: 'panel',
						primaryTerminal: {
							currentCwd: '/projects/orchbee',
							id: 'terminal-context-3-terminal-2',
							launchCwd: '/projects/orchbee',
							phase: 'running',
						},
					},
				},
			],
			id: 'terminal-context-3-split-1',
		});
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
			primaryTerminal: {
				currentCwd: '/projects/feature',
				launchCwd: '/projects/feature',
				title: 'Feature',
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
