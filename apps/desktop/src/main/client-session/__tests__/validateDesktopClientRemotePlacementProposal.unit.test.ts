import {
	createEmptyWorkbenchSharedTerminalContext,
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
} from '@orchbee/workbench/document';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
} from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it } from 'vitest';

import { validateDesktopClientRemotePlacementProposal } from '../validateDesktopClientRemotePlacementProposal';

function emptyDocument(): WorkbenchSharedDocument {
	return createWorkbenchSharedDocument({
		projectWorkspace: {
			nextProjectSequence: 1,
			nextTerminalContextSequence: 2,
			nextWorkspaceSequence: 1,
			projects: [],
			standaloneTerminalContextId: 'terminal-context-1',
		},
		revision: workbenchSharedDocumentRevision('0'),
		terminalContexts: [
			{
				nextTabSequence: 1,
				panelWorkspace: {
					nextPanelSequence: 1,
					nextPanelSlotSequence: 1,
					nextSplitSequence: 1,
					nextTerminalSequence: 1,
					pendingRemovals: [],
					tabs: [],
				},
				tabCollection: { groups: [], tabs: [] },
				terminalContextId: 'terminal-context-1',
			},
		],
	});
}

function remoteDocument(): WorkbenchSharedDocument {
	const empty = emptyDocument();
	return {
		...empty,
		projectWorkspace: {
			...empty.projectWorkspace,
			nextProjectSequence: 2,
			nextTerminalContextSequence: 3,
			nextWorkspaceSequence: 2,
			projects: [
				{
					directories: [
						{
							available: false,
							git: null,
							id: 'environment-directory:00000000000000000000000000000001',
							kind: 'folder',
							name: 'orchbee',
							path: '/srv/orchbee',
							placement: {
								environmentId: 'environment:00000000000000000000000000000001',
								kind: 'environment',
							},
							source: 'selected',
						},
					],
					id: 'project-1',
					launchConfigurations: [],
					name: 'orchbee',
					primaryDirectoryId: 'environment-directory:00000000000000000000000000000001',
					workspaces: [
						{
							id: 'workspace-1',
							launchConfigurationId: null,
							name: 'orchbee',
							primaryDirectoryId: 'environment-directory:00000000000000000000000000000001',
							terminalContextId: 'terminal-context-2',
						},
					],
				},
			],
		},
		terminalContexts: [
			...empty.terminalContexts,
			createEmptyWorkbenchSharedTerminalContext('terminal-context-2'),
		],
	};
}

describe('validateDesktopClientRemotePlacementProposal', () => {
	it('allows exact retention and deletion of a canonical remote Directory', () => {
		const canonical = remoteDocument();
		expect(
			validateDesktopClientRemotePlacementProposal(canonical, {
				...canonical,
				revision: workbenchSharedDocumentRevision('1'),
			}),
		).toBe(true);
		expect(validateDesktopClientRemotePlacementProposal(canonical, emptyDocument())).toBe(true);
	});

	it('rejects client introduction, mutation, relabeling, and reparenting', () => {
		const canonical = remoteDocument();
		const project = canonical.projectWorkspace.projects[0];
		const directory = project?.directories[0];
		if (project === undefined || directory === undefined) throw new Error('Missing fixture.');

		expect(validateDesktopClientRemotePlacementProposal(null, canonical)).toBe(false);
		for (const replacement of [
			{ ...directory, path: '/srv/forged' },
			{ ...directory, available: true },
			{ ...directory, placement: { kind: 'local' as const } },
			{
				...directory,
				placement: {
					environmentId: 'environment:ffffffffffffffffffffffffffffffff',
					kind: 'environment' as const,
				},
			},
		]) {
			expect(
				validateDesktopClientRemotePlacementProposal(canonical, {
					...canonical,
					projectWorkspace: {
						...canonical.projectWorkspace,
						projects: [{ ...project, directories: [replacement] }],
					},
				}),
			).toBe(false);
		}
		expect(
			validateDesktopClientRemotePlacementProposal(canonical, {
				...canonical,
				projectWorkspace: {
					...canonical.projectWorkspace,
					projects: [{ ...project, id: 'project-2' }],
				},
			}),
		).toBe(false);
		expect(
			validateDesktopClientRemotePlacementProposal(canonical, {
				...canonical,
				projectWorkspace: {
					...canonical.projectWorkspace,
					projects: [
						{
							...project,
							directories: [{ ...directory, placement: { kind: 'local' } }],
							id: 'project-2',
						},
					],
				},
			}),
		).toBe(false);
	});

	it('rejects remote Workspace mutation, duplication, and protected identity reuse', () => {
		const canonical = remoteDocument();
		const project = canonical.projectWorkspace.projects[0];
		const workspace = project?.workspaces[0];
		if (project === undefined || workspace === undefined) throw new Error('Missing fixture.');

		for (const replacement of [
			{ ...workspace, name: 'forged' },
			{ ...workspace, id: 'workspace-forged' },
			{ ...workspace, terminalContextId: 'terminal-context-1' },
		]) {
			expect(
				validateDesktopClientRemotePlacementProposal(canonical, {
					...canonical,
					projectWorkspace: {
						...canonical.projectWorkspace,
						projects: [{ ...project, workspaces: [replacement] }],
					},
				}),
			).toBe(false);
		}

		const duplicateContext = createEmptyWorkbenchSharedTerminalContext('terminal-context-3');
		const duplicated = createWorkbenchSharedDocument({
			projectWorkspace: {
				...canonical.projectWorkspace,
				nextTerminalContextSequence: 4,
				nextWorkspaceSequence: 3,
				projects: [
					{
						...project,
						workspaces: [
							workspace,
							{
								...workspace,
								id: 'workspace-2',
								terminalContextId: duplicateContext.terminalContextId,
							},
						],
					},
				],
			},
			revision: workbenchSharedDocumentRevision('1'),
			terminalContexts: [...canonical.terminalContexts, duplicateContext],
		});
		expect(validateDesktopClientRemotePlacementProposal(canonical, duplicated)).toBe(false);
	});

	it('rejects client-injected terminal state in a remote Workspace context', () => {
		const canonical = remoteDocument();
		const remoteContext = canonical.terminalContexts[1];
		if (remoteContext === undefined) throw new Error('Missing remote context fixture.');
		const tabId = 'terminal-context-2-tab-1';
		const forged = createWorkbenchSharedDocument({
			projectWorkspace: canonical.projectWorkspace,
			revision: workbenchSharedDocumentRevision('1'),
			terminalContexts: [
				canonical.terminalContexts[0] ?? remoteContext,
				{
					...remoteContext,
					nextTabSequence: 2,
					panelWorkspace: {
						...remoteContext.panelWorkspace,
						nextPanelSequence: 2,
						nextTerminalSequence: 2,
						tabs: [
							{
								root: {
									childPresentations: [],
									id: 'panel-1',
									kind: 'panel',
									primaryPresentation: {
										intent: {
											kind: 'provision',
											operationId: terminalPresentationOperationId('forged-operation'),
										},
										launchCwd: '/local/fallback',
										presentationId: terminalPresentationId('forged-presentation'),
										title: 'Forged Terminal',
									},
								},
								tabId,
							},
						],
					},
					tabCollection: {
						groups: [],
						tabs: [
							{
								customTitle: null,
								group: null,
								id: tabId,
								pinned: false,
								title: 'Forged Terminal',
							},
						],
					},
				},
			],
		});

		expect(validateDesktopClientRemotePlacementProposal(canonical, forged)).toBe(false);
	});

	it('allows only the authorized remote provision-to-retain lifecycle convergence', () => {
		const canonical = remoteDocumentWithTerminal();
		const remoteContext = canonical.terminalContexts[1];
		const tab = remoteContext?.panelWorkspace.tabs[0];
		if (remoteContext === undefined || tab?.root.kind !== 'panel') {
			throw new Error('Missing remote terminal fixture.');
		}
		const retained = createWorkbenchSharedDocument({
			projectWorkspace: canonical.projectWorkspace,
			revision: workbenchSharedDocumentRevision('1'),
			terminalContexts: [
				canonical.terminalContexts[0] ?? remoteContext,
				{
					...remoteContext,
					panelWorkspace: {
						...remoteContext.panelWorkspace,
						tabs: [
							{
								...tab,
								root: {
									...tab.root,
									primaryPresentation: {
										...tab.root.primaryPresentation,
										intent: { kind: 'retain' },
									},
								},
							},
						],
					},
				},
			],
		});

		expect(validateDesktopClientRemotePlacementProposal(canonical, retained)).toBe(true);
		const retainedContext = retained.terminalContexts[1];
		const retainedTab = retainedContext?.panelWorkspace.tabs[0];
		if (retainedContext === undefined || retainedTab?.root.kind !== 'panel') {
			throw new Error('Missing retained remote terminal fixture.');
		}
		expect(
			validateDesktopClientRemotePlacementProposal(canonical, {
				...retained,
				terminalContexts: [
					retained.terminalContexts[0] ?? retainedContext,
					{
						...retainedContext,
						panelWorkspace: {
							...retainedContext.panelWorkspace,
							tabs: [
								{
									...retainedTab,
									root: {
										...retainedTab.root,
										primaryPresentation: {
											...retainedTab.root.primaryPresentation,
											launchCwd: '/forged',
										},
									},
								},
							],
						},
					},
				],
			}),
		).toBe(false);
	});
});

function remoteDocumentWithTerminal(): WorkbenchSharedDocument {
	const remote = remoteDocument();
	const remoteContext = remote.terminalContexts[1];
	if (remoteContext === undefined) throw new Error('Missing remote context fixture.');
	const tabId = 'terminal-context-2-tab-1';
	return createWorkbenchSharedDocument({
		projectWorkspace: remote.projectWorkspace,
		revision: remote.revision,
		terminalContexts: [
			remote.terminalContexts[0] ?? remoteContext,
			{
				...remoteContext,
				nextTabSequence: 2,
				panelWorkspace: {
					...remoteContext.panelWorkspace,
					nextPanelSequence: 2,
					nextTerminalSequence: 2,
					tabs: [
						{
							root: {
								childPresentations: [],
								id: 'panel-1',
								kind: 'panel',
								primaryPresentation: {
									intent: {
										kind: 'provision',
										operationId: terminalPresentationOperationId('main-operation'),
									},
									launchCwd: '/srv/orchbee',
									presentationId: terminalPresentationId('main-presentation'),
									title: 'Remote Terminal',
								},
							},
							tabId,
						},
					],
				},
				tabCollection: {
					groups: [],
					tabs: [
						{
							customTitle: null,
							group: null,
							id: tabId,
							pinned: false,
							title: 'Remote Terminal',
						},
					],
				},
			},
		],
	});
}
