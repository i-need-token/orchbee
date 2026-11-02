import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import { describe, expect, it, vi } from 'vitest';
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
import type { WorkbenchAuthority } from '../../types/workbench-authority';
import type { WorkbenchTerminalHandover } from '../../types/workbench-focus-routing';
import type { WorkbenchProjectCreationReceipt } from '../../types/workbench-project-folder-acquisition';
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

	describe('with an authority', () => {
		function withAuthority(
			authority: Pick<WorkbenchAuthority, 'change' | 'create'> & Partial<WorkbenchAuthority>,
			handover: WorkbenchTerminalHandover = { complete: () => undefined, held: () => true },
			confirm: () => boolean = () => true,
		) {
			const actionRuntime = new ActionRuntime({ confirm });
			const scope = actionRuntime.createScope('workbench');
			const projectWorkspaceRuntime = projectRuntime();
			registerWorkbenchProjectWorkspaceActions({
				authority: {
					close: () => Promise.reject(new Error('Nothing closes here.')),
					createTab: () => Promise.reject(new Error('No Tab is created here.')),
					watchClose: () => () => undefined,
					...authority,
				},
				beginTerminalHandover: () => handover,
				defaultWorkspaceName: 'Workspace',
				primaryWorkspaceName: 'Primary',
				projectWorkspaceRuntime,
				scope,
				typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(actionRuntime),
				workspaceCopySuffix: 'Copy',
				workspaceTerminalTitle: 'Terminal 1',
			});
			return {
				execute: (actionId: string, input: unknown) =>
					actionRuntime.execute(actionId, input, { scope, source: 'programmatic' }).completion,
				projectWorkspaceRuntime,
			};
		}
		const receiptFor = (workspaceId: string): WorkbenchProjectCreationReceipt => ({
			directoryId: 'directory-1',
			path: '/workspace/orchbee',
			presentationId: 'presentation-1',
			projectId: null,
			resourceScopeId: 'resource-scope-1',
			terminalContextId: 'terminal-context-1',
			workspaceId,
		});

		it('asks the authority for a shared change and leaves the read model to its publication', async () => {
			const change = vi.fn<WorkbenchAuthority['change']>(() =>
				Promise.resolve({ kind: 'completed' }),
			);
			const { execute, projectWorkspaceRuntime } = withAuthority({
				change,
				create: () => Promise.resolve({ kind: 'cancelled' }),
			});
			const before = projectWorkspaceRuntime.getSnapshot();

			await expect(
				execute(WORKBENCH_ACTION_IDS.renameProject, { name: 'Renamed', projectId: 'project-1' }),
			).resolves.toEqual({ kind: 'completed' });

			expect(change).toHaveBeenCalledWith(expect.anything(), {
				name: 'Renamed',
				projectId: 'project-1',
				type: 'project-renamed',
			});
			expect(projectWorkspaceRuntime.getSnapshot()).toBe(before);
		});

		it('enters a Workspace it created and gives its terminal the keyboard', async () => {
			const complete = vi.fn<() => void>();
			const { execute, projectWorkspaceRuntime } = withAuthority(
				{
					change: () => Promise.resolve({ kind: 'completed' }),
					create: () => Promise.resolve({ kind: 'completed', value: receiptFor('workspace-1') }),
				},
				{ complete, held: () => true },
			);
			projectWorkspaceRuntime.transition({ type: 'standalone-activated' });

			await expect(execute(WORKBENCH_ACTION_IDS.createWorkspace, undefined)).resolves.toEqual({
				kind: 'completed',
			});

			expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
				kind: 'workspace',
				workspaceId: 'workspace-1',
			});
			expect(complete).toHaveBeenCalledOnce();
		});

		it('gives the keyboard to a first Workspace the read model already entered', async () => {
			const complete = vi.fn<() => void>();
			const { execute, projectWorkspaceRuntime } = withAuthority(
				{
					change: () => Promise.resolve({ kind: 'completed' }),
					create: () => {
						projectWorkspaceRuntime.transition({
							type: 'workspace-activated',
							workspaceId: 'workspace-1',
						});
						return Promise.resolve({ kind: 'completed', value: receiptFor('workspace-1') });
					},
				},
				{ complete, held: () => true },
			);
			projectWorkspaceRuntime.transition({ type: 'standalone-activated' });

			await execute(WORKBENCH_ACTION_IDS.createWorkspace, undefined);

			expect(complete).toHaveBeenCalledOnce();
		});

		it('stays where it is after a later press, while the creation still completes', async () => {
			const complete = vi.fn<() => void>();
			const { execute, projectWorkspaceRuntime } = withAuthority(
				{
					change: () => Promise.resolve({ kind: 'completed' }),
					create: () => Promise.resolve({ kind: 'completed', value: receiptFor('workspace-1') }),
				},
				{ complete, held: () => false },
			);
			projectWorkspaceRuntime.transition({ type: 'standalone-activated' });

			await expect(execute(WORKBENCH_ACTION_IDS.createWorkspace, undefined)).resolves.toEqual({
				kind: 'completed',
			});

			expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({ kind: 'standalone' });
			expect(complete).not.toHaveBeenCalled();
		});

		it('leaves the person where they went while a creation was pending', async () => {
			const pending: { finish?: () => void } = {};
			const complete = vi.fn<() => void>();
			const { execute, projectWorkspaceRuntime } = withAuthority(
				{
					change: () => Promise.resolve({ kind: 'completed' }),
					create: () =>
						new Promise((resolve) => {
							pending.finish = () =>
								resolve({ kind: 'completed', value: receiptFor('workspace-1') });
						}),
				},
				{ complete, held: () => true },
			);
			projectWorkspaceRuntime.transition({ type: 'standalone-activated' });
			const creation = execute(WORKBENCH_ACTION_IDS.createWorkspace, undefined);
			await vi.waitFor(() => expect(pending.finish).toBeDefined());

			projectWorkspaceRuntime.transition({
				directoryId: 'directory-1',
				launchConfigurationId: null,
				name: 'Feature',
				projectId: 'project-1',
				resourceScopeId: 'resource-scope-1',
				terminalTitle: 'Terminal 2',
				type: 'workspace-created',
			});
			projectWorkspaceRuntime.transition({
				type: 'workspace-activated',
				workspaceId: 'workspace-2',
			});
			pending.finish?.();
			await creation;

			expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
				kind: 'workspace',
				workspaceId: 'workspace-2',
			});
			expect(complete).not.toHaveBeenCalled();
		});

		it('offers nothing the authority has no command for', async () => {
			const { execute, projectWorkspaceRuntime } = withAuthority({
				change: () => Promise.reject(new Error('No command exists for this.')),
				create: () => Promise.reject(new Error('No command exists for this.')),
			});
			const before = projectWorkspaceRuntime.getSnapshot();

			for (const [actionId, input] of [
				[WORKBENCH_ACTION_IDS.duplicateWorkspace, { workspaceId: 'workspace-1' }],
				[
					WORKBENCH_ACTION_IDS.addResourceFolder,
					{ path: '/workspace/other', resourceScopeId: 'resource-scope-1' },
				],
				[
					WORKBENCH_ACTION_IDS.removeResourceDirectory,
					{ directoryId: 'directory-2', resourceScopeId: 'resource-scope-1' },
				],
				[
					WORKBENCH_ACTION_IDS.setResourcePrimaryDirectory,
					{ directoryId: 'directory-2', resourceScopeId: 'resource-scope-1' },
				],
			] as const) {
				await expect(execute(actionId, input)).resolves.toMatchObject({
					code: 'action-unavailable',
					kind: 'blocked',
				});
			}

			expect(projectWorkspaceRuntime.getSnapshot()).toBe(before);
		});

		it('closes a Workspace that runs nothing without asking and returns the keyboard', async () => {
			const close = vi.fn<WorkbenchAuthority['close']>(() =>
				Promise.resolve({ kind: 'completed' }),
			);
			const complete = vi.fn<() => void>();
			const confirm = vi.fn<() => boolean>(() => true);
			const { execute } = withAuthority(
				{
					change: () => Promise.resolve({ kind: 'completed' }),
					close,
					create: () => Promise.resolve({ kind: 'cancelled' }),
				},
				{ complete, held: () => true },
				confirm,
			);

			await expect(
				execute(WORKBENCH_ACTION_IDS.deleteWorkspace, { workspaceId: 'workspace-1' }),
			).resolves.toEqual({ kind: 'completed' });

			expect(close.mock.calls.map(([, closure]) => closure)).toEqual([
				{ type: 'workspace-close-if-idle', workspaceId: 'workspace-1' },
			]);
			expect(confirm).not.toHaveBeenCalled();
			expect(complete).toHaveBeenCalledOnce();
		});

		it('asks only after the authority refuses to close a busy Workspace unasked', async () => {
			const close = vi.fn<WorkbenchAuthority['close']>((_context, closure) =>
				Promise.resolve(
					closure.type === 'workspace-close-if-idle'
						? { code: 'confirmation-required', kind: 'blocked' }
						: { kind: 'completed' },
				),
			);
			const confirm = vi.fn<() => boolean>(() => true);
			const { execute } = withAuthority(
				{
					change: () => Promise.resolve({ kind: 'completed' }),
					close,
					create: () => Promise.resolve({ kind: 'cancelled' }),
				},
				undefined,
				confirm,
			);

			await expect(
				execute(WORKBENCH_ACTION_IDS.deleteWorkspace, { workspaceId: 'workspace-1' }),
			).resolves.toEqual({ kind: 'unchanged' });

			await vi.waitFor(() =>
				expect(close.mock.calls.map(([, closure]) => closure)).toEqual([
					{ type: 'workspace-close-if-idle', workspaceId: 'workspace-1' },
					{ type: 'workspace-close', workspaceId: 'workspace-1' },
				]),
			);
			expect(confirm).toHaveBeenCalledOnce();
		});

		it('leaves a busy Workspace alone when the person declines', async () => {
			const close = vi.fn<WorkbenchAuthority['close']>(() =>
				Promise.resolve({ code: 'confirmation-required', kind: 'blocked' }),
			);
			const confirm = vi.fn<() => boolean>(() => false);
			const { execute } = withAuthority(
				{
					change: () => Promise.resolve({ kind: 'completed' }),
					close,
					create: () => Promise.resolve({ kind: 'cancelled' }),
				},
				undefined,
				confirm,
			);

			await execute(WORKBENCH_ACTION_IDS.deleteWorkspace, { workspaceId: 'workspace-1' });

			await vi.waitFor(() => expect(confirm).toHaveBeenCalledOnce());
			expect(close).toHaveBeenCalledOnce();
		});

		it('still renames a Workspace the Server holds without a directory', async () => {
			const change = vi.fn<WorkbenchAuthority['change']>(() =>
				Promise.resolve({ kind: 'completed' }),
			);
			const { execute, projectWorkspaceRuntime } = withAuthority({
				change,
				create: () => Promise.resolve({ kind: 'cancelled' }),
			});
			const resources = projectWorkspaceRuntime.getSnapshot().resourceScopes[0];
			const directory = resources?.directories[0];
			if (resources === undefined || directory === undefined) {
				throw new Error('The Workspace has no directory.');
			}
			projectWorkspaceRuntime.transition({
				directory: { ...directory, available: false },
				resourceScopeId: resources.id,
				type: 'directory-updated',
			});

			await expect(
				execute(WORKBENCH_ACTION_IDS.updateWorkspace, {
					name: 'Renamed',
					primaryDirectoryId: 'directory-1',
					workspaceId: 'workspace-1',
				}),
			).resolves.toEqual({ kind: 'completed' });
			expect(change).toHaveBeenCalledOnce();
		});
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
