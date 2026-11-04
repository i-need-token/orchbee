import type { TabStripArrangementChange } from '@orchbee/tab-strip';
import type {
	WorkbenchAuthorityChange,
	WorkbenchAuthorityContext,
	WorkbenchAuthorityCreation,
	WorkbenchAuthorityTabCreation,
} from '../../../workbench';
import {
	UserLaunchProfileCatalogSchema,
	UserLaunchProfileIdSchema,
	UserLaunchProfileSchema,
	parseWorkbenchResponse,
	type UserLaunchProfile,
	type UserLaunchProfileCatalog,
	type WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';
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
const TABS_CONTEXT = 'dddddddddddddddddddddddddddddddd';
const THIRD_WORKSPACE = 'eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee';
const SECOND_TAB = 'ffffffffffffffffffffffffffffffff';
const GROUP = '12121212121212121212121212121212';
const MISSING_GROUP = '13131313131313131313131313131313';
const MISSING_TAB = '14141414141414141414141414141414';
const SECOND_PANEL = '15151515151515151515151515151515';
const SECOND_PRESENTATION = '16161616161616161616161616161616';

function tabRecord(tabId: string, panelId: string, presentationId: string, label: string) {
	return {
		groupId: null,
		label,
		pinned: false,
		root: {
			childPresentations: [],
			kind: 'panel',
			panelId,
			primaryPresentation: {
				binding: null,
				customLabel: null,
				label: 'Terminal',
				presentationId,
			},
		},
		tabId,
	};
}

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
						{
							label: 'With Tabs',
							primaryDirectoryId: DIRECTORY,
							projectId: null,
							terminalContextId: TABS_CONTEXT,
							workspaceId: THIRD_WORKSPACE,
						},
					],
				},
				protocolVersion: 1,
				revision: '5',
				sessionId: SESSION,
				terminalContexts: [
					{ groups: [], tabs: [], terminalContextId: FIRST_CONTEXT },
					{ groups: [], tabs: [], terminalContextId: SECOND_CONTEXT },
					{
						groups: [{ color: 'blue', groupId: GROUP, label: 'Named group' }],
						tabs: [
							tabRecord(TAB, PANEL, PRESENTATION, 'Given name'),
							{
								...tabRecord(SECOND_TAB, SECOND_PANEL, SECOND_PRESENTATION, '2'),
								automaticLabel: true,
							},
						],
						terminalContextId: TABS_CONTEXT,
					},
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

const TAB_CREATED = settled({
	kind: 'tab-created',
	panelId: PANEL,
	presentationId: PRESENTATION,
	revision: '6',
	tabId: SECOND_TAB,
	terminalContextId: TABS_CONTEXT,
	terminalOutcome: {
		geometry: { cellPixelHeight: 18, cellPixelWidth: 9, columns: 120, rows: 30 },
		kind: 'started',
	},
});

function tabsChanged(change: TabStripArrangementChange): WorkbenchAuthorityChange {
	return { change, terminalContextId: TABS_CONTEXT, type: 'tabs-changed' };
}

function tabsConfigured(operation: Record<string, unknown>): Record<string, unknown> {
	return { kind: 'tabs.configure', operation, terminalContextId: TABS_CONTEXT };
}

const SYSTEM_SHELL = UserLaunchProfileSchema.parse({
	command: { kind: 'system-shell', login: false },
	id: 'system-default',
	name: null,
});
const POWERSHELL = UserLaunchProfileSchema.parse({
	command: {
		arguments: ['-NoLogo'],
		executable: 'C:\\Program Files\\PowerShell\\7\\pwsh.exe',
		kind: 'custom-executable',
	},
	id: 'profile-1',
	name: 'PowerShell',
});
const LOGIN_SHELL = UserLaunchProfileSchema.parse({
	command: { kind: 'system-shell', login: true },
	id: 'profile-2',
	name: 'Login shell',
});
const LAUNCH_PROFILES = UserLaunchProfileCatalogSchema.parse({
	defaultProfileId: SYSTEM_SHELL.id,
	profiles: [SYSTEM_SHELL, POWERSHELL, LOGIN_SHELL],
	version: 1,
});

function harness(
	answer: ServerWorkbenchSubmission,
	focusedWorkspaceId: string | null = null,
	currentDirectory: string | null = null,
	launchProfiles: UserLaunchProfileCatalog | null = null,
) {
	const actions: ServerWorkbenchAction[] = [];
	const session: Parameters<typeof createServerWorkbenchAuthority>[0]['session'] = {
		ask: (request) =>
			Promise.resolve(
				currentDirectory === null || request.kind !== 'workbench.current-directory'
					? { kind: 'not-submitted' }
					: {
							kind: 'answered',
							response: parseWorkbenchResponse({
								...request,
								kind: 'workbench.current-directory.result',
								outcome: { directory: currentDirectory, kind: 'observed' },
							}),
						},
			),
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
			...(launchProfiles === null
				? {}
				: {
						launchProfiles: {
							getSnapshot: () => ({ catalog: launchProfiles }),
							subscribe: () => () => undefined,
						},
					}),
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
			'renames a Workspace',
			{ name: 'Renamed', type: 'workspace-updated', workspaceId: FIRST_WORKSPACE },
			{ kind: 'workspace.rename', label: 'Renamed', workspaceId: FIRST_WORKSPACE },
		],
		[
			'renames a Tab',
			{ label: 'Named', tabId: TAB, terminalContextId: TABS_CONTEXT, type: 'tab-renamed' },
			{ kind: 'tab.rename', label: 'Named', tabId: TAB, terminalContextId: TABS_CONTEXT },
		],
		[
			'makes a named group of Tabs in its color',
			tabsChanged({ color: 'blue', kind: 'tabs-grouped', tabIds: [TAB], title: 'Build' }),
			tabsConfigured({
				color: 'blue',
				kind: 'create-group-for-tabs',
				label: 'Build',
				tabIds: [TAB],
			}),
		],
		[
			'moves Tabs to a group',
			tabsChanged({ groupId: GROUP, kind: 'tabs-group-set', tabIds: [TAB] }),
			tabsConfigured({ groupId: GROUP, kind: 'set-tabs-group', tabIds: [TAB] }),
		],
		[
			'pins Tabs',
			tabsChanged({ kind: 'tabs-pinned-set', pinned: true, tabIds: [TAB] }),
			tabsConfigured({ kind: 'set-tabs-pinned', pinned: true, tabIds: [TAB] }),
		],
		[
			'moves a Tab, saying whether its neighbour is collapsed here',
			tabsChanged({
				adjacentGroupCollapsed: true,
				direction: 'next',
				kind: 'tab-moved',
				tabId: TAB,
			}),
			tabsConfigured({
				adjacentGroupCollapsed: true,
				direction: 'next',
				kind: 'move-tab',
				tabId: TAB,
			}),
		],
		[
			'moves a group',
			tabsChanged({ direction: -1, groupId: GROUP, kind: 'group-moved' }),
			tabsConfigured({ direction: 'previous', groupId: GROUP, kind: 'move-group' }),
		],
		[
			'ungroups',
			tabsChanged({ groupId: GROUP, kind: 'group-ungrouped' }),
			tabsConfigured({ groupId: GROUP, kind: 'ungroup' }),
		],
		[
			'recolors a group and keeps its name',
			tabsChanged({ color: 'red', groupId: GROUP, kind: 'group-updated' }),
			tabsConfigured({ color: 'red', groupId: GROUP, kind: 'update-group', label: 'Named group' }),
		],
		[
			'renames a group and keeps its color',
			tabsChanged({ groupId: GROUP, kind: 'group-updated', title: '' }),
			tabsConfigured({ color: 'blue', groupId: GROUP, kind: 'update-group', label: '' }),
		],
		[
			'releases a drag in the order it started from',
			tabsChanged({
				groupId: null,
				insertIndex: 2,
				kind: 'tabs-dragged',
				sourceOrder: [TAB, SECOND_TAB],
				tabIds: [TAB],
			}),
			tabsConfigured({
				groupId: null,
				insertIndex: 2,
				kind: 'move-tabs-by-drag',
				tabIds: [TAB],
			}),
		],
	])('%s', async (_name, change, expected) => {
		const { actions, authority } = harness(RENAMED);
		const { context, markCommitted } = executionContext();

		await expect(authority.change(context, change)).resolves.toEqual({ kind: 'completed' });

		expect(actions).toEqual([expect.objectContaining(expected)]);
		expect(markCommitted).toHaveBeenCalled();
	});

	it.each<[string, TabStripArrangementChange]>([
		[
			'a drag whose order has changed since it started',
			{
				groupId: null,
				insertIndex: 2,
				kind: 'tabs-dragged',
				sourceOrder: [SECOND_TAB, TAB],
				tabIds: [TAB],
			},
		],
		['a group that is gone', { color: 'red', groupId: MISSING_GROUP, kind: 'group-updated' }],
	])('asks nothing of %s', async (_name, change) => {
		const { actions, authority } = harness(RENAMED);
		const { context, markCommitted } = executionContext();

		await expect(authority.change(context, tabsChanged(change))).resolves.toEqual({
			code: 'not-submitted',
			kind: 'blocked',
		});

		expect(actions).toEqual([]);
		expect(markCommitted).not.toHaveBeenCalled();
	});

	it.each<[string, WorkbenchAuthorityTabCreation, Record<string, unknown>]>([
		[
			'names a new Tab by its place, for the terminal to replace',
			{ placement: null, terminalContextId: TABS_CONTEXT },
			{ automaticLabel: true, kind: 'tab.create', label: '3' },
		],
		[
			'starts a Tab where the person asked',
			{ cwd: 'C:\\work\\here', placement: null, terminalContextId: TABS_CONTEXT },
			{ kind: 'tab.create', options: { cwd: 'C:\\work\\here' } },
		],
		[
			'places a Tab after another',
			{ placement: { kind: 'after-tab', tabId: TAB }, terminalContextId: TABS_CONTEXT },
			{ kind: 'tab.create', placement: { kind: 'after-tab', tabId: TAB } },
		],
		[
			'places a Tab in a group',
			{ placement: { groupId: GROUP, kind: 'in-group' }, terminalContextId: TABS_CONTEXT },
			{ kind: 'tab.create', placement: { groupId: GROUP, kind: 'in-group' } },
		],
		[
			'places a Tab in a new group',
			{ placement: { kind: 'new-group' }, terminalContextId: TABS_CONTEXT },
			{ kind: 'tab.create', placement: { kind: 'new-group' } },
		],
		[
			'copies a name that was given',
			{ placement: { kind: 'duplicate', tabId: TAB }, terminalContextId: TABS_CONTEXT },
			{ kind: 'tab.create', label: 'Given name', placement: { kind: 'duplicate', tabId: TAB } },
		],
		[
			'copies a name that follows the terminal',
			{ placement: { kind: 'duplicate', tabId: SECOND_TAB }, terminalContextId: TABS_CONTEXT },
			{ automaticLabel: true, kind: 'tab.create', label: '2' },
		],
	])('%s', async (_name, creation, expected) => {
		const { actions, authority } = harness(TAB_CREATED);
		const { context, markCommitted } = executionContext();

		await expect(authority.createTab(context, creation)).resolves.toEqual({
			kind: 'completed',
			value: { tabId: SECOND_TAB, terminalContextId: TABS_CONTEXT, terminalStarted: true },
		});

		expect(actions).toEqual([expect.objectContaining(expected)]);
		expect(markCommitted).toHaveBeenCalled();
	});

	it.each<[string, unknown]>([
		[
			'is pending',
			{
				kind: 'tab-create-pending',
				panelId: PANEL,
				presentationId: PRESENTATION,
				revision: '6',
				tabId: SECOND_TAB,
				terminalContextId: TABS_CONTEXT,
			},
		],
		[
			'failed',
			{
				kind: 'tab-created',
				panelId: PANEL,
				presentationId: PRESENTATION,
				revision: '6',
				tabId: SECOND_TAB,
				terminalContextId: TABS_CONTEXT,
				terminalOutcome: { kind: 'failed', reason: 'terminal-create-failed' },
			},
		],
	])('reports a Tab whose terminal %s as not started', async (_name, outcome) => {
		const { authority } = harness(settled(outcome));

		await expect(
			authority.createTab(executionContext().context, {
				placement: null,
				terminalContextId: TABS_CONTEXT,
			}),
		).resolves.toEqual({
			kind: 'completed',
			value: { tabId: SECOND_TAB, terminalContextId: TABS_CONTEXT, terminalStarted: false },
		});
	});

	it('asks nothing of a copy whose source is gone', async () => {
		const { actions, authority } = harness(TAB_CREATED);

		await expect(
			authority.createTab(executionContext().context, {
				placement: { kind: 'duplicate', tabId: MISSING_TAB },
				terminalContextId: TABS_CONTEXT,
			}),
		).resolves.toEqual({ code: 'not-submitted', kind: 'blocked' });

		expect(actions).toEqual([]);
	});

	it('starts a custom launch profile as its literal program and the default shell as the default', async () => {
		const { actions, authority } = harness(TAB_CREATED, null, null, LAUNCH_PROFILES);
		const creation = { placement: null, terminalContextId: TABS_CONTEXT };

		await authority.createTab(executionContext().context, { ...creation, profile: POWERSHELL });
		await authority.createTab(executionContext().context, { ...creation, profile: SYSTEM_SHELL });

		expect(actions).toEqual([
			expect.objectContaining({
				kind: 'tab.create',
				launch: { args: ['-NoLogo'], exec: 'C:\\Program Files\\PowerShell\\7\\pwsh.exe' },
			}),
			expect.not.objectContaining({ launch: expect.anything() }),
		]);
	});

	it.each<[string, UserLaunchProfile]>([
		['a system shell that is not the default', LOGIN_SHELL],
		[
			'a profile changed since it was shown',
			UserLaunchProfileSchema.parse({
				...POWERSHELL,
				command: { ...POWERSHELL.command, arguments: ['-NoProfile'] },
			}),
		],
		[
			'a profile no longer saved',
			{ ...POWERSHELL, id: UserLaunchProfileIdSchema.parse('profile-9') },
		],
	])('starts nothing in place of %s', async (_name, profile) => {
		const { actions, authority } = harness(TAB_CREATED, null, null, LAUNCH_PROFILES);
		const { context, markCommitted } = executionContext();

		await expect(
			authority.createTab(context, { placement: null, profile, terminalContextId: TABS_CONTEXT }),
		).resolves.toEqual({ code: 'launch-profile-unavailable', kind: 'blocked' });

		expect(actions).toEqual([]);
		expect(markCommitted).not.toHaveBeenCalled();
	});

	it('opens New Workspace where the Server finds the focused Workspace working now', async () => {
		const { actions, authority } = harness(LAUNCHED, SECOND_WORKSPACE, 'D:\\now');

		await authority.create(executionContext().context, {
			projectId: null,
			type: 'workspace-created-default',
		});

		expect(actions).toEqual([
			expect.objectContaining({
				kind: 'workspace.launch',
				label: 'now',
				options: expect.objectContaining({ cwd: 'D:\\now' }),
			}),
		]);
	});

	it.each<[string, WorkbenchAuthorityCreation, string | null, Record<string, unknown>]>([
		[
			'opens New Workspace in the directory the focused Workspace last reported when the Server cannot read it',
			{ projectId: null, type: 'workspace-created-default' },
			SECOND_WORKSPACE,
			{
				label: 'elsewhere',
				options: expect.objectContaining({ cwd: 'C:\\work\\elsewhere' }),
				placement: { kind: 'ungrouped' },
			},
		],
		[
			'avoids every label the snapshot holds when no terminal reported a directory',
			{ projectId: null, type: 'workspace-created-default' },
			FIRST_WORKSPACE,
			{
				label: 'repo (3)',
				options: expect.objectContaining({ cwd: 'C:\\work\\repo' }),
				placement: { kind: 'ungrouped' },
			},
		],
		[
			'opens New Workspace where this Client works when nothing is focused',
			{ projectId: null, type: 'workspace-created-default' },
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
			authority.create(executionContext().context, {
				projectId: null,
				type: 'workspace-created-default',
			}),
		).resolves.toEqual({
			kind: 'completed',
			value: {
				directoryId: DIRECTORY,
				path: 'C:\\Users\\person',
				presentationId: PRESENTATION,
				projectId: null,
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
