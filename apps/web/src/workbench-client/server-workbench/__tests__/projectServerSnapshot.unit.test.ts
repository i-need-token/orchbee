import { createOrchbeeIntl } from '@orchbee/i18n';
import { WorkbenchProjectWorkspaceRuntime } from '@orchbee/workbench';
import {
	parseWorkbenchResponse,
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
	type WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';
import { describe, expect, it } from 'vitest';

import { projectServerSnapshot } from '../projectServerSnapshot';

const DEFAULT_DIRECTORY = 'C:\\Users\\someone';
const INTL = createOrchbeeIntl('en-US');
const WORKER_ID = hex(0x2);

function hex(value: number): string {
	return value.toString(16).padStart(32, '0');
}

function presentation(sequence: number, overrides: object = {}) {
	return {
		binding: { terminalId: hex(0x900 + sequence), workerId: WORKER_ID },
		customLabel: null,
		label: `Terminal ${sequence}`,
		presentationId: hex(0x800 + sequence),
		...overrides,
	};
}

function panel(sequence: number, primary: object = presentation(sequence)) {
	return {
		childPresentations: [],
		kind: 'panel',
		panelId: hex(0x700 + sequence),
		primaryPresentation: primary,
	};
}

function tab(sequence: number, root: object = panel(sequence), overrides: object = {}) {
	return {
		groupId: null,
		label: `Tab ${sequence}`,
		pinned: false,
		root,
		tabId: hex(0x600 + sequence),
		...overrides,
	};
}

function context(sequence: number, tabs: readonly object[]) {
	return { groups: [], tabs, terminalContextId: hex(0x500 + sequence) };
}

function workspace(sequence: number, overrides: object = {}) {
	return {
		label: `Workspace ${sequence}`,
		primaryDirectoryId: hex(0x300 + sequence),
		projectId: null,
		terminalContextId: hex(0x500 + sequence),
		workspaceId: hex(0x400 + sequence),
		...overrides,
	};
}

function directory(sequence: number) {
	return {
		directoryId: hex(0x300 + sequence),
		path: `C:\\work\\directory-${sequence}`,
		workerId: WORKER_ID,
	};
}

function snapshot(
	revision: number,
	navigation: { readonly workspaces: readonly object[] } & Record<string, unknown>,
	terminalContexts: readonly object[],
): WorkbenchSnapshot {
	const response = parseWorkbenchResponse({
		kind: 'workbench.snapshot.result',
		outcome: {
			kind: 'available',
			snapshot: {
				kind: 'workbench.snapshot',
				navigation: { directories: [], projects: [], ...navigation },
				protocolVersion: 1,
				revision: `${revision}`,
				sessionId: hex(0x1),
				terminalContexts,
			},
		},
		protocolVersion: 1,
		requestId: hex(0xf00),
	});
	if (response.kind !== 'workbench.snapshot.result' || response.outcome.kind !== 'available') {
		throw new Error('The fixture is not an available snapshot.');
	}
	return response.outcome.snapshot;
}

function oneWorkspaceSnapshot(revision: number, tabs: readonly object[]): WorkbenchSnapshot {
	return snapshot(revision, { directories: [directory(1)], workspaces: [workspace(1)] }, [
		context(1, tabs),
	]);
}

function createRuntime(initial: WorkbenchSnapshot): WorkbenchProjectWorkspaceRuntime {
	let sequence = 0;
	return new WorkbenchProjectWorkspaceRuntime({
		initialPublication: projectServerSnapshot(initial, DEFAULT_DIRECTORY, INTL),
		standaloneCwd: DEFAULT_DIRECTORY,
		standaloneTerminalTitle: 'Terminal',
		terminalPresentation: {
			createOperationId: () => terminalPresentationOperationId(`operation:${(sequence += 1)}`),
			createPresentationId: () => terminalPresentationId(`presentation:${(sequence += 1)}`),
			createRemovalId: () => terminalPresentationRemovalId(`removal:${(sequence += 1)}`),
			dispatchCommands: () => undefined,
			initialPresentationId: terminalPresentationId('presentation:initial'),
			kind: 'managed',
		},
	});
}

function publish(runtime: WorkbenchProjectWorkspaceRuntime, next: WorkbenchSnapshot): void {
	runtime.applyPublishedWorkbench(projectServerSnapshot(next, DEFAULT_DIRECTORY, INTL));
}

function activeLeaf(runtime: WorkbenchProjectWorkspaceRuntime, tabIndex = 0) {
	const root = runtime.getActiveTerminalContext().panelWorkspaceStore.getSnapshot().tabs[
		tabIndex
	]?.root;
	if (root?.kind !== 'panel') throw new Error('The fixture Tab is not one Panel.');
	return root;
}

describe('projectServerSnapshot', () => {
	it('shows Projects and Workspaces in display order and opens the first Workspace', () => {
		const runtime = createRuntime(
			snapshot(
				1,
				{
					directories: [directory(1), directory(2)],
					projectOrder: [hex(0x202), hex(0x201)],
					projects: [
						{ label: 'First identity', projectId: hex(0x201) },
						{ label: 'Second identity', projectId: hex(0x202) },
					],
					workspaceOrder: [hex(0x402), hex(0x401)],
					workspaces: [
						workspace(1, { projectId: hex(0x201) }),
						workspace(2, { projectId: hex(0x202) }),
					],
				},
				[context(1, [tab(1)]), context(2, [tab(2)])],
			),
		);

		const state = runtime.getSnapshot();
		expect(state.projects.map((project) => project.id)).toEqual([hex(0x202), hex(0x201)]);
		expect(state.workspaces.map((candidate) => candidate.id)).toEqual([hex(0x402), hex(0x401)]);
		expect(state.activeLocation).toEqual({ kind: 'workspace', workspaceId: hex(0x402) });
	});

	it('attaches only to a bound terminal whose launch did not fail', () => {
		const runtime = createRuntime(
			oneWorkspaceSnapshot(1, [
				tab(1),
				tab(2, panel(2, presentation(2, { binding: null }))),
				tab(
					3,
					panel(
						3,
						presentation(3, {
							launchFailure: { executable: null, reason: 'terminal-create-failed' },
						}),
					),
				),
			]),
		);

		expect(activeLeaf(runtime, 0).primaryPresentation.lifecycle.kind).toBe('running');
		// Nothing is launching it, so it never reads as provisioning.
		expect(activeLeaf(runtime, 1).primaryPresentation.lifecycle).toMatchObject({
			failure: { code: 'terminal-unbound', retryable: false },
			kind: 'provision-failed',
		});
		expect(activeLeaf(runtime, 2).primaryPresentation.lifecycle).toMatchObject({
			failure: { code: 'terminal-create-failed' },
			kind: 'provision-failed',
		});
	});

	it('gives every Client the same Split sizes from the shared ratio', () => {
		const split = (ratio: object) => ({
			children: [
				{ node: panel(1), slotId: hex(0xa01) },
				{ node: panel(2), slotId: hex(0xa02) },
			],
			kind: 'split',
			orientation: 'horizontal',
			splitId: hex(0xb01),
			...ratio,
		});
		const runtime = createRuntime(oneWorkspaceSnapshot(1, [tab(1, split({ ratio: 0.3 }))]));
		const weights = (): readonly number[] => {
			const root = runtime.getActiveTerminalContext().panelWorkspaceStore.getSnapshot()
				.tabs[0]?.root;
			if (root?.kind !== 'split') throw new Error('The fixture Tab is not one Split.');
			return root.children.map((child) => child.weight);
		};

		expect(weights()).toEqual([30, 70]);
		publish(runtime, oneWorkspaceSnapshot(2, [tab(1, split({}))]));
		expect(weights()).toEqual([50, 50]);
	});

	it('holds a Workspace that has no directory without inventing an available one', () => {
		const runtime = createRuntime(
			snapshot(1, { workspaces: [workspace(1, { primaryDirectoryId: null })] }, [context(1, [])]),
		);

		const [scope] = runtime.getSnapshot().resourceScopes;
		expect(scope?.directories).toMatchObject([{ available: false, path: DEFAULT_DIRECTORY }]);
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toEqual([]);
	});

	it('lets a name a person chose outrank what the terminal reports', () => {
		const named = panel(1, presentation(1, { customLabel: 'build' }));
		const runtime = createRuntime(
			oneWorkspaceSnapshot(1, [tab(1, named, { automaticLabel: true })]),
		);
		const presentationId = activeLeaf(runtime).primaryPresentation.presentationId;

		runtime.applyTerminalMetadataObservation(presentationId, {
			closeActivity: 'unknown',
			currentDirectory: { currentCwd: 'C:\\elsewhere', kind: 'reported' },
			naturalTitle: 'pwsh',
		});

		const [title] = runtime
			.getActiveTerminalContext()
			.tabModel.getSnapshot()
			.tabs.map((candidate) => candidate.title);
		expect(title).toBe('build');
		expect(activeLeaf(runtime).primaryPresentation.currentCwd).toBe('C:\\elsewhere');
	});

	it('names a terminal that shows no title by its launched program, else by its Tab place', () => {
		const runtime = createRuntime(
			oneWorkspaceSnapshot(1, [
				tab(1, panel(1, presentation(1, { launchProgram: 'pwsh.exe' })), { automaticLabel: true }),
				tab(
					2,
					panel(
						2,
						presentation(2, {
							launchFailure: {
								executable: 'C:\\tools\\node.exe',
								reason: 'terminal-create-failed',
							},
						}),
					),
					{ automaticLabel: true },
				),
				tab(3, panel(3, presentation(3, { binding: null })), { automaticLabel: true }),
				tab(4, panel(4, presentation(4, { launchProgram: 'cmd.exe' })), { label: 'build' }),
			]),
		);

		const titles = runtime
			.getActiveTerminalContext()
			.tabModel.getSnapshot()
			.tabs.map((candidate) => candidate.title);
		expect(titles).toEqual(['PowerShell', 'node', 'Tab 3', 'build']);
	});
});

describe('applyPublishedWorkbench', () => {
	it('keeps the live models of a context that did not change', () => {
		const navigation = {
			directories: [directory(1), directory(2)],
			workspaces: [workspace(1), workspace(2)],
		};
		const runtime = createRuntime(
			snapshot(1, navigation, [context(1, [tab(1)]), context(2, [tab(2)])]),
		);
		const untouched = runtime.getTerminalContext(hex(0x501));
		const changed = runtime.getTerminalContext(hex(0x502));

		publish(runtime, snapshot(2, navigation, [context(1, [tab(1)]), context(2, [tab(2), tab(3)])]));

		expect(runtime.getTerminalContext(hex(0x501))).toBe(untouched);
		expect(runtime.getTerminalContext(hex(0x502))).not.toBe(changed);
	});

	it('keeps this Client selection and what a terminal reported when its context changes', () => {
		const runtime = createRuntime(oneWorkspaceSnapshot(1, [tab(1), tab(2)]));
		const secondTabId = hex(0x602);
		runtime.getActiveTerminalContext().tabModel.selectTab(secondTabId);
		const presentationId = activeLeaf(runtime, 1).primaryPresentation.presentationId;
		runtime.applyTerminalMetadataObservation(presentationId, {
			closeActivity: 'unknown',
			currentDirectory: { kind: 'ignored' },
			naturalTitle: 'vim',
		});

		publish(runtime, oneWorkspaceSnapshot(2, [tab(1), tab(2), tab(3)]));

		const tabs = runtime.getActiveTerminalContext().tabModel.getSnapshot();
		expect(tabs.activeTabId).toBe(secondTabId);
		expect(tabs.tabs).toHaveLength(3);
		expect(activeLeaf(runtime, 1).primaryPresentation.naturalTitle).toBe('vim');
	});

	it('moves to the first Workspace when its Workspace is gone, and to none when all are', () => {
		const runtime = createRuntime(
			snapshot(
				1,
				{
					directories: [directory(1), directory(2), directory(3)],
					workspaces: [workspace(1), workspace(2), workspace(3)],
				},
				[context(1, [tab(1)]), context(2, [tab(2)]), context(3, [tab(3)])],
			),
		);
		runtime.transition({ type: 'workspace-activated', workspaceId: hex(0x403) });

		publish(
			runtime,
			snapshot(
				2,
				{ directories: [directory(1), directory(2)], workspaces: [workspace(1), workspace(2)] },
				[context(1, [tab(1)]), context(2, [tab(2)])],
			),
		);
		expect(runtime.getSnapshot().activeLocation).toEqual({
			kind: 'workspace',
			workspaceId: hex(0x401),
		});

		publish(runtime, snapshot(3, { workspaces: [] }, []));
		expect(runtime.getSnapshot().activeLocation).toEqual({ kind: 'standalone' });
		expect(runtime.getActiveTerminalContext().tabModel.getSnapshot().tabs).toEqual([]);
	});

	it('expands a Project it has not seen and leaves a collapsed one collapsed', () => {
		const firstProject = { label: 'First', projectId: hex(0x201) };
		const runtime = createRuntime(
			snapshot(
				1,
				{
					directories: [directory(1)],
					projects: [firstProject],
					workspaces: [workspace(1, { projectId: firstProject.projectId })],
				},
				[context(1, [tab(1)])],
			),
		);
		expect(runtime.getSnapshot().expandedProjectIds).toEqual([firstProject.projectId]);
		runtime.transition({ projectId: firstProject.projectId, type: 'project-expansion-toggled' });

		const secondProject = { label: 'Second', projectId: hex(0x202) };
		publish(
			runtime,
			snapshot(
				2,
				{
					directories: [directory(1)],
					projects: [firstProject, secondProject],
					workspaces: [workspace(1, { projectId: firstProject.projectId })],
				},
				[context(1, [tab(1)])],
			),
		);

		expect(runtime.getSnapshot().expandedProjectIds).toEqual([secondProject.projectId]);
	});
});
