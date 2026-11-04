import { ActionRuntime, createTypedActionRuntime } from '@orchbee/actions';
import type { WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import {
	createSnapshotRuntime,
	fixtureDirectoryId,
	fixturePresentationId,
	fixtureProjectId,
	fixtureWorkspaceId,
	publishSnapshot,
	snapshotContext,
	snapshotDirectory,
	snapshotProject,
	snapshotTab,
	snapshotWorkspace,
	workbenchSnapshot,
} from '../../../__tests__/server-workbench-snapshot';
import { inertWorkbenchAuthority } from '../../../__tests__/inertWorkbenchAuthority';
import type { WorkbenchAuthority } from '../../types/workbench-authority';
import type { WorkbenchTerminalHandover } from '../../types/workbench-focus-routing';
import type { WorkbenchProjectCreationReceipt } from '../../types/workbench-project-folder-acquisition';
import { registerWorkbenchProjectWorkspaceActions } from '../registerWorkbenchProjectWorkspaceActions';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../workbench-action-ids';

const FIRST_PROJECT_ID = fixtureProjectId(1);
const PRIMARY_WORKSPACE_ID = fixtureWorkspaceId(1);
const FEATURE_WORKSPACE_ID = fixtureWorkspaceId(2);
const OTHER_WORKSPACE_ID = fixtureWorkspaceId(3);

/** Each Workspace `[sequence, Project sequence]` in Directory 1, with its own one-Tab context. */
function projectSnapshot(
	revision: number,
	workspaces: readonly (readonly [number, number])[] = [[1, 1]],
): WorkbenchSnapshot {
	const projectSequences = [...new Set(workspaces.map(([, project]) => project))];
	return workbenchSnapshot(
		revision,
		{
			directories: [snapshotDirectory(1)],
			projects: projectSequences.map((sequence) =>
				snapshotProject(sequence, sequence === 1 ? 'orchbee' : `Project ${sequence}`),
			),
			workspaces: workspaces.map(([sequence, project]) =>
				snapshotWorkspace(sequence, {
					label: sequence === 1 ? 'Primary' : `Workspace ${sequence}`,
					primaryDirectoryId: fixtureDirectoryId(1),
					projectId: fixtureProjectId(project),
				}),
			),
		},
		workspaces.map(([sequence]) => snapshotContext(sequence, [snapshotTab(sequence)])),
	);
}

function createHarness(
	authority: Partial<WorkbenchAuthority> = {},
	handover: WorkbenchTerminalHandover = { complete: () => undefined, held: () => true },
	confirm: () => boolean = () => true,
	snapshot: WorkbenchSnapshot = projectSnapshot(1),
) {
	const actionRuntime = new ActionRuntime({ confirm });
	const scope = actionRuntime.createScope('workbench');
	const projectWorkspaceRuntime = createSnapshotRuntime(snapshot);
	registerWorkbenchProjectWorkspaceActions({
		authority: inertWorkbenchAuthority(authority),
		beginTerminalHandover: () => handover,
		projectWorkspaceRuntime,
		scope,
		typedRuntime: createTypedActionRuntime<WorkbenchActionCatalog>(actionRuntime),
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
	terminalContextId: 'terminal-context-1',
	workspaceId,
});

/** Primary and Feature Workspaces in Project 1. */
const TWO_WORKSPACES = projectSnapshot(1, [
	[1, 1],
	[2, 1],
]);

/** No Workspace, as when the last one closed. */
const NO_WORKSPACES = workbenchSnapshot(1, { workspaces: [] }, []);

function enterFeatureWorkspace(
	runtime: ReturnType<typeof createHarness>['projectWorkspaceRuntime'],
) {
	runtime.transition({ type: 'workspace-activated', workspaceId: FEATURE_WORKSPACE_ID });
}

describe('registerWorkbenchProjectWorkspaceActions', () => {
	it('rejects a creation receipt whose first presentation was replaced', async () => {
		const { execute, projectWorkspaceRuntime } = createHarness(
			undefined,
			undefined,
			undefined,
			TWO_WORKSPACES,
		);
		const workspace = projectWorkspaceRuntime.getSnapshot().workspaces[0];
		const directory = workspace?.directory;
		if (workspace === undefined || directory === undefined || directory === null) {
			throw new Error('Expected a Workspace in a Folder.');
		}
		const receipt: WorkbenchProjectCreationReceipt = {
			directoryId: directory.id,
			path: directory.path,
			presentationId: fixturePresentationId(1),
			projectId: workspace.projectId,
			terminalContextId: workspace.terminalContextId,
			workspaceId: workspace.id,
		};
		enterFeatureWorkspace(projectWorkspaceRuntime);

		await expect(
			execute(WORKBENCH_ACTION_IDS.activateWorkspace, {
				creationReceipt: { ...receipt, presentationId: 'replacement' },
				workspaceId: receipt.workspaceId,
			}),
		).resolves.toMatchObject({ kind: 'blocked' });
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
			kind: 'workspace',
			workspaceId: FEATURE_WORKSPACE_ID,
		});

		await expect(
			execute(WORKBENCH_ACTION_IDS.activateWorkspace, {
				creationReceipt: receipt,
				workspaceId: receipt.workspaceId,
			}),
		).resolves.toEqual({ kind: 'completed' });
		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
			kind: 'workspace',
			workspaceId: receipt.workspaceId,
		});
	});

	it('asks the authority to reorder Workspaces and leaves the read model to its publication', async () => {
		const change = vi.fn<WorkbenchAuthority['change']>(() =>
			Promise.resolve({ kind: 'completed' }),
		);
		const { execute, projectWorkspaceRuntime } = createHarness(
			{ change },
			undefined,
			undefined,
			projectSnapshot(1, [
				[1, 1],
				[2, 1],
			]),
		);
		const before = projectWorkspaceRuntime.getSnapshot();

		await expect(
			execute(WORKBENCH_ACTION_IDS.reorderWorkspace, {
				beforeWorkspaceId: PRIMARY_WORKSPACE_ID,
				workspaceId: FEATURE_WORKSPACE_ID,
			}),
		).resolves.toEqual({ kind: 'completed' });

		expect(change).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			beforeWorkspaceId: PRIMARY_WORKSPACE_ID,
			type: 'workspace-reordered',
			workspaceId: FEATURE_WORKSPACE_ID,
		});
		expect(projectWorkspaceRuntime.getSnapshot()).toBe(before);
	});

	it('rejects stale reorder targets without asking the authority', async () => {
		const change = vi.fn<WorkbenchAuthority['change']>(() =>
			Promise.resolve({ kind: 'completed' }),
		);
		const { execute } = createHarness(
			{ change },
			undefined,
			undefined,
			projectSnapshot(1, [
				[1, 1],
				[2, 1],
				[3, 2],
			]),
		);

		for (const input of [
			// The only Workspace of its Project.
			{ beforeWorkspaceId: null, workspaceId: OTHER_WORKSPACE_ID },
			// A Workspace of another Project.
			{ beforeWorkspaceId: OTHER_WORKSPACE_ID, workspaceId: FEATURE_WORKSPACE_ID },
			{ beforeWorkspaceId: null, workspaceId: 'workspace-missing' },
		]) {
			await expect(execute(WORKBENCH_ACTION_IDS.reorderWorkspace, input)).resolves.toMatchObject({
				code: 'action-unavailable',
				kind: 'blocked',
			});
		}
		expect(change).not.toHaveBeenCalled();
	});

	it('creates one Project from an entered Folder path', async () => {
		const create = vi.fn<WorkbenchAuthority['create']>(() =>
			Promise.resolve({ kind: 'cancelled' }),
		);
		const { execute } = createHarness({ create });

		await expect(execute(WORKBENCH_ACTION_IDS.createProject, { path: '   ' })).resolves.toEqual({
			code: 'invalid-action-input',
			details: { validationCode: 'expected-workbench-project-create-input' },
			kind: 'blocked',
		});
		expect(create).not.toHaveBeenCalled();

		await execute(WORKBENCH_ACTION_IDS.createProject, { path: '/workspace/orchbee/' });
		expect(create).toHaveBeenCalledExactlyOnceWith(expect.anything(), {
			path: '/workspace/orchbee/',
			projectName: 'orchbee',
			type: 'project-created',
		});
	});

	it('asks the authority for a shared change and leaves the read model to its publication', async () => {
		const change = vi.fn<WorkbenchAuthority['change']>(() =>
			Promise.resolve({ kind: 'completed' }),
		);
		const { execute, projectWorkspaceRuntime } = createHarness({ change });
		const before = projectWorkspaceRuntime.getSnapshot();

		await expect(
			execute(WORKBENCH_ACTION_IDS.renameProject, {
				name: 'Renamed',
				projectId: FIRST_PROJECT_ID,
			}),
		).resolves.toEqual({ kind: 'completed' });

		expect(change).toHaveBeenCalledWith(expect.anything(), {
			name: 'Renamed',
			projectId: FIRST_PROJECT_ID,
			type: 'project-renamed',
		});
		expect(projectWorkspaceRuntime.getSnapshot()).toBe(before);
	});

	it('enters a Workspace it created and gives its terminal the keyboard', async () => {
		const complete = vi.fn<() => void>();
		const { execute, projectWorkspaceRuntime } = createHarness(
			{
				create: () =>
					Promise.resolve({ kind: 'completed', value: receiptFor(PRIMARY_WORKSPACE_ID) }),
			},
			{ complete, held: () => true },
			undefined,
			TWO_WORKSPACES,
		);
		enterFeatureWorkspace(projectWorkspaceRuntime);

		await expect(execute(WORKBENCH_ACTION_IDS.createWorkspace, undefined)).resolves.toEqual({
			kind: 'completed',
		});

		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
			kind: 'workspace',
			workspaceId: PRIMARY_WORKSPACE_ID,
		});
		expect(complete).toHaveBeenCalledOnce();
	});

	it('gives the keyboard to a first Workspace the read model already entered', async () => {
		const complete = vi.fn<() => void>();
		const { execute, projectWorkspaceRuntime } = createHarness(
			{
				create: () => {
					// Once a Workspace exists, No Workspace is no longer a place to be.
					publishSnapshot(projectWorkspaceRuntime, projectSnapshot(2));
					return Promise.resolve({
						kind: 'completed',
						value: receiptFor(PRIMARY_WORKSPACE_ID),
					});
				},
			},
			{ complete, held: () => true },
			undefined,
			NO_WORKSPACES,
		);

		await execute(WORKBENCH_ACTION_IDS.createWorkspace, undefined);

		expect(complete).toHaveBeenCalledOnce();
	});

	it('stays where it is after a later press, while the creation still completes', async () => {
		const complete = vi.fn<() => void>();
		const { execute, projectWorkspaceRuntime } = createHarness(
			{
				create: () =>
					Promise.resolve({ kind: 'completed', value: receiptFor(PRIMARY_WORKSPACE_ID) }),
			},
			{ complete, held: () => false },
			undefined,
			TWO_WORKSPACES,
		);
		enterFeatureWorkspace(projectWorkspaceRuntime);

		await expect(execute(WORKBENCH_ACTION_IDS.createWorkspace, undefined)).resolves.toEqual({
			kind: 'completed',
		});

		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
			kind: 'workspace',
			workspaceId: FEATURE_WORKSPACE_ID,
		});
		expect(complete).not.toHaveBeenCalled();
	});

	it('leaves the person where they went while a creation was pending', async () => {
		const pending: { finish?: () => void } = {};
		const complete = vi.fn<() => void>();
		const { execute, projectWorkspaceRuntime } = createHarness(
			{
				create: () =>
					new Promise((resolve) => {
						pending.finish = () =>
							resolve({ kind: 'completed', value: receiptFor(PRIMARY_WORKSPACE_ID) });
					}),
			},
			{ complete, held: () => true },
			undefined,
			NO_WORKSPACES,
		);
		const creation = execute(WORKBENCH_ACTION_IDS.createWorkspace, undefined);
		await vi.waitFor(() => expect(pending.finish).toBeDefined());

		publishSnapshot(
			projectWorkspaceRuntime,
			projectSnapshot(2, [
				[1, 1],
				[2, 1],
			]),
		);
		projectWorkspaceRuntime.transition({
			type: 'workspace-activated',
			workspaceId: FEATURE_WORKSPACE_ID,
		});
		pending.finish?.();
		await creation;

		expect(projectWorkspaceRuntime.getSnapshot().activeLocation).toEqual({
			kind: 'workspace',
			workspaceId: FEATURE_WORKSPACE_ID,
		});
		expect(complete).not.toHaveBeenCalled();
	});

	it('closes a Workspace that runs nothing without asking and returns the keyboard', async () => {
		const close = vi.fn<WorkbenchAuthority['close']>(() => Promise.resolve({ kind: 'completed' }));
		const complete = vi.fn<() => void>();
		const confirm = vi.fn<() => boolean>(() => true);
		const { execute } = createHarness({ close }, { complete, held: () => true }, confirm);

		await expect(
			execute(WORKBENCH_ACTION_IDS.deleteWorkspace, { workspaceId: PRIMARY_WORKSPACE_ID }),
		).resolves.toEqual({ kind: 'completed' });

		expect(close.mock.calls.map(([, closure]) => closure)).toEqual([
			{ type: 'workspace-close-if-idle', workspaceId: PRIMARY_WORKSPACE_ID },
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
		const { execute } = createHarness({ close }, undefined, confirm);

		await expect(
			execute(WORKBENCH_ACTION_IDS.deleteWorkspace, { workspaceId: PRIMARY_WORKSPACE_ID }),
		).resolves.toEqual({ kind: 'unchanged' });

		await vi.waitFor(() =>
			expect(close.mock.calls.map(([, closure]) => closure)).toEqual([
				{ type: 'workspace-close-if-idle', workspaceId: PRIMARY_WORKSPACE_ID },
				{ type: 'workspace-close', workspaceId: PRIMARY_WORKSPACE_ID },
			]),
		);
		expect(confirm).toHaveBeenCalledOnce();
	});

	it('leaves a busy Workspace alone when the person declines', async () => {
		const close = vi.fn<WorkbenchAuthority['close']>(() =>
			Promise.resolve({ code: 'confirmation-required', kind: 'blocked' }),
		);
		const confirm = vi.fn<() => boolean>(() => false);
		const { execute } = createHarness({ close }, undefined, confirm);

		await execute(WORKBENCH_ACTION_IDS.deleteWorkspace, { workspaceId: PRIMARY_WORKSPACE_ID });

		await vi.waitFor(() => expect(confirm).toHaveBeenCalledOnce());
		expect(close).toHaveBeenCalledOnce();
	});

	it('still renames a Workspace the Server holds without a directory', async () => {
		const change = vi.fn<WorkbenchAuthority['change']>(() =>
			Promise.resolve({ kind: 'completed' }),
		);
		const { execute } = createHarness(
			{ change },
			undefined,
			undefined,
			workbenchSnapshot(1, { workspaces: [snapshotWorkspace(1, { primaryDirectoryId: null })] }, [
				snapshotContext(1, [snapshotTab(1)]),
			]),
		);

		await expect(
			execute(WORKBENCH_ACTION_IDS.updateWorkspace, {
				name: 'Renamed',
				primaryDirectoryId: fixtureDirectoryId(1),
				workspaceId: PRIMARY_WORKSPACE_ID,
			}),
		).resolves.toEqual({ kind: 'completed' });
		expect(change).toHaveBeenCalledOnce();
	});
});
