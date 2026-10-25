import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
} from '@orchbee/workbench/document';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
} from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it } from 'vitest';

import { validateDesktopClientRemotePlacementProposal } from '../validateDesktopClientRemotePlacementProposal';

function emptyDocument(): WorkbenchSharedDocument {
	return createWorkbenchSharedDocument({
		projectWorkspace: {
			nextResourceScopeSequence: 1,
			resourceScopes: [],
			workspaces: [],
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
			projects: [{ id: 'project-1', name: 'orchbee' }],
			resourceScopes: [
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
					id: 'resource-scope-1',
					launchConfigurations: [],
					name: 'orchbee',
					primaryDirectoryId: 'environment-directory:00000000000000000000000000000001',
				},
			],
			workspaces: [
				{
					projectId: 'project-1',
					resourceScopeId: 'resource-scope-1',
					id: 'workspace-1',
					launchConfigurationId: null,
					name: 'orchbee',
					primaryDirectoryId: 'environment-directory:00000000000000000000000000000001',
					terminalContextId: 'terminal-context-2',
				},
			],
			nextResourceScopeSequence: 2,
		},
		terminalContexts: [
			...empty.terminalContexts,
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
				terminalContextId: 'terminal-context-2',
			} satisfies WorkbenchSharedDocument['terminalContexts'][number],
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
		const project = canonical.projectWorkspace.resourceScopes[0];
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
						resourceScopes: [{ ...project, directories: [replacement] }],
					},
				}),
			).toBe(false);
		}
		expect(
			validateDesktopClientRemotePlacementProposal(canonical, {
				...canonical,
				projectWorkspace: {
					...canonical.projectWorkspace,
					resourceScopes: [{ ...project, id: 'other-scope' }],
				},
			}),
		).toBe(false);
		expect(
			validateDesktopClientRemotePlacementProposal(canonical, {
				...canonical,
				projectWorkspace: {
					...canonical.projectWorkspace,
					resourceScopes: [
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
		const project = canonical.projectWorkspace.resourceScopes[0];
		const workspace = canonical.projectWorkspace.workspaces[0];
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
						workspaces: [replacement],
					},
				}),
			).toBe(false);
		}

		const duplicateContext = {
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
			terminalContextId: 'terminal-context-3',
		} satisfies WorkbenchSharedDocument['terminalContexts'][number];
		const duplicated = createWorkbenchSharedDocument({
			projectWorkspace: {
				...canonical.projectWorkspace,
				nextTerminalContextSequence: 4,
				nextWorkspaceSequence: 3,
				workspaces: [
					workspace,
					{
						...workspace,
						id: 'workspace-2',
						terminalContextId: duplicateContext.terminalContextId,
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

	it('allows an exact remote Tab close request without allowing Terminal mutation', () => {
		const canonical = remoteDocumentWithTerminal();
		const remoteContext = canonical.terminalContexts[1];
		const tab = remoteContext?.panelWorkspace.tabs[0];
		if (remoteContext === undefined || tab?.root.kind !== 'panel') {
			throw new Error('Missing remote terminal fixture.');
		}
		const removalId = terminalPresentationRemovalId('remote-removal');
		const closing = createWorkbenchSharedDocument({
			projectWorkspace: canonical.projectWorkspace,
			revision: workbenchSharedDocumentRevision('1'),
			terminalContexts: [
				canonical.terminalContexts[0] ?? remoteContext,
				{
					...remoteContext,
					panelWorkspace: {
						...remoteContext.panelWorkspace,
						pendingRemovals: [
							{
								kind: 'tab',
								presentationIds: [tab.root.primaryPresentation.presentationId],
								removalId,
								tabId: tab.tabId,
							},
						],
						tabs: [
							{
								...tab,
								root: {
									...tab.root,
									primaryPresentation: {
										...tab.root.primaryPresentation,
										intent: {
											kind: 'close',
											operationId: terminalPresentationOperationId('remote-close'),
											removalId,
										},
									},
								},
							},
						],
					},
				},
			],
		});

		expect(validateDesktopClientRemotePlacementProposal(canonical, closing)).toBe(true);
		const closingContext = closing.terminalContexts[1];
		const closingTab = closingContext?.panelWorkspace.tabs[0];
		if (closingContext === undefined || closingTab?.root.kind !== 'panel') {
			throw new Error('Missing closing remote terminal fixture.');
		}
		expect(
			validateDesktopClientRemotePlacementProposal(canonical, {
				...closing,
				terminalContexts: [
					closing.terminalContexts[0] ?? closingContext,
					{
						...closingContext,
						panelWorkspace: {
							...closingContext.panelWorkspace,
							tabs: [
								{
									...closingTab,
									root: {
										...closingTab.root,
										primaryPresentation: {
											...closingTab.root.primaryPresentation,
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

	it('allows renderer-owned remote Tab display metadata without allowing topology changes', () => {
		const canonical = remoteDocumentWithTerminal();
		const remoteContext = canonical.terminalContexts[1];
		const tab = remoteContext?.tabCollection.tabs[0];
		if (remoteContext === undefined || tab === undefined) {
			throw new Error('Missing remote terminal fixture.');
		}
		const retitled = createWorkbenchSharedDocument({
			projectWorkspace: canonical.projectWorkspace,
			revision: workbenchSharedDocumentRevision('1'),
			terminalContexts: [
				canonical.terminalContexts[0] ?? remoteContext,
				{
					...remoteContext,
					tabCollection: {
						...remoteContext.tabCollection,
						tabs: [
							{
								...tab,
								customTitle: 'Remote work',
								pinned: true,
								title: 'remote-host',
							},
						],
					},
				},
			],
		});

		expect(validateDesktopClientRemotePlacementProposal(canonical, retitled)).toBe(true);
		expect(
			validateDesktopClientRemotePlacementProposal(canonical, {
				...retitled,
				terminalContexts: [
					retitled.terminalContexts[0] ?? remoteContext,
					{
						...remoteContext,
						tabCollection: {
							...remoteContext.tabCollection,
							tabs: [{ ...tab, id: 'forged-tab' }],
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
