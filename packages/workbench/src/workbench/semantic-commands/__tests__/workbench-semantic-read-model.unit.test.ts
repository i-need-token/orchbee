import { describe, expect, it } from 'vitest';

import type { WorkbenchDirectory } from '../../../project-workspace';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '../../../terminal-presentation';
import { workbenchSharedDocumentRevision } from '../../document';
import { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import { executeWorkbenchTerminalCreate } from '../executeWorkbenchTerminalCreate';
import {
	projectWorkbenchDirectoryList,
	projectWorkbenchPanelList,
	projectWorkbenchProjectList,
	projectWorkbenchTabList,
	projectWorkbenchTerminalList,
	projectWorkbenchWorkspaceList,
} from '../workbench-semantic-read-model';

const PRIMARY_DIRECTORY: WorkbenchDirectory = {
	available: true,
	git: null,
	id: 'directory-primary',
	kind: 'folder',
	name: 'primary',
	path: '/projects/primary',
	placement: { kind: 'local' },
	source: 'selected',
};

const FEATURE_DIRECTORY: WorkbenchDirectory = {
	...PRIMARY_DIRECTORY,
	id: 'directory-feature',
	name: 'feature',
	path: '/projects/feature',
	placement: { kind: 'local' },
	source: 'discovered',
};

const UNAVAILABLE_DIRECTORY: WorkbenchDirectory = {
	...PRIMARY_DIRECTORY,
	available: false,
	id: 'directory-unavailable',
	name: 'unavailable',
	path: '/projects/unavailable',
};

function createRuntime(): WorkbenchProjectWorkspaceRuntime {
	let operationSequence = 1;
	let presentationSequence = 1;
	let removalSequence = 1;
	return new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Standalone',
		terminalPresentation: {
			createOperationId: () =>
				terminalPresentationOperationId(`read:operation:${operationSequence++}`),
			createPresentationId: () =>
				terminalPresentationId(`read:presentation:${presentationSequence++}`),
			createRemovalId: () => terminalPresentationRemovalId(`read:removal:${removalSequence++}`),
			dispatchCommands: () => undefined,
			initialPresentationId: terminalPresentationId('read:presentation:initial'),
			kind: 'managed',
		},
	});
}

function addProject(
	runtime: WorkbenchProjectWorkspaceRuntime,
	input: { readonly directory: WorkbenchDirectory; readonly name: string },
): void {
	const result = runtime.transition({
		directory: input.directory,
		launchConfigurations: [],
		primaryWorkspaceLaunchConfigurationId: null,
		primaryWorkspaceName: `${input.name} workspace`,
		projectName: input.name,
		terminalTitle: `${input.name} terminal`,
		type: 'project-created',
	});
	if (result.outcome !== 'applied') throw new Error(`Failed to create ${input.name}.`);
}

describe('Workbench semantic read model', () => {
	it('projects canonical resource order and exact terminal topology from one document revision', () => {
		const runtime = createRuntime();
		addProject(runtime, { directory: PRIMARY_DIRECTORY, name: 'alpha' });
		runtime.transition({
			directory: FEATURE_DIRECTORY,
			projectId: 'project-1',
			type: 'directory-added',
		});
		runtime.transition({
			directory: UNAVAILABLE_DIRECTORY,
			projectId: 'project-1',
			type: 'directory-added',
		});
		addProject(runtime, {
			directory: {
				...PRIMARY_DIRECTORY,
				id: 'directory-beta',
				name: 'beta',
				path: '/projects/beta',
			},
			name: 'beta',
		});

		const alpha = runtime.getSnapshot().projects[0];
		const alphaWorkspace = alpha?.workspaces[0];
		if (alpha === undefined || alphaWorkspace === undefined) {
			throw new Error('Expected the alpha Project and Workspace.');
		}
		const alphaContext = runtime.getTerminalContext(alphaWorkspace.terminalContextId);
		const initialTab = alphaContext?.panelWorkspaceStore.getSnapshot().tabs[0];
		if (alphaContext === null || alphaContext === undefined || initialTab?.root.kind !== 'panel') {
			throw new Error('Expected the alpha terminal context.');
		}
		alphaContext.panelWorkspaceStore.transition({
			newTerminalTitle: 'Alpha child',
			panelId: initialTab.root.id,
			presentation: alphaContext.panelWorkspaceStore.createTerminalPresentation(),
			tabId: initialTab.tabId,
			type: 'child-terminal-created',
		});
		alphaContext.panelWorkspaceStore.transition({
			direction: 'right',
			newTerminalTitle: 'Alpha split',
			panelId: initialTab.root.id,
			presentation: alphaContext.panelWorkspaceStore.createTerminalPresentation(),
			tabId: initialTab.tabId,
			type: 'panel-split',
		});
		const creation = executeWorkbenchTerminalCreate(runtime, {
			directoryId: FEATURE_DIRECTORY.id,
			fallbackTitle: 'Alpha extra',
			location: {
				kind: 'workspace',
				projectId: alpha.id,
				workspaceId: alphaWorkspace.id,
			},
			placement: { groupId: null },
		});
		if (creation.kind !== 'created') throw new Error(`Creation rejected: ${creation.reason}`);

		const revision = workbenchSharedDocumentRevision('7');
		const document = runtime.getSharedDocument(revision);

		expect(projectWorkbenchProjectList(document)).toEqual({
			items: [
				{ name: 'alpha', projectId: 'project-1' },
				{ name: 'beta', projectId: 'project-2' },
			],
			revision,
		});
		expect(projectWorkbenchDirectoryList(document, alpha.id)).toEqual({
			items: [
				{
					available: true,
					directoryId: PRIMARY_DIRECTORY.id,
					kind: 'folder',
					name: 'primary',
					path: '/projects/primary',
					placement: { kind: 'local' },
					primary: true,
					source: 'selected',
				},
				{
					available: true,
					directoryId: FEATURE_DIRECTORY.id,
					kind: 'folder',
					name: 'feature',
					path: '/projects/feature',
					placement: { kind: 'local' },
					primary: false,
					source: 'discovered',
				},
				{
					available: false,
					directoryId: UNAVAILABLE_DIRECTORY.id,
					kind: 'folder',
					name: 'unavailable',
					path: '/projects/unavailable',
					placement: { kind: 'local' },
					primary: false,
					source: 'selected',
				},
			],
			revision,
		});
		expect(projectWorkbenchWorkspaceList(document, alpha.id)).toEqual({
			items: [
				{
					name: 'alpha workspace',
					primaryDirectoryId: PRIMARY_DIRECTORY.id,
					workspaceId: alphaWorkspace.id,
				},
			],
			revision,
		});
		const location = {
			kind: 'workspace' as const,
			projectId: alpha.id,
			workspaceId: alphaWorkspace.id,
		};
		const tabs = projectWorkbenchTabList(document, location);
		if ('kind' in tabs) throw new Error(`Tab projection rejected: ${tabs.reason}`);
		expect(tabs).toEqual({
			items: [
				{
					groupId: null,
					panelCount: 2,
					pinned: false,
					tabCustomTitle: null,
					tabFallbackTitle: 'alpha terminal',
					tabId: initialTab.tabId,
					terminalContextId: alphaWorkspace.terminalContextId,
					terminalCount: 3,
				},
				{
					groupId: null,
					panelCount: 1,
					pinned: false,
					tabCustomTitle: null,
					tabFallbackTitle: 'Alpha extra',
					tabId: creation.tabId,
					terminalContextId: alphaWorkspace.terminalContextId,
					terminalCount: 1,
				},
			],
			revision,
		});

		const panels = projectWorkbenchPanelList(document, location, initialTab.tabId);
		if ('kind' in panels) throw new Error(`Panel projection rejected: ${panels.reason}`);
		expect(panels.revision).toBe(revision);
		expect(panels.items).toHaveLength(2);
		expect(panels.items[0]).toEqual({
			panelId: initialTab.root.id,
			position: 1,
			tabId: initialTab.tabId,
			terminalContextId: alphaWorkspace.terminalContextId,
			terminalCount: 2,
		});
		expect(panels.items[1]).toMatchObject({
			position: 2,
			tabId: initialTab.tabId,
			terminalContextId: alphaWorkspace.terminalContextId,
			terminalCount: 1,
		});

		const terminals = projectWorkbenchTerminalList(document, location);
		if ('kind' in terminals) throw new Error(`Terminal projection rejected: ${terminals.reason}`);
		expect(terminals.revision).toBe(revision);
		expect(
			terminals.items.map((terminal) => ({
				launchDirectoryId: terminal.launchDirectoryId,
				role: terminal.role,
				tabId: terminal.tabId,
				terminalTitle: terminal.terminalTitle,
			})),
		).toEqual([
			{
				launchDirectoryId: PRIMARY_DIRECTORY.id,
				role: 'primary',
				tabId: initialTab.tabId,
				terminalTitle: 'alpha terminal',
			},
			{
				launchDirectoryId: PRIMARY_DIRECTORY.id,
				role: 'child',
				tabId: initialTab.tabId,
				terminalTitle: 'Alpha child',
			},
			{
				launchDirectoryId: PRIMARY_DIRECTORY.id,
				role: 'primary',
				tabId: initialTab.tabId,
				terminalTitle: 'Alpha split',
			},
			{
				launchDirectoryId: FEATURE_DIRECTORY.id,
				role: 'primary',
				tabId: creation.tabId,
				terminalTitle: 'Alpha extra',
			},
		]);
		expect(
			terminals.items.every(
				(terminal) => terminal.terminalContextId === alphaWorkspace.terminalContextId,
			),
		).toBe(true);
	});

	it('projects an authoritative remote Directory placement without creating a Workspace', () => {
		const runtime = createRuntime();
		const registration = runtime.transition({
			directory: {
				available: false,
				git: null,
				id: 'environment-directory:11111111111111111111111111111111',
				kind: 'folder',
				name: 'orchbee',
				path: '/srv/orchbee',
				placement: {
					environmentId: 'environment:22222222222222222222222222222222',
					kind: 'environment',
				},
				source: 'selected',
			},
			projectName: 'orchbee',
			type: 'environment-project-registered',
		});
		expect(registration).toMatchObject({ commands: [], outcome: 'applied' });

		const document = runtime.getSharedDocument(workbenchSharedDocumentRevision('1'));
		expect(projectWorkbenchDirectoryList(document, 'project-1')).toEqual({
			items: [
				{
					available: false,
					directoryId: 'environment-directory:11111111111111111111111111111111',
					kind: 'folder',
					name: 'orchbee',
					path: '/srv/orchbee',
					placement: {
						environmentId: 'environment:22222222222222222222222222222222',
						kind: 'environment',
					},
					primary: true,
					source: 'selected',
				},
			],
			revision: '1',
		});
		expect(projectWorkbenchWorkspaceList(document, 'project-1')).toEqual({
			items: [],
			revision: '1',
		});
	});

	it('rejects stale and cross-Project read targets without a Client View fallback', () => {
		const runtime = createRuntime();
		addProject(runtime, { directory: PRIMARY_DIRECTORY, name: 'alpha' });
		addProject(runtime, {
			directory: {
				...PRIMARY_DIRECTORY,
				id: 'directory-beta',
				path: '/projects/beta',
			},
			name: 'beta',
		});
		const document = runtime.getSharedDocument(workbenchSharedDocumentRevision('8'));

		expect(projectWorkbenchDirectoryList(document, 'missing')).toEqual({
			kind: 'rejected',
			reason: 'project-not-found',
		});
		expect(projectWorkbenchWorkspaceList(document, 'missing')).toEqual({
			kind: 'rejected',
			reason: 'project-not-found',
		});
		expect(
			projectWorkbenchTerminalList(document, {
				kind: 'workspace',
				projectId: 'project-1',
				workspaceId: 'workspace-2',
			}),
		).toEqual({ kind: 'rejected', reason: 'workspace-not-found' });
		expect(
			projectWorkbenchTerminalList(document, {
				kind: 'workspace',
				projectId: 'missing',
				workspaceId: 'workspace-2',
			}),
		).toEqual({ kind: 'rejected', reason: 'project-not-found' });
		expect(
			projectWorkbenchTabList(document, {
				kind: 'workspace',
				projectId: 'project-1',
				workspaceId: 'workspace-2',
			}),
		).toEqual({ kind: 'rejected', reason: 'workspace-not-found' });
		expect(
			projectWorkbenchPanelList(
				document,
				{
					kind: 'workspace',
					projectId: 'project-1',
					workspaceId: 'workspace-1',
				},
				'missing-tab',
			),
		).toEqual({ kind: 'rejected', reason: 'tab-not-found' });
	});
});
