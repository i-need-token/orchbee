import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { describe, expect, it } from 'vitest';

import { terminalPresentationId } from '../../../terminal-presentation';
import { createWorkbenchPreviewTerminalPresentationComposition } from '../../model/createWorkbenchPreviewTerminalPresentationComposition';
import { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import { registerWorkbenchProjectWorkspaceActions } from '../registerWorkbenchProjectWorkspaceActions';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../workbench-action-ids';

function createTestTerminalPresentationComposition(scope: string) {
	let sequence = 0;
	return createWorkbenchPreviewTerminalPresentationComposition(
		terminalPresentationId(`${scope}:initial`),
		() => terminalPresentationId(`${scope}:dynamic:${(sequence += 1)}`),
	);
}

describe('registerWorkbenchProjectWorkspaceActions', () => {
	it('routes Workspace reorder through the scoped semantic Action', async () => {
		const actionRuntime = new ActionRuntime();
		const scope = actionRuntime.createScope('workbench');
		const projectWorkspaceRuntime = projectRuntime();
		projectWorkspaceRuntime.transition({
			directoryId: 'directory-1',
			launchConfigurationId: null,
			name: 'Feature',
			projectId: 'project-1',
			terminalTitle: 'Terminal 2',
			type: 'workspace-created',
		});
		const activeLocation = projectWorkspaceRuntime.getSnapshot().activeLocation;
		const firstContext = projectWorkspaceRuntime.getTerminalContext('terminal-context-2');
		const secondContext = projectWorkspaceRuntime.getTerminalContext('terminal-context-3');
		const registrations = registerWorkbenchProjectWorkspaceActions({
			primaryWorkspaceName: 'Primary',
			projectWorkspaceRuntime,
			scope,
			typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(actionRuntime),
			workspaceCopySuffix: 'Copy',
			workspaceTerminalTitle: 'Terminal 1',
		});

		await expect(
			actionRuntime.execute(
				WORKBENCH_ACTION_IDS.reorderWorkspace,
				{
					beforeWorkspaceId: 'workspace-1',
					projectId: 'project-1',
					workspaceId: 'workspace-2',
				},
				{ scope, source: 'pointer' },
			).completion,
		).resolves.toEqual({ kind: 'completed' });
		expect(
			projectWorkspaceRuntime
				.getSnapshot()
				.projects[0]?.workspaces.map((workspace) => workspace.id),
		).toEqual(['workspace-2', 'workspace-1']);
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toBe(activeLocation);
		expect(projectWorkspaceRuntime.getTerminalContext('terminal-context-2')).toBe(firstContext);
		expect(projectWorkspaceRuntime.getTerminalContext('terminal-context-3')).toBe(secondContext);

		for (const registration of registrations) registration.dispose();
	});

	it('reads current Workspace state across consecutive Actions and rejects stale reorder targets', async () => {
		const actionRuntime = new ActionRuntime();
		const scope = actionRuntime.createScope('workbench');
		const projectWorkspaceRuntime = projectRuntime();
		const registrations = registerWorkbenchProjectWorkspaceActions({
			primaryWorkspaceName: 'Primary',
			projectWorkspaceRuntime,
			scope,
			typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(actionRuntime),
			workspaceCopySuffix: 'Copy',
			workspaceTerminalTitle: 'Terminal 1',
		});
		const execute = <Input>(actionId: string, input: Input) =>
			actionRuntime.execute(actionId, input, { scope, source: 'programmatic' }).completion;

		await expect(
			execute(WORKBENCH_ACTION_IDS.reorderWorkspace, {
				beforeWorkspaceId: null,
				projectId: 'project-1',
				workspaceId: 'workspace-1',
			}),
		).resolves.toMatchObject({ code: 'action-unavailable', kind: 'blocked' });
		await expect(
			execute(WORKBENCH_ACTION_IDS.updateWorkspace, {
				name: 'Renamed',
				primaryDirectoryId: 'directory-2',
				projectId: 'project-1',
				workspaceId: 'workspace-1',
			}),
		).resolves.toEqual({ kind: 'completed' });
		await expect(
			execute(WORKBENCH_ACTION_IDS.duplicateWorkspace, {
				projectId: 'project-1',
				workspaceId: 'workspace-1',
			}),
		).resolves.toEqual({ kind: 'completed' });
		expect(projectWorkspaceRuntime.getSnapshot().projects[0]?.workspaces[1]?.name).toBe(
			'Renamed Copy',
		);
		await expect(
			execute(WORKBENCH_ACTION_IDS.reorderWorkspace, {
				beforeWorkspaceId: 'workspace-1',
				projectId: 'project-1',
				workspaceId: 'workspace-2',
			}),
		).resolves.toEqual({ kind: 'completed' });
		await expect(
			execute(WORKBENCH_ACTION_IDS.reorderWorkspace, {
				beforeWorkspaceId: 'workspace-2',
				projectId: 'project-1',
				workspaceId: 'workspace-2',
			}),
		).resolves.toEqual({ kind: 'unchanged' });

		await expect(
			execute(WORKBENCH_ACTION_IDS.createProject, { path: '/workspace/other' }),
		).resolves.toEqual({ kind: 'completed' });
		await expect(
			execute(WORKBENCH_ACTION_IDS.reorderWorkspace, {
				beforeWorkspaceId: 'workspace-3',
				projectId: 'project-1',
				workspaceId: 'workspace-2',
			}),
		).resolves.toMatchObject({ code: 'action-unavailable', kind: 'blocked' });
		await expect(
			execute(WORKBENCH_ACTION_IDS.reorderWorkspace, {
				beforeWorkspaceId: null,
				projectId: 'project-1',
				workspaceId: 'workspace-missing',
			}),
		).resolves.toMatchObject({ code: 'action-unavailable', kind: 'blocked' });
		expect(
			projectWorkspaceRuntime
				.getSnapshot()
				.projects[0]?.workspaces.map((workspace) => workspace.id),
		).toEqual(['workspace-2', 'workspace-1']);

		for (const registration of registrations) registration.dispose();
	});

	it('routes primary Folder removal through the accepted circular fallback', async () => {
		const actionRuntime = new ActionRuntime();
		const scope = actionRuntime.createScope('workbench');
		const projectWorkspaceRuntime = projectRuntime();
		const registrations = registerWorkbenchProjectWorkspaceActions({
			primaryWorkspaceName: 'Primary',
			projectWorkspaceRuntime,
			scope,
			typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(actionRuntime),
			workspaceCopySuffix: 'Copy',
			workspaceTerminalTitle: 'Terminal 1',
		});
		await expect(
			actionRuntime.execute(
				WORKBENCH_ACTION_IDS.removeProjectDirectory,
				{ directoryId: 'directory-1', projectId: 'project-1' },
				{ scope, source: 'button' },
			).completion,
		).resolves.toEqual({ kind: 'completed' });
		expect(projectWorkspaceRuntime.getSnapshot().projects[0]).toMatchObject({
			primaryDirectoryId: 'directory-2',
			workspaces: [{ primaryDirectoryId: 'directory-2' }],
		});

		for (const registration of registrations) registration.dispose();
	});

	it('creates one Project atomically from an entered Folder path', async () => {
		const actionRuntime = new ActionRuntime();
		const scope = actionRuntime.createScope('workbench');
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Terminal 1',
			terminalPresentation: createTestTerminalPresentationComposition('action-create-project'),
		});
		const registrations = registerWorkbenchProjectWorkspaceActions({
			primaryWorkspaceName: 'Primary',
			projectWorkspaceRuntime,
			scope,
			typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(actionRuntime),
			workspaceCopySuffix: 'Copy',
			workspaceTerminalTitle: 'Terminal 1',
		});
		const initialState = projectWorkspaceRuntime.getSnapshot();

		await expect(
			actionRuntime.execute(
				WORKBENCH_ACTION_IDS.createProject,
				{ path: '   ' },
				{ scope, source: 'button' },
			).completion,
		).resolves.toEqual({
			code: 'invalid-action-input',
			details: { validationCode: 'expected-workbench-project-create-input' },
			kind: 'blocked',
		});
		expect(projectWorkspaceRuntime.getSnapshot()).toBe(initialState);

		await expect(
			actionRuntime.execute(
				WORKBENCH_ACTION_IDS.createProject,
				{ path: '/workspace/orchbee/' },
				{ scope, source: 'button' },
			).completion,
		).resolves.toEqual({ kind: 'completed' });
		expect(projectWorkspaceRuntime.getSnapshot()).toMatchObject({
			activeLocation: { kind: 'workspace', projectId: 'project-1', workspaceId: 'workspace-1' },
			projects: [
				{
					directories: [
						{
							available: true,
							git: null,
							id: 'entered-folder:/workspace/orchbee/',
							kind: 'folder',
							name: 'orchbee',
							path: '/workspace/orchbee/',
							placement: { kind: 'local' },
							source: 'selected',
						},
					],
					name: 'orchbee',
					workspaces: [{ name: 'Primary' }],
				},
			],
		});
		expect(
			projectWorkspaceRuntime.getActiveTerminalContext().panelWorkspaceStore.getSnapshot().tabs[0]
				?.root,
		).toMatchObject({
			kind: 'panel',
			primaryPresentation: {
				currentCwd: '/workspace/orchbee/',
				launchCwd: '/workspace/orchbee/',
				title: 'Terminal 1',
			},
		});

		for (const registration of registrations) registration.dispose();
	});

	it('adds one unique synthetic Folder to a Project from an entered path', async () => {
		const actionRuntime = new ActionRuntime();
		const scope = actionRuntime.createScope('workbench');
		const projectWorkspaceRuntime = projectRuntime();
		const registrations = registerWorkbenchProjectWorkspaceActions({
			primaryWorkspaceName: 'Primary',
			projectWorkspaceRuntime,
			scope,
			typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(actionRuntime),
			workspaceCopySuffix: 'Copy',
			workspaceTerminalTitle: 'Terminal 1',
		});
		const initialState = projectWorkspaceRuntime.getSnapshot();

		await expect(
			actionRuntime.execute(
				WORKBENCH_ACTION_IDS.addProjectFolder,
				{ path: '/workspace/orchbee', projectId: 'project-1' },
				{ scope, source: 'button' },
			).completion,
		).resolves.toEqual({ code: 'directory-already-exists', kind: 'blocked' });
		expect(projectWorkspaceRuntime.getSnapshot()).toBe(initialState);

		await expect(
			actionRuntime.execute(
				WORKBENCH_ACTION_IDS.addProjectFolder,
				{ path: ' /workspace/ghostty/ ', projectId: 'project-1' },
				{ scope, source: 'button' },
			).completion,
		).resolves.toEqual({ kind: 'completed' });
		expect(projectWorkspaceRuntime.getSnapshot().projects[0]).toMatchObject({
			directories: [
				{ id: 'directory-1' },
				{ id: 'directory-2' },
				{
					available: true,
					git: null,
					id: 'entered-folder:/workspace/ghostty/',
					kind: 'folder',
					name: 'ghostty',
					path: '/workspace/ghostty/',
					placement: { kind: 'local' },
					source: 'selected',
				},
			],
			primaryDirectoryId: 'directory-1',
			workspaces: [{ primaryDirectoryId: 'directory-1' }],
		});

		for (const registration of registrations) registration.dispose();
	});
});

function projectRuntime(): WorkbenchProjectWorkspaceRuntime {
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Terminal 1',
		terminalPresentation: createTestTerminalPresentationComposition('project-actions'),
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
		projectName: 'orchbee',
		terminalTitle: 'Terminal 1',
		type: 'project-created',
	});
	runtime.transition({
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
		projectId: 'project-1',
		type: 'directory-added',
	});
	return runtime;
}
