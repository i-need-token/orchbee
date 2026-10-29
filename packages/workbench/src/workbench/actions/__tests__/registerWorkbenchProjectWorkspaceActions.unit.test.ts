import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { describe, expect, it } from 'vitest';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench-client-protocol';

import {
	createWorkbenchPreviewTerminalPresentationComposition,
	type WorkbenchTerminalPresentationComposition,
} from '../../model/createWorkbenchPreviewTerminalPresentationComposition';
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
	it('captures the creation receipt before observers replace the current route and rejects a replaced first presentation', async () => {
		const actionRuntime = new ActionRuntime();
		const typed = createTypedActionRuntime<WorkbenchActionCatalog>(actionRuntime);
		const scope = actionRuntime.createScope('workbench');
		let sequence = 0;
		const runtime = projectRuntime({
			createOperationId: () => terminalPresentationOperationId(`receipt:operation:${++sequence}`),
			createPresentationId: () => terminalPresentationId(`receipt:presentation:${++sequence}`),
			createRemovalId: () => terminalPresentationRemovalId(`receipt:removal:${++sequence}`),
			dispatchCommands: () => undefined,
			initialPresentationId: terminalPresentationId('receipt:initial'),
			kind: 'managed',
		});
		const registrations = registerWorkbenchProjectWorkspaceActions({
			defaultWorkspaceName: 'Workspace',
			primaryWorkspaceName: 'Primary',
			projectWorkspaceRuntime: runtime,
			scope,
			typedRuntime: typed,
			workspaceCopySuffix: 'Copy',
			workspaceTerminalTitle: 'Terminal',
		});
		let replaced = false;
		const unsubscribe = runtime.subscribe(() => {
			if (replaced) return;
			replaced = true;
			runtime.transition({ type: 'standalone-activated' });
		});
		const result = await typed.execute(
			WORKBENCH_ACTION_IDS.createProject,
			{ path: '/workspace/new' },
			{ scope, source: 'button' },
		).completion;
		unsubscribe();
		if (result.kind !== 'completed' || result.value === undefined)
			throw new Error('Expected the creation receipt.');
		const receipt = result.value;
		expect(runtime.getSnapshot().activeLocation).toEqual({ kind: 'standalone' });
		expect(receipt).toMatchObject({
			projectId: 'project-2',
			workspaceId: 'workspace-2',
			path: '/workspace/new',
		});
		const document = runtime.getSharedDocument(workbenchSharedDocumentRevision('1'));
		runtime.applySharedDocument({
			...document,
			terminalContexts: document.terminalContexts.map((context) =>
				context.terminalContextId !== receipt.terminalContextId
					? context
					: {
							...context,
							panelWorkspace: {
								...context.panelWorkspace,
								tabs: context.panelWorkspace.tabs.map((tab) =>
									tab.root.kind !== 'panel'
										? tab
										: {
												...tab,
												root: {
													...tab.root,
													primaryPresentation: {
														...tab.root.primaryPresentation,
														presentationId: terminalPresentationId('replacement'),
													},
												},
											},
								),
							},
						},
			),
		});
		await expect(
			typed.execute(
				WORKBENCH_ACTION_IDS.activateWorkspace,
				{
					creationReceipt: receipt,

					workspaceId: receipt.workspaceId,
				},
				{ scope, source: 'button' },
			).completion,
		).resolves.toMatchObject({ kind: 'blocked' });
		expect(runtime.getSnapshot().activeLocation).toEqual({ kind: 'standalone' });
		for (const registration of registrations) registration.dispose();
		actionRuntime.dispose();
	});

	it('routes Workspace reorder through the scoped semantic Action', async () => {
		const actionRuntime = new ActionRuntime();
		const scope = actionRuntime.createScope('workbench');
		const projectWorkspaceRuntime = projectRuntime();
		projectWorkspaceRuntime.transition({
			directoryId: 'directory-1',
			launchConfigurationId: null,
			name: 'Feature',
			resourceScopeId: 'resource-scope-1',
			projectId: 'project-1',
			terminalTitle: 'Terminal 2',
			type: 'workspace-created',
		});
		const activeLocation = projectWorkspaceRuntime.getSnapshot().activeLocation;
		const firstContext = projectWorkspaceRuntime.getTerminalContext('terminal-context-2');
		const secondContext = projectWorkspaceRuntime.getTerminalContext('terminal-context-3');
		const registrations = registerWorkbenchProjectWorkspaceActions({
			defaultWorkspaceName: 'Workspace',
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

					workspaceId: 'workspace-2',
				},
				{ scope, source: 'pointer' },
			).completion,
		).resolves.toEqual({ kind: 'completed' });
		expect(
			projectWorkspaceRuntime
				.getSnapshot()
				.workspaces.filter((workspace) => workspace.projectId === 'project-1')
				.map((workspace) => workspace.id),
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
			defaultWorkspaceName: 'Workspace',
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

				workspaceId: 'workspace-1',
			}),
		).resolves.toMatchObject({ code: 'action-unavailable', kind: 'blocked' });
		await expect(
			execute(WORKBENCH_ACTION_IDS.updateWorkspace, {
				name: 'Renamed',
				primaryDirectoryId: 'directory-2',

				workspaceId: 'workspace-1',
			}),
		).resolves.toEqual({ kind: 'completed' });
		await expect(
			execute(WORKBENCH_ACTION_IDS.duplicateWorkspace, {
				workspaceId: 'workspace-1',
			}),
		).resolves.toEqual({ kind: 'completed' });
		expect(
			projectWorkspaceRuntime
				.getSnapshot()
				.workspaces.filter(
					(workspace) =>
						workspace.projectId === projectWorkspaceRuntime.getSnapshot().projects[0]?.id,
				)[1]?.name,
		).toBe('Renamed Copy');
		await expect(
			execute(WORKBENCH_ACTION_IDS.reorderWorkspace, {
				beforeWorkspaceId: 'workspace-1',

				workspaceId: 'workspace-2',
			}),
		).resolves.toEqual({ kind: 'completed' });
		await expect(
			execute(WORKBENCH_ACTION_IDS.reorderWorkspace, {
				beforeWorkspaceId: 'workspace-2',

				workspaceId: 'workspace-2',
			}),
		).resolves.toEqual({ kind: 'unchanged' });

		await expect(
			execute(WORKBENCH_ACTION_IDS.createProject, { path: '/workspace/other' }),
		).resolves.toMatchObject({ kind: 'completed', value: { path: '/workspace/other' } });
		await expect(
			execute(WORKBENCH_ACTION_IDS.reorderWorkspace, {
				beforeWorkspaceId: 'workspace-3',

				workspaceId: 'workspace-2',
			}),
		).resolves.toMatchObject({ code: 'action-unavailable', kind: 'blocked' });
		await expect(
			execute(WORKBENCH_ACTION_IDS.reorderWorkspace, {
				beforeWorkspaceId: null,

				workspaceId: 'workspace-missing',
			}),
		).resolves.toMatchObject({ code: 'action-unavailable', kind: 'blocked' });
		expect(
			projectWorkspaceRuntime
				.getSnapshot()
				.workspaces.filter((workspace) => workspace.projectId === 'project-1')
				.map((workspace) => workspace.id),
		).toEqual(['workspace-2', 'workspace-1']);

		for (const registration of registrations) registration.dispose();
	});

	it('routes primary Folder removal through the accepted circular fallback', async () => {
		const actionRuntime = new ActionRuntime();
		const scope = actionRuntime.createScope('workbench');
		const projectWorkspaceRuntime = projectRuntime();
		const registrations = registerWorkbenchProjectWorkspaceActions({
			defaultWorkspaceName: 'Workspace',
			primaryWorkspaceName: 'Primary',
			projectWorkspaceRuntime,
			scope,
			typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(actionRuntime),
			workspaceCopySuffix: 'Copy',
			workspaceTerminalTitle: 'Terminal 1',
		});
		await expect(
			actionRuntime.execute(
				WORKBENCH_ACTION_IDS.removeResourceDirectory,
				{ directoryId: 'directory-1', resourceScopeId: 'resource-scope-1' },
				{ scope, source: 'button' },
			).completion,
		).resolves.toEqual({ kind: 'completed' });
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
			defaultWorkspaceName: 'Workspace',
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
		).resolves.toMatchObject({
			kind: 'completed',
			value: {
				directoryId: 'entered-folder:/workspace/orchbee/',
				path: '/workspace/orchbee/',
				resourceScopeId: 'resource-scope-1',
				projectId: 'project-1',
				workspaceId: 'workspace-1',
				terminalContextId: 'terminal-context-2',
				presentationId: expect.any(String),
			},
		});
		expect(projectWorkspaceRuntime.getSnapshot()).toMatchObject({
			activeLocation: { kind: 'workspace', workspaceId: 'workspace-1' },
			projects: [{ name: 'orchbee' }],
			resourceScopes: [
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
				},
			],
			workspaces: [{ name: 'Primary' }],
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
			defaultWorkspaceName: 'Workspace',
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
				WORKBENCH_ACTION_IDS.addResourceFolder,
				{ path: '/workspace/orchbee', resourceScopeId: 'resource-scope-1' },
				{ scope, source: 'button' },
			).completion,
		).resolves.toEqual({ code: 'directory-already-exists', kind: 'blocked' });
		expect(projectWorkspaceRuntime.getSnapshot()).toBe(initialState);

		await expect(
			actionRuntime.execute(
				WORKBENCH_ACTION_IDS.addResourceFolder,
				{ path: ' /workspace/ghostty/ ', resourceScopeId: 'resource-scope-1' },
				{ scope, source: 'button' },
			).completion,
		).resolves.toEqual({ kind: 'completed' });
		expect(projectWorkspaceRuntime.getSnapshot().resourceScopes[0]).toMatchObject({
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
		});
		expect(
			projectWorkspaceRuntime
				.getSnapshot()
				.workspaces.filter(
					(workspace) =>
						workspace.projectId === projectWorkspaceRuntime.getSnapshot().projects[0]?.id,
				),
		).toMatchObject([{ primaryDirectoryId: 'directory-1' }]);

		for (const registration of registrations) registration.dispose();
	});
});

function projectRuntime(
	terminalPresentation: WorkbenchTerminalPresentationComposition = createTestTerminalPresentationComposition(
		'project-actions',
	),
): WorkbenchProjectWorkspaceRuntime {
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Terminal 1',
		terminalPresentation,
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
		resourceScopeId: 'resource-scope-1',
		type: 'directory-added',
	});
	return runtime;
}
