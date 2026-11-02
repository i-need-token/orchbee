import type {
	WorkbenchAuthorityChange,
	WorkbenchAuthorityContext,
	WorkbenchAuthorityCreation,
} from '@orchbee/workbench';
import { parseWorkbenchResponse, type WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import { createServerWorkbenchAuthority } from '../createServerWorkbenchAuthority';
import type { ServerWorkbenchAction, ServerWorkbenchSubmission } from '../ServerWorkbenchSession';

const SESSION = '11111111111111111111111111111111';
const FIRST_WORKSPACE = '22222222222222222222222222222222';
const SECOND_WORKSPACE = '33333333333333333333333333333333';
const PROJECT = '44444444444444444444444444444444';
const DIRECTORY = '55555555555555555555555555555555';
const WORKER = '66666666666666666666666666666666';
const FIRST_CONTEXT = '77777777777777777777777777777777';
const SECOND_CONTEXT = '88888888888888888888888888888888';
const TAB = '99999999999999999999999999999999';
const PANEL = 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa';
const PRESENTATION = 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';
const REQUEST = 'cccccccccccccccccccccccccccccccc';

function readSnapshot(): WorkbenchSnapshot {
	const response = parseWorkbenchResponse({
		kind: 'workbench.snapshot.result',
		outcome: {
			kind: 'available',
			snapshot: {
				kind: 'workbench.snapshot',
				navigation: {
					directories: [{ directoryId: DIRECTORY, path: 'C:\\work\\repo', workerId: WORKER }],
					projects: [{ label: 'Project', projectId: PROJECT }],
					workspaces: [
						{
							label: 'repo',
							primaryDirectoryId: DIRECTORY,
							projectId: PROJECT,
							terminalContextId: FIRST_CONTEXT,
							workspaceId: FIRST_WORKSPACE,
						},
						{
							label: 'repo (2)',
							primaryDirectoryId: DIRECTORY,
							projectId: null,
							terminalContextId: SECOND_CONTEXT,
							terminalDirectory: 'C:\\work\\elsewhere',
							workspaceId: SECOND_WORKSPACE,
						},
					],
				},
				protocolVersion: 1,
				revision: '5',
				sessionId: SESSION,
				terminalContexts: [
					{ groups: [], tabs: [], terminalContextId: FIRST_CONTEXT },
					{ groups: [], tabs: [], terminalContextId: SECOND_CONTEXT },
				],
			},
		},
		protocolVersion: 1,
		requestId: REQUEST,
	});
	if (response.kind !== 'workbench.snapshot.result' || response.outcome.kind !== 'available') {
		throw new Error('The snapshot did not parse as available.');
	}
	return response.outcome.snapshot;
}

const SNAPSHOT = readSnapshot();

function settled(outcome: unknown): ServerWorkbenchSubmission {
	const response = parseWorkbenchResponse({
		kind: 'workbench.command.result',
		outcome,
		protocolVersion: 1,
		requestId: REQUEST,
	});
	if (response.kind !== 'workbench.command.result') {
		throw new Error('The outcome did not parse as a command result.');
	}
	return { kind: 'settled', outcome: response.outcome };
}

const RENAMED = settled({ kind: 'renamed', revision: '6' });
const LAUNCHED = settled({
	directoryId: DIRECTORY,
	kind: 'workspace-launch-pending',
	panelId: PANEL,
	presentationId: PRESENTATION,
	projectId: null,
	revision: '6',
	tabId: TAB,
	terminalContextId: SECOND_CONTEXT,
	workspaceId: SECOND_WORKSPACE,
});

function harness(answer: ServerWorkbenchSubmission, focusedWorkspaceId: string | null = null) {
	const actions: ServerWorkbenchAction[] = [];
	const session: Parameters<typeof createServerWorkbenchAuthority>[0]['session'] = {
		ask: () => Promise.resolve({ kind: 'not-submitted' }),
		published: () => Promise.resolve(),
		snapshot: SNAPSHOT,
		submit: (decide) => {
			const action = decide(SNAPSHOT);
			if (action === null) return Promise.resolve({ kind: 'not-submitted' });
			actions.push(action);
			return Promise.resolve(answer);
		},
		subscribe: () => () => undefined,
	};
	return {
		actions,
		authority: createServerWorkbenchAuthority({
			defaultDirectory: 'C:\\Users\\person',
			defaultWorkspaceLabel: () => 'Workspace',
			focusedWorkspaceId: () => focusedWorkspaceId,
			session,
		}),
	};
}

function executionContext(signal = new AbortController().signal) {
	const markCommitted = vi.fn<WorkbenchAuthorityContext['markCommitted']>();
	const context: WorkbenchAuthorityContext = { markCommitted, signal };
	return { context, markCommitted };
}

describe('createServerWorkbenchAuthority', () => {
	it.each<[string, WorkbenchAuthorityChange, Record<string, unknown>]>([
		[
			'renames a Project',
			{ name: 'Renamed', projectId: PROJECT, type: 'project-renamed' },
			{ kind: 'project.rename', label: 'Renamed', projectId: PROJECT },
		],
		[
			'dissolves a Project',
			{ projectId: PROJECT, type: 'project-dissolved' },
			{ kind: 'project.dissolve', projectId: PROJECT },
		],
		[
			'moves a Project',
			{ beforeProjectId: null, projectId: PROJECT, type: 'project-reordered' },
			{
				kind: 'navigation.configure',
				operation: { beforeProjectId: null, kind: 'move-project', projectId: PROJECT },
			},
		],
		[
			'moves a Workspace within the Project holding it',
			{
				beforeWorkspaceId: SECOND_WORKSPACE,
				type: 'workspace-reordered',
				workspaceId: FIRST_WORKSPACE,
			},
			{
				kind: 'navigation.configure',
				operation: {
					beforeWorkspaceId: SECOND_WORKSPACE,
					kind: 'move-workspace',
					projectId: PROJECT,
					workspaceId: FIRST_WORKSPACE,
				},
			},
		],
		[
			'moves a Workspace to a Project',
			{ projectId: PROJECT, type: 'workspace-project-set', workspaceId: SECOND_WORKSPACE },
			{ kind: 'workspace.set-project', projectId: PROJECT, workspaceId: SECOND_WORKSPACE },
		],
		[
			'moves a Workspace to a new Project',
			{ name: 'New', type: 'workspace-project-created', workspaceId: SECOND_WORKSPACE },
			{ kind: 'workspace.create-project', label: 'New', workspaceId: SECOND_WORKSPACE },
		],
		[
			'renames a Workspace and leaves its directory alone',
			{
				launchConfigurationId: null,
				name: 'Renamed',
				primaryDirectoryId: 'elsewhere',
				type: 'workspace-updated',
				workspaceId: FIRST_WORKSPACE,
			},
			{ kind: 'workspace.rename', label: 'Renamed', workspaceId: FIRST_WORKSPACE },
		],
	])('%s', async (_name, change, expected) => {
		const { actions, authority } = harness(RENAMED);
		const { context, markCommitted } = executionContext();

		await expect(authority.change(context, change)).resolves.toEqual({ kind: 'completed' });

		expect(actions).toEqual([expect.objectContaining(expected)]);
		expect(markCommitted).toHaveBeenCalled();
	});

	it.each<[string, WorkbenchAuthorityCreation, string | null, Record<string, unknown>]>([
		[
			'opens New Workspace in the directory the focused Workspace last reported',
			{ type: 'workspace-created-default' },
			SECOND_WORKSPACE,
			{
				label: 'elsewhere',
				options: expect.objectContaining({ cwd: 'C:\\work\\elsewhere' }),
				placement: { kind: 'ungrouped' },
			},
		],
		[
			'avoids every label the snapshot holds when no terminal reported a directory',
			{ type: 'workspace-created-default' },
			FIRST_WORKSPACE,
			{
				label: 'repo (3)',
				options: expect.objectContaining({ cwd: 'C:\\work\\repo' }),
				placement: { kind: 'ungrouped' },
			},
		],
		[
			'opens New Workspace where this Client works when nothing is focused',
			{ type: 'workspace-created-default' },
			null,
			{
				label: 'person',
				options: expect.objectContaining({ cwd: 'C:\\Users\\person' }),
				placement: { kind: 'ungrouped' },
			},
		],
		[
			'opens a Workspace from a known directory in the Project asked for',
			{ directoryId: DIRECTORY, projectId: PROJECT, type: 'workspace-created' },
			null,
			{
				options: expect.objectContaining({ cwd: 'C:\\work\\repo' }),
				placement: { kind: 'project', projectId: PROJECT },
			},
		],
		[
			'opens a new Project from a chosen path, naming a root by the fallback',
			{ path: 'D:\\', projectName: 'Named', type: 'project-created' },
			null,
			{
				label: 'Workspace',
				options: expect.objectContaining({ cwd: 'D:\\' }),
				placement: { kind: 'new-project', label: 'Named' },
			},
		],
	])('%s', async (_name, creation, focused, expected) => {
		const { actions, authority } = harness(LAUNCHED, focused);

		await authority.create(executionContext().context, creation);

		expect(actions).toEqual([
			expect.objectContaining({ automaticLabel: true, kind: 'workspace.launch', ...expected }),
		]);
	});

	it('reports the Workspace the Server committed and the directory it was decided from', async () => {
		const { authority } = harness(LAUNCHED);

		await expect(
			authority.create(executionContext().context, { type: 'workspace-created-default' }),
		).resolves.toEqual({
			kind: 'completed',
			value: {
				directoryId: DIRECTORY,
				path: 'C:\\Users\\person',
				presentationId: PRESENTATION,
				projectId: null,
				resourceScopeId: `resource-scope:${SECOND_WORKSPACE}`,
				terminalContextId: SECOND_CONTEXT,
				workspaceId: SECOND_WORKSPACE,
			},
		});
	});

	it('keeps a refusal apart from an unknown outcome', async () => {
		const resultFor = (answer: ServerWorkbenchSubmission): Promise<unknown> =>
			harness(answer).authority.change(executionContext().context, {
				projectId: PROJECT,
				type: 'project-dissolved',
			});

		await expect(
			resultFor(settled({ kind: 'rejected', reason: 'revision-mismatch', revision: '7' })),
		).resolves.toEqual({ code: 'revision-mismatch', kind: 'blocked' });
		await expect(resultFor({ kind: 'not-submitted' })).resolves.toEqual({
			code: 'not-submitted',
			kind: 'blocked',
		});
		await expect(resultFor({ kind: 'outcome-unknown' })).resolves.toEqual({
			failure: { code: 'outcome-unknown' },
			kind: 'failed',
		});
		await expect(resultFor(settled({ kind: 'unchanged', revision: '5' }))).resolves.toEqual({
			kind: 'unchanged',
		});
	});

	it('asks nothing for a target or a path the Server could not accept', async () => {
		const { actions, authority } = harness(RENAMED);

		await expect(
			authority.create(executionContext().context, {
				directoryId: 'missing',
				projectId: null,
				type: 'workspace-created',
			}),
		).resolves.toEqual({ code: 'not-submitted', kind: 'blocked' });
		await expect(
			authority.create(executionContext().context, {
				path: 'relative\\path',
				projectName: 'path',
				type: 'project-created',
			}),
		).resolves.toEqual({ code: 'not-submitted', kind: 'blocked' });

		expect(actions).toEqual([]);
	});

	it('crosses no commit point once cancelled', async () => {
		const controller = new AbortController();
		controller.abort();
		const { actions, authority } = harness(RENAMED);
		const { context, markCommitted } = executionContext(controller.signal);

		await expect(
			authority.change(context, { projectId: PROJECT, type: 'project-dissolved' }),
		).resolves.toEqual({ kind: 'cancelled' });

		expect(actions).toEqual([]);
		expect(markCommitted).not.toHaveBeenCalled();
	});
});
