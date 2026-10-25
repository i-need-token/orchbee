import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
} from '@orchbee/workbench/document';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it } from 'vitest';

import {
	resolveDesktopTerminalContextEffectTarget,
	resolveDesktopTerminalPresentationEffectTarget,
} from '../desktop-terminal-effect-target';

const presentationId = terminalPresentationId('presentation-1');

describe('resolveDesktopTerminalEffectTarget', () => {
	it('resolves a standalone Presentation only to the local runtime', () => {
		const document = terminalDocument();

		expect(resolveDesktopTerminalPresentationEffectTarget(document, presentationId)).toEqual({
			kind: 'local',
		});
	});

	it('rejects a Workspace Presentation with an Environment placement', () => {
		const document = terminalDocument('environment:00000000000000000000000000000001');
		expect(() => resolveDesktopTerminalPresentationEffectTarget(document, presentationId)).toThrow(
			'local Directory',
		);
	});

	it('fails closed when the Presentation is absent or appears in multiple contexts', () => {
		const document = terminalDocument();
		expect(() =>
			resolveDesktopTerminalPresentationEffectTarget(
				document,
				terminalPresentationId('missing-presentation'),
			),
		).toThrow('exactly one Terminal Context');

		const duplicateContext = {
			...document.terminalContexts[0],
			terminalContextId: 'terminal-context-2',
		};
		const ambiguous = {
			...document,
			terminalContexts: [...document.terminalContexts, duplicateContext],
		} as WorkbenchSharedDocument;
		expect(() => resolveDesktopTerminalPresentationEffectTarget(ambiguous, presentationId)).toThrow(
			'exactly one Terminal Context',
		);
	});

	it('fails closed when one context contains the Presentation more than once', () => {
		const document = terminalDocument();
		const context = document.terminalContexts[0];
		const tab = context?.panelWorkspace.tabs[0];
		if (context === undefined || tab === undefined) throw new Error('Missing test terminal tab.');
		const ambiguous = {
			...document,
			terminalContexts: [
				{
					...context,
					panelWorkspace: {
						...context.panelWorkspace,
						tabs: [tab, { ...tab, tabId: 'tab-2' }],
					},
				},
			],
		} as WorkbenchSharedDocument;

		expect(() => resolveDesktopTerminalPresentationEffectTarget(ambiguous, presentationId)).toThrow(
			'exactly one Terminal Context',
		);
	});

	it('fails closed when a context has competing standalone and Workspace authority', () => {
		const remote = terminalDocument('environment:00000000000000000000000000000001');
		const ambiguous = {
			...remote,
			projectWorkspace: {
				...remote.projectWorkspace,
				resourceScopes: remote.projectWorkspace.resourceScopes.map((project) => ({
					...project,
					directories: project.directories.map((directory) => ({
						...directory,
						placement: { kind: 'local' as const },
					})),
				})),
				standaloneTerminalContextId: 'terminal-context-1',
			},
		} as WorkbenchSharedDocument;

		expect(() =>
			resolveDesktopTerminalContextEffectTarget(ambiguous, 'terminal-context-1'),
		).toThrow('exactly one effect target');
	});
});

function terminalDocument(environmentId?: string): WorkbenchSharedDocument {
	const remote = environmentId !== undefined;
	return createWorkbenchSharedDocument({
		projectWorkspace: {
			nextProjectSequence: remote ? 2 : 1,
			nextTerminalContextSequence: remote ? 3 : 2,
			nextWorkspaceSequence: remote ? 2 : 1,
			projects: remote ? [{ id: 'project-1', name: 'workspace' }] : [],
			resourceScopes: remote
				? [
						{
							directories: [
								{
									available: false,
									git: null,
									id: 'directory-1',
									kind: 'folder',
									name: 'workspace',
									path: '/workspace',
									placement: { environmentId, kind: 'environment' },
									source: 'selected',
								},
							],
							id: 'resource-scope-1',
							launchConfigurations: [],
							name: 'workspace',
							primaryDirectoryId: 'directory-1',
						},
					]
				: [],
			workspaces: remote
				? [
						{
							projectId: 'project-1',
							resourceScopeId: 'resource-scope-1',
							id: 'workspace-1',
							launchConfigurationId: null,
							name: 'workspace',
							primaryDirectoryId: 'directory-1',
							terminalContextId: 'terminal-context-1',
						},
					]
				: [],
			nextResourceScopeSequence: 2,
			standaloneTerminalContextId: remote ? 'terminal-context-2' : 'terminal-context-1',
		},
		revision: workbenchSharedDocumentRevision('0'),
		terminalContexts: [
			{
				nextTabSequence: 2,
				panelWorkspace: {
					nextPanelSequence: 2,
					nextPanelSlotSequence: 1,
					nextSplitSequence: 1,
					nextTerminalSequence: 2,
					pendingRemovals: [],
					tabs: [
						{
							root: {
								childPresentations: [],
								id: 'panel-1',
								kind: 'panel',
								primaryPresentation: {
									intent: { kind: 'retain' },
									launchCwd: '/workspace',
									presentationId,
									title: 'Terminal',
								},
							},
							tabId: 'tab-1',
						},
					],
				},
				tabCollection: {
					groups: [],
					tabs: [
						{
							customTitle: null,
							group: null,
							id: 'tab-1',
							pinned: false,
							title: 'Terminal',
						},
					],
				},
				terminalContextId: 'terminal-context-1',
			},
			...(remote
				? [
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
					]
				: []),
		],
	});
}
