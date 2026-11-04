import fc from 'fast-check';
import { describe, expect, it, vi } from 'vitest';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
	parseWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
} from '@orchbee/workbench-client-protocol';

import type { WorkbenchDirectory } from '../../../project-workspace';
import { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import {
	createWorkbenchPreviewTerminalPresentationComposition,
	type WorkbenchTerminalPresentationCommand,
} from '../../model/createWorkbenchPreviewTerminalPresentationComposition';
import type { WorkbenchLocationRef } from '../semantic-command-contracts';
import { executeWorkbenchTerminalCreate } from '../executeWorkbenchTerminalCreate';

const ALPHA_DIRECTORY: WorkbenchDirectory = {
	available: true,
	git: null,
	id: 'directory-alpha',
	kind: 'folder',
	name: 'alpha',
	path: '/projects/alpha',
	placement: { kind: 'local' },
	source: 'selected',
};

const ALPHA_UNAVAILABLE_DIRECTORY: WorkbenchDirectory = {
	...ALPHA_DIRECTORY,
	available: false,
	id: 'directory-alpha-unavailable',
	name: 'alpha unavailable',
	path: '/projects/alpha-unavailable',
};

const BETA_DIRECTORY: WorkbenchDirectory = {
	...ALPHA_DIRECTORY,
	id: 'directory-beta',
	name: 'beta',
	path: '/projects/beta',
};

interface Fixture {
	readonly commands: WorkbenchTerminalPresentationCommand[];
	readonly runtime: WorkbenchProjectWorkspaceRuntime;
}

function createFixture(): Fixture {
	let operationSequence = 1;
	let presentationSequence = 1;
	let removalSequence = 1;
	const commands: WorkbenchTerminalPresentationCommand[] = [];
	const runtime = new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Standalone',
		terminalPresentation: {
			createOperationId: () =>
				terminalPresentationOperationId(`create:operation:${operationSequence++}`),
			createPresentationId: () =>
				terminalPresentationId(`create:presentation:${presentationSequence++}`),
			createRemovalId: () => terminalPresentationRemovalId(`create:removal:${removalSequence++}`),
			dispatchCommands: (nextCommands) => commands.push(...nextCommands),
			initialPresentationId: terminalPresentationId('create:presentation:initial'),
			kind: 'managed',
		},
	});
	addProject(runtime, ALPHA_DIRECTORY, 'alpha');
	runtime.transition({
		directory: ALPHA_UNAVAILABLE_DIRECTORY,
		resourceScopeId: 'resource-scope-1',
		type: 'directory-added',
	});
	addProject(runtime, BETA_DIRECTORY, 'beta');
	return { commands, runtime };
}

function addProject(
	runtime: WorkbenchProjectWorkspaceRuntime,
	directory: WorkbenchDirectory,
	name: string,
): void {
	const result = runtime.transition({
		directory,
		launchConfigurations: [],
		primaryWorkspaceLaunchConfigurationId: null,
		primaryWorkspaceName: `${name} workspace`,
		projectName: name,
		terminalTitle: `${name} terminal`,
		type: 'project-created',
	});
	if (result.outcome !== 'applied') throw new Error(`Failed to create ${name}.`);
}

function location(workspaceId: string): WorkbenchLocationRef {
	return { kind: 'workspace', workspaceId };
}

function tabCounts(runtime: WorkbenchProjectWorkspaceRuntime): ReadonlyMap<string, number> {
	const counts = new Map<string, number>();
	for (const context of runtime.getClientView().terminalContexts) {
		const runtimeContext = runtime.getTerminalContext(context.terminalContextId);
		if (runtimeContext === null) throw new Error('Expected every Client View context to exist.');
		counts.set(context.terminalContextId, runtimeContext.tabModel.getSnapshot().tabs.length);
	}
	return counts;
}

function selectionProjection(runtime: WorkbenchProjectWorkspaceRuntime): unknown {
	const view = runtime.getClientView();
	return {
		activeLocation: view.activeLocation,
		terminalContexts: view.terminalContexts.map((context) => ({
			activeTabId: context.tabCollection.activeTabId,
			selectedTabIds: context.tabCollection.selectedTabIds,
			terminalContextId: context.terminalContextId,
		})),
	};
}

function documentFingerprint(runtime: WorkbenchProjectWorkspaceRuntime): string {
	return JSON.stringify(runtime.getSharedDocument(workbenchSharedDocumentRevision('0')));
}

describe('executeWorkbenchTerminalCreate', () => {
	it('creates one exact non-active terminal intent and returns its complete identity tuple', () => {
		const { commands, runtime } = createFixture();
		commands.length = 0;
		const beforeSelection = selectionProjection(runtime);
		const beforeCounts = tabCounts(runtime);
		const beforeCommit = vi.fn<() => void>();

		const result = executeWorkbenchTerminalCreate(
			runtime,
			{
				directoryId: ALPHA_DIRECTORY.id,
				fallbackTitle: 'Exact alpha',
				location: location('workspace-1'),
				placement: { groupId: null },
			},
			beforeCommit,
		);

		expect(result.kind).toBe('created');
		if (result.kind !== 'created') throw new Error(`Creation rejected: ${result.reason}`);
		expect(beforeCommit).toHaveBeenCalledOnce();
		expect(selectionProjection(runtime)).toEqual(beforeSelection);
		expect(runtime.getSnapshot().activeLocation).toEqual(location('workspace-2'));
		const afterCounts = tabCounts(runtime);
		expect(afterCounts.get(result.terminalContextId)).toBe(
			(beforeCounts.get(result.terminalContextId) ?? 0) + 1,
		);
		expect(
			new Map(
				[...afterCounts].filter(
					([terminalContextId]) => terminalContextId !== result.terminalContextId,
				),
			),
		).toEqual(
			new Map(
				[...beforeCounts].filter(
					([terminalContextId]) => terminalContextId !== result.terminalContextId,
				),
			),
		);
		const context = runtime.getTerminalContext(result.terminalContextId);
		const tab = context?.panelWorkspaceStore
			.getSnapshot()
			.tabs.find((candidate) => candidate.tabId === result.tabId);
		if (tab?.root.kind !== 'panel') throw new Error('Expected one new top-level Panel Tab.');
		expect(tab.root.id).toBe(result.panelId);
		expect(tab.root.childPresentations).toEqual([]);
		expect(tab.root.primaryPresentation).toMatchObject({
			launchCwd: ALPHA_DIRECTORY.path,
			presentationId: result.presentationId,
			title: 'Exact alpha',
		});
		if (result.intent.kind !== 'provision') {
			throw new Error('Expected managed creation to return one provision intent.');
		}
		expect(tab.root.primaryPresentation.lifecycle).toEqual({
			kind: 'provisioning',
			operationId: result.intent.operationId,
		});
		expect(commands).toEqual([
			{
				launchCwd: ALPHA_DIRECTORY.path,
				operationId: result.intent.operationId,
				presentationId: result.presentationId,
				type: 'terminal-provision-requested',
			},
		]);
		expect(() =>
			parseWorkbenchSharedDocument(runtime.getSharedDocument(workbenchSharedDocumentRevision('1'))),
		).not.toThrow();
	});

	it('uses the created Tab as the required fallback when the exact context is empty', () => {
		let presentationSequence = 1;
		const runtime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/system',
			standaloneTerminalTitle: 'Standalone',
			terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
				terminalPresentationId('empty:initial'),
				() => terminalPresentationId(`empty:presentation:${presentationSequence++}`),
			),
		});
		const context = runtime.getActiveTerminalContext();
		const initialTabId = context.tabModel.getSnapshot().tabs[0]?.id;
		if (initialTabId === undefined) throw new Error('Expected the initial standalone Tab.');
		context.tabModel.closeTab(initialTabId);
		context.panelWorkspaceStore.reconcileTabs();
		expect(context.tabModel.getSnapshot().activeTabId).toBeNull();

		const result = executeWorkbenchTerminalCreate(runtime, {
			fallbackTitle: 'Recreated',
			location: { kind: 'standalone' },
			placement: { groupId: null },
		});

		expect(result.kind).toBe('created');
		if (result.kind !== 'created') throw new Error(`Creation rejected: ${result.reason}`);
		expect(result.intent).toEqual({ kind: 'preview' });
		expect(context.tabModel.getSnapshot()).toMatchObject({
			activeTabId: result.tabId,
			tabs: [{ id: result.tabId }],
		});
	});

	it('rejects every pre-commit target, Directory, and placement failure without mutation', () => {
		const { runtime } = createFixture();
		const cases = [
			{
				expected: 'workspace-not-found',
				input: { location: location('missing') },
			},
			{
				expected: 'explicit-directory-not-found',
				input: {
					directoryId: BETA_DIRECTORY.id,
					location: location('workspace-1'),
				},
			},
			{
				expected: 'explicit-directory-unavailable',
				input: {
					directoryId: ALPHA_UNAVAILABLE_DIRECTORY.id,
					location: location('workspace-1'),
				},
			},
			{
				expected: 'tab-placement-invalid',
				input: {
					location: location('workspace-1'),
					placement: { groupId: 'missing-group' },
				},
			},
		] as const;

		for (const candidate of cases) {
			const beforeDocument = documentFingerprint(runtime);
			const beforeSelection = selectionProjection(runtime);
			const beforeCommit = vi.fn<() => void>();
			const result = executeWorkbenchTerminalCreate(
				runtime,
				{
					...('directoryId' in candidate.input ? { directoryId: candidate.input.directoryId } : {}),
					fallbackTitle: 'Rejected',
					location: candidate.input.location,
					placement: 'placement' in candidate.input ? candidate.input.placement : { groupId: null },
				},
				beforeCommit,
			);
			expect(result).toEqual({ kind: 'rejected', reason: candidate.expected });
			expect(beforeCommit).not.toHaveBeenCalled();
			expect(documentFingerprint(runtime)).toBe(beforeDocument);
			expect(selectionProjection(runtime)).toEqual(beforeSelection);
		}
	});

	it('rejects sealed creation and allocator conflicts before the commit callback', () => {
		const sealed = createFixture().runtime;
		sealed.requestAllTerminalSessionsStop();
		const sealedDocument = documentFingerprint(sealed);
		const sealedCommit = vi.fn<() => void>();
		expect(
			executeWorkbenchTerminalCreate(
				sealed,
				{
					fallbackTitle: 'Sealed',
					location: { kind: 'standalone' },
					placement: { groupId: null },
				},
				sealedCommit,
			),
		).toEqual({ kind: 'rejected', reason: 'terminal-creation-sealed' });
		expect(sealedCommit).not.toHaveBeenCalled();
		expect(documentFingerprint(sealed)).toBe(sealedDocument);

		const presentationConflict = createConflictRuntime('presentation');
		const presentationCommit = vi.fn<() => void>();
		expect(
			executeWorkbenchTerminalCreate(
				presentationConflict,
				{
					fallbackTitle: 'Conflict',
					location: { kind: 'standalone' },
					placement: { groupId: null },
				},
				presentationCommit,
			),
		).toEqual({ kind: 'rejected', reason: 'terminal-presentation-conflict' });
		expect(presentationCommit).not.toHaveBeenCalled();

		const operationConflict = createConflictRuntime('operation');
		const operationCommit = vi.fn<() => void>();
		expect(
			executeWorkbenchTerminalCreate(
				operationConflict,
				{
					fallbackTitle: 'Conflict',
					location: { kind: 'standalone' },
					placement: { groupId: null },
				},
				operationCommit,
			),
		).toEqual({ kind: 'rejected', reason: 'terminal-operation-conflict' });
		expect(operationCommit).not.toHaveBeenCalled();
	});

	it('preserves exact targeting, uniqueness, validity, and selection over fixed-seed traces', () => {
		expect(() =>
			fc.assert(
				fc.property(
					fc.array(
						fc.constantFrom(
							'alpha',
							'beta',
							'standalone',
							'stale-workspace',
							'foreign-directory',
							'unavailable-directory',
							'invalid-placement',
						),
						{ maxLength: 40 },
					),
					propertyTracePreservesInvariants,
				),
				{ numRuns: 80, seed: 20_260_827 },
			),
		).not.toThrow();
	});
});

function propertyTracePreservesInvariants(trace: readonly string[]): boolean {
	const { runtime } = createFixture();
	const identities = new Set<string>();
	for (const [index, step] of trace.entries()) {
		const beforeDocument = documentFingerprint(runtime);
		const beforeCounts = tabCounts(runtime);
		const beforeSelection = selectionProjection(runtime);
		const command = propertyCommand(step, index);
		let commitCount = 0;
		const result = executeWorkbenchTerminalCreate(runtime, command.input, () => {
			commitCount += 1;
		});
		if (command.terminalContextId === null) {
			if (
				result.kind !== 'rejected' ||
				commitCount !== 0 ||
				documentFingerprint(runtime) !== beforeDocument
			) {
				return false;
			}
		} else {
			if (
				result.kind !== 'created' ||
				result.terminalContextId !== command.terminalContextId ||
				commitCount !== 1
			) {
				return false;
			}
			const resultIdentities = [
				`tab:${result.tabId}`,
				`panel:${result.terminalContextId}:${result.panelId}`,
				`presentation:${result.presentationId}`,
				...(result.intent.kind === 'provision' ? [`operation:${result.intent.operationId}`] : []),
			];
			if (resultIdentities.some((identity) => identities.has(identity))) return false;
			for (const identity of resultIdentities) identities.add(identity);

			const context = runtime.getTerminalContext(result.terminalContextId);
			const tab = context?.panelWorkspaceStore
				.getSnapshot()
				.tabs.find((candidate) => candidate.tabId === result.tabId);
			if (
				tab?.root.kind !== 'panel' ||
				tab.root.id !== result.panelId ||
				tab.root.childPresentations.length !== 0 ||
				tab.root.primaryPresentation.presentationId !== result.presentationId
			) {
				return false;
			}
			const afterCounts = tabCounts(runtime);
			if (afterCounts.size !== beforeCounts.size) return false;
			for (const [terminalContextId, count] of beforeCounts) {
				if (
					afterCounts.get(terminalContextId) !==
					count + (terminalContextId === command.terminalContextId ? 1 : 0)
				) {
					return false;
				}
			}
		}
		if (JSON.stringify(selectionProjection(runtime)) !== JSON.stringify(beforeSelection)) {
			return false;
		}
		try {
			parseWorkbenchSharedDocument(
				runtime.getSharedDocument(workbenchSharedDocumentRevision(String(index + 1))),
			);
		} catch {
			return false;
		}
	}
	return true;
}

function createConflictRuntime(
	conflict: 'operation' | 'presentation',
): WorkbenchProjectWorkspaceRuntime {
	let presentationSequence = 1;
	const sharedOperationId = terminalPresentationOperationId('conflict:operation');
	const sharedPresentationId = terminalPresentationId('conflict:presentation');
	return new WorkbenchProjectWorkspaceRuntime({
		standaloneCwd: '/system',
		standaloneTerminalTitle: 'Standalone',
		terminalPresentation: {
			createOperationId: () =>
				conflict === 'operation'
					? sharedOperationId
					: terminalPresentationOperationId(`conflict:operation:${presentationSequence}`),
			createPresentationId: () =>
				conflict === 'presentation'
					? sharedPresentationId
					: terminalPresentationId(`conflict:presentation:${presentationSequence++}`),
			createRemovalId: () => terminalPresentationRemovalId('conflict:removal'),
			dispatchCommands: () => undefined,
			initialPresentationId: sharedPresentationId,
			kind: 'managed',
		},
	});
}

function propertyCommand(
	step: string,
	index: number,
): {
	readonly input: Parameters<typeof executeWorkbenchTerminalCreate>[1];
	readonly terminalContextId: string | null;
} {
	const fallbackTitle = `Trace ${index}`;
	switch (step) {
		case 'alpha':
			return {
				input: {
					directoryId: ALPHA_DIRECTORY.id,
					fallbackTitle,
					location: location('workspace-1'),
					placement: { groupId: null },
				},
				terminalContextId: 'terminal-context-2',
			};
		case 'beta':
			return {
				input: {
					fallbackTitle,
					location: location('workspace-2'),
					placement: { groupId: null },
				},
				terminalContextId: 'terminal-context-3',
			};
		case 'standalone':
			return {
				input: {
					fallbackTitle,
					location: { kind: 'standalone' },
					placement: { groupId: null },
				},
				terminalContextId: 'terminal-context-1',
			};
		case 'stale-workspace':
			return rejectedPropertyCommand(fallbackTitle, location('missing'));
		case 'foreign-directory':
			return {
				input: {
					directoryId: BETA_DIRECTORY.id,
					fallbackTitle,
					location: location('workspace-1'),
					placement: { groupId: null },
				},
				terminalContextId: null,
			};
		case 'unavailable-directory':
			return {
				input: {
					directoryId: ALPHA_UNAVAILABLE_DIRECTORY.id,
					fallbackTitle,
					location: location('workspace-1'),
					placement: { groupId: null },
				},
				terminalContextId: null,
			};
		default:
			return {
				input: {
					fallbackTitle,
					location: location('workspace-1'),
					placement: { groupId: 'missing-group' },
				},
				terminalContextId: null,
			};
	}
}

function rejectedPropertyCommand(
	fallbackTitle: string,
	target: WorkbenchLocationRef,
): ReturnType<typeof propertyCommand> {
	return {
		input: { fallbackTitle, location: target, placement: { groupId: null } },
		terminalContextId: null,
	};
}
