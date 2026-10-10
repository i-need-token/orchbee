import { describe, expect, it, vi } from 'vitest';

import {
	parseWorkbenchClientView,
	parseWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '../../document';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '../../../terminal-presentation';
import { WorkbenchProjectWorkspaceRuntime } from '../WorkbenchProjectWorkspaceRuntime';

function composition(scope: string, allocate = () => undefined) {
	let sequence = 0;
	return {
		kind: 'managed' as const,
		initialPresentationId: terminalPresentationId(`${scope}-initial`),
		createPresentationId: () => {
			allocate();
			return terminalPresentationId(`${scope}-${++sequence}`);
		},
		createOperationId: () => terminalPresentationOperationId(`${scope}-${++sequence}`),
		createRemovalId: () => terminalPresentationRemovalId(`${scope}-${++sequence}`),
		dispatchCommands: vi.fn<() => void>(),
	};
}

function runtime() {
	const result = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/home',
		standaloneTerminalTitle: 'Shell',
		terminalPresentation: composition('original'),
	});
	for (const name of ['alpha', 'beta']) {
		result.transition({
			type: 'project-created',
			projectName: name,
			primaryWorkspaceName: name,
			terminalTitle: 'Shell',
			directory: {
				id: 'directory',
				path: `/${name}`,
				name,
				available: true,
				git: null,
				kind: 'folder',
				placement: { kind: 'local' },
				source: 'selected',
			},
			launchConfigurations: [
				{ id: 'launch', name },
				{ id: 'unused-launch', name: `Unused ${name}` },
			],
			primaryWorkspaceLaunchConfigurationId: 'launch',
		});
		result.transition({
			type: 'directory-added',
			resourceScopeId: `resource-scope-${name === 'alpha' ? 1 : 2}`,
			directory: {
				id: 'unused',
				path: `/${name}/unused`,
				name: 'Unused',
				available: true,
				git: null,
				kind: 'folder',
				placement: { kind: 'local' },
				source: 'selected',
			},
		});
	}
	return result;
}

function historicalDocument() {
	const original = runtime().getSharedDocument(workbenchSharedDocumentRevision('27'));
	const graph = original.projectWorkspace;
	return {
		...original,
		version: 2,
		projectWorkspace: {
			nextProjectSequence: graph.nextProjectSequence,
			nextWorkspaceSequence: graph.nextWorkspaceSequence,
			nextTerminalContextSequence: graph.nextTerminalContextSequence,
			standaloneTerminalContextId: graph.standaloneTerminalContextId,
			projects: graph.projects.map((project, index) => {
				const resources = graph.resourceScopes[index];
				if (resources === undefined) throw new Error('Expected a resource namespace.');
				return {
					id: index === 0 ? 'resource-scope-1' : project.id,
					name: project.name,
					directories: resources.directories,
					primaryDirectoryId: resources.primaryDirectoryId,
					launchConfigurations: resources.launchConfigurations,
					workspaces: graph.workspaces
						.filter((workspace) => workspace.projectId === project.id)
						.map(({ id, name, primaryDirectoryId, launchConfigurationId, terminalContextId }) => ({
							id,
							name,
							primaryDirectoryId,
							launchConfigurationId,
							terminalContextId,
						})),
				};
			}),
		},
	};
}

describe('Workspace organization migration and live publication', () => {
	it('migrates strict v2 graphs without flattening scoped IDs or dropping unused resources', () => {
		const old = historicalDocument();
		const migrated = parseWorkbenchSharedDocument(old);
		expect(migrated.version).toBe(3);
		expect(migrated.revision).toBe(old.revision);
		expect(migrated.terminalContexts).toEqual(old.terminalContexts);
		expect(migrated.projectWorkspace.resourceScopes).toEqual(
			old.projectWorkspace.projects.map(
				({ id, name, directories, primaryDirectoryId, launchConfigurations }) => ({
					id,
					name,
					directories,
					primaryDirectoryId,
					launchConfigurations,
				}),
			),
		);
		expect(
			migrated.projectWorkspace.workspaces.map((workspace) => [
				workspace.id,
				workspace.resourceScopeId,
				workspace.launchConfigurationId,
			]),
		).toEqual([
			['workspace-1', 'resource-scope-1', 'launch'],
			['workspace-2', 'project-2', 'launch'],
		]);
		const client = new WorkbenchProjectWorkspaceRuntime({
			initialDocument: migrated,
			standaloneCwd: '/home',
			standaloneTerminalTitle: 'Shell',
			terminalPresentation: composition('migrated'),
		});
		client.createDefaultWorkspace('Workspace', 'Shell');
		expect(client.getSnapshot().workspaces[2]?.resourceScopeId).toBe('resource-scope-2');
		expect(
			client.getSharedDocument(migrated.revision).projectWorkspace.resourceScopes.slice(0, 2),
		).toEqual(migrated.projectWorkspace.resourceScopes);
	});

	it('uses available localized one-click names while explicit names remain unchanged', () => {
		const client = runtime();
		for (const name of ['Workspace', 'Workspace', '工作区', '工作区'])
			client.createDefaultWorkspace(name, 'Shell');
		expect(client.getSnapshot().workspaces.map((workspace) => workspace.name)).toEqual([
			'alpha',
			'beta',
			'Workspace',
			'Workspace (2)',
			'工作区',
			'工作区 (2)',
		]);
		client.transition({
			type: 'workspace-updated',
			workspaceId: 'workspace-3',
			name: 'Workspace (4)',
			primaryDirectoryId: 'default-directory',
			launchConfigurationId: null,
		});
		client.createDefaultWorkspace('Workspace', 'Shell');
		client.createDefaultWorkspace('Workspace', 'Shell');
		client.createDefaultWorkspace('Workspace', 'Shell');
		expect(
			client
				.getSnapshot()
				.workspaces.slice(-3)
				.map((workspace) => workspace.name),
		).toEqual(['Workspace', 'Workspace (3)', 'Workspace (5)']);
		expect(client.getSnapshot().workspaces[2]?.name).toBe('Workspace (4)');
	});

	it('rejects malformed historical structure and views before migration instead of resetting them', () => {
		const old = historicalDocument();
		expect(() =>
			parseWorkbenchSharedDocument({
				...old,
				projectWorkspace: {
					...old.projectWorkspace,
					projects: old.projectWorkspace.projects.map((project) => ({
						...project,
						workspaces: project.workspaces.map((workspace) => ({
							...workspace,
							primaryDirectoryId: 'missing',
						})),
					})),
				},
			}),
		).toThrow('Historical Workspace workspace-1 has a missing Directory.');
		expect(() =>
			parseWorkbenchSharedDocument({
				...old,
				projectWorkspace: {
					...old.projectWorkspace,
					projects: old.projectWorkspace.projects.map((project) => ({
						...project,
						directories: [...project.directories, project.directories[0]],
					})),
				},
			}),
		).toThrow('Duplicate historical Directory directory.');
		const currentView = runtime().getClientView();
		const oldView = {
			...currentView,
			version: 1,
			activeLocation: { kind: 'workspace', projectId: 'project-2', workspaceId: 'workspace-2' },
		};
		expect(parseWorkbenchClientView(oldView)).toEqual({
			...currentView,
			version: 2,
			activeLocation: { kind: 'workspace', workspaceId: 'workspace-2' },
		});
		expect(() =>
			parseWorkbenchClientView({
				...oldView,
				activeLocation: { kind: 'workspace', workspaceId: 'workspace-2' },
			}),
		).toThrow('projectId');
		expect(() =>
			parseWorkbenchClientView({
				...oldView,
				terminalContexts: [...oldView.terminalContexts, oldView.terminalContexts[0]],
			}),
		).toThrow('Client terminal contexts must use unique canonical identity order.');
	});

	it('preserves two clients and their live context objects, focus, zoom and child selection on move and dissolve', () => {
		const first = runtime();
		first.transition({ type: 'workspace-activated', workspaceId: 'workspace-1' });
		const context = first.getActiveTerminalContext();
		const initialTab = context.panelWorkspaceStore.getSnapshot().tabs[0];
		if (initialTab?.root.kind !== 'panel') throw new Error('Expected a primary panel.');
		context.panelWorkspaceStore.transition({
			type: 'panel-split',
			direction: 'right',
			panelId: initialTab.root.id,
			tabId: initialTab.tabId,
			newTerminalTitle: 'Split',
			presentation: context.panelWorkspaceStore.createTerminalPresentation(),
		});
		context.panelWorkspaceStore.transition({
			type: 'child-terminal-created',
			panelId: initialTab.root.id,
			tabId: initialTab.tabId,
			newTerminalTitle: 'Child',
			presentation: context.panelWorkspaceStore.createTerminalPresentation(),
		});
		context.panelWorkspaceStore.transition({
			type: 'panel-maximization-toggled',
			panelId: initialTab.root.id,
			tabId: initialTab.tabId,
		});
		const shared = first.getSharedDocument(workbenchSharedDocumentRevision('1'));
		const second = new WorkbenchProjectWorkspaceRuntime({
			initialDocument: shared,
			standaloneCwd: '/home',
			standaloneTerminalTitle: 'Shell',
			terminalPresentation: composition('second'),
		});
		second.transition({ type: 'workspace-activated', workspaceId: 'workspace-2' });
		const secondContext = second.getActiveTerminalContext();
		const views = [first.getClientView(), second.getClientView()];
		const locations = [first.getSnapshot().activeLocation, second.getSnapshot().activeLocation];
		const navigation = vi.fn<() => void>();
		const unsubscribe = first.subscribeNavigation(navigation);
		first.transition({
			type: 'workspace-project-set',
			workspaceId: 'workspace-1',
			projectId: 'project-2',
		});
		first.transition({ type: 'project-dissolved', projectId: 'project-2' });
		second.applySharedDocument(first.getSharedDocument(workbenchSharedDocumentRevision('2')));
		expect(first.getActiveTerminalContext()).toBe(context);
		expect(second.getActiveTerminalContext()).toBe(secondContext);
		for (const [index, client] of [first, second].entries()) {
			expect(client.getSnapshot().activeLocation).toBe(locations[index]);
			expect(client.getClientView().terminalContexts).toEqual(views[index]?.terminalContexts);
		}
		expect(first.getSharedDocument(shared.revision).terminalContexts).toEqual(
			shared.terminalContexts,
		);
		expect(navigation).not.toHaveBeenCalled();
		first.createTerminal({ groupId: null, source: 'programmatic', title: 'Another' });
		expect(navigation).toHaveBeenCalled();
		unsubscribe();
	});

	it('does not publish grouping or a Workspace when allocating its first shell fails', () => {
		const commit = vi.fn<() => void>();
		const client = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/home',
			standaloneTerminalTitle: 'Shell',
			terminalPresentation: composition('failure', () => {
				throw new Error('allocation failed');
			}),
		});
		const before = client.getSharedDocument(workbenchSharedDocumentRevision('0'));
		const state = client.getSnapshot();
		expect(() => client.createDefaultWorkspace('Workspace', 'Shell', commit)).toThrow(
			'allocation failed',
		);
		expect(client.getSnapshot()).toBe(state);
		expect(client.getSharedDocument(before.revision)).toEqual(before);
		expect(commit).not.toHaveBeenCalled();
	});
});
