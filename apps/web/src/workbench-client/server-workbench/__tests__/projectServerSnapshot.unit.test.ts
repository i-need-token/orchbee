import type { WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import {
	activeTerminalContext,
	createSnapshotRuntime,
	FIXTURE_INTL,
	FIXTURE_WORKER_ID,
	fixtureBinding,
	fixtureId,
	oneWorkspaceSnapshot,
	publishSnapshot,
	snapshotContext,
	snapshotDirectory,
	snapshotPanel,
	snapshotPresentation,
	snapshotTab,
	snapshotWorkspace,
	workbenchSnapshot,
} from '../../../__tests__/server-workbench-snapshot';
import type { WorkbenchProjectWorkspaceRuntime } from '../../../workbench';
import { projectServerSnapshot } from '../projectServerSnapshot';
import { ServerTerminalEndObserver } from '../server-terminal-ends';

/** Adopts `next` as the composition does, with what the window saw end. */
function publishWithEnds(
	runtime: WorkbenchProjectWorkspaceRuntime,
	next: WorkbenchSnapshot,
	terminalEnds: ServerTerminalEndObserver,
): void {
	terminalEnds.prune(next);
	runtime.applyPublishedWorkbench(projectServerSnapshot(next, FIXTURE_INTL, terminalEnds.ends));
}

function activeLeaf(runtime: WorkbenchProjectWorkspaceRuntime, tabIndex = 0) {
	const root =
		activeTerminalContext(runtime).panelWorkspaceStore.getSnapshot().tabs[tabIndex]?.root;
	if (root?.kind !== 'panel') throw new Error('The fixture Tab is not one Panel.');
	return root;
}

describe('projectServerSnapshot', () => {
	it('shows Projects and Workspaces in display order and opens the first Workspace', () => {
		const runtime = createSnapshotRuntime(
			workbenchSnapshot(
				1,
				{
					directories: [snapshotDirectory(1), snapshotDirectory(2)],
					projectOrder: [fixtureId(0x202), fixtureId(0x201)],
					projects: [
						{ label: 'First identity', projectId: fixtureId(0x201) },
						{ label: 'Second identity', projectId: fixtureId(0x202) },
					],
					workspaceOrder: [fixtureId(0x402), fixtureId(0x401)],
					workspaces: [
						snapshotWorkspace(1, { projectId: fixtureId(0x201) }),
						snapshotWorkspace(2, { projectId: fixtureId(0x202) }),
					],
				},
				[snapshotContext(1, [snapshotTab(1)]), snapshotContext(2, [snapshotTab(2)])],
			),
		);

		const state = runtime.getSnapshot();
		expect(state.projects.map((project) => project.id)).toEqual([
			fixtureId(0x202),
			fixtureId(0x201),
		]);
		expect(state.workspaces.map((candidate) => candidate.id)).toEqual([
			fixtureId(0x402),
			fixtureId(0x401),
		]);
		expect(state.activeLocation).toEqual({ kind: 'workspace', workspaceId: fixtureId(0x402) });
	});

	it('attaches only to a bound terminal whose launch did not fail', () => {
		const runtime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [
				snapshotTab(1),
				snapshotTab(2, snapshotPanel(2, snapshotPresentation(2, { binding: null }))),
				snapshotTab(
					3,
					snapshotPanel(
						3,
						snapshotPresentation(3, {
							launchFailure: { executable: null, reason: 'terminal-create-failed' },
						}),
					),
				),
			]),
		);

		expect(activeLeaf(runtime, 0).primaryPresentation.lifecycle.kind).toBe('running');
		// Nothing is launching it, so it never reads as starting.
		expect(activeLeaf(runtime, 1).primaryPresentation.lifecycle).toEqual({ kind: 'unbound' });
		expect(activeLeaf(runtime, 2).primaryPresentation.lifecycle).toEqual({
			failure: { code: 'terminal-create-failed' },
			kind: 'launch-failed',
		});
	});

	it('gives every Client the same Split sizes from the shared ratio', () => {
		const split = (ratio: object) => ({
			children: [
				{ node: snapshotPanel(1), slotId: fixtureId(0xa01) },
				{ node: snapshotPanel(2), slotId: fixtureId(0xa02) },
			],
			kind: 'split',
			orientation: 'horizontal',
			splitId: fixtureId(0xb01),
			...ratio,
		});
		const runtime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [snapshotTab(1, split({ ratio: 0.3 }))]),
		);
		const weights = (): readonly number[] => {
			const root = activeTerminalContext(runtime).panelWorkspaceStore.getSnapshot().tabs[0]?.root;
			if (root?.kind !== 'split') throw new Error('The fixture Tab is not one Split.');
			return root.children.map((child) => child.weight);
		};

		expect(weights()).toEqual([30, 70]);
		publishSnapshot(runtime, oneWorkspaceSnapshot(2, [snapshotTab(1, split({}))]));
		expect(weights()).toEqual([50, 50]);
	});

	it('holds a Workspace that has no directory without inventing one', () => {
		const runtime = createSnapshotRuntime(
			workbenchSnapshot(1, { workspaces: [snapshotWorkspace(1, { primaryDirectoryId: null })] }, [
				snapshotContext(1, []),
			]),
		);

		expect(runtime.getSnapshot().workspaces).toMatchObject([{ directory: null }]);
		expect(activeTerminalContext(runtime).tabModel.getSnapshot().tabs).toEqual([]);
	});

	it('lets a name a person chose outrank what the terminal reports', () => {
		const named = snapshotPanel(1, snapshotPresentation(1, { customLabel: 'build' }));
		const runtime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [snapshotTab(1, named, { automaticLabel: true })]),
		);
		const presentationId = activeLeaf(runtime).primaryPresentation.presentationId;

		runtime.applyTerminalMetadataObservation(presentationId, {
			currentDirectory: { currentCwd: 'C:\\elsewhere', kind: 'reported' },
			naturalTitle: 'pwsh',
		});

		const [title] = activeTerminalContext(runtime)
			.tabModel.getSnapshot()
			.tabs.map((candidate) => candidate.title);
		expect(title).toBe('build');
		expect(activeLeaf(runtime).primaryPresentation.currentCwd).toBe('C:\\elsewhere');
	});

	it('names a terminal that shows no title by its launched program, else by its Tab place', () => {
		const runtime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [
				snapshotTab(1, snapshotPanel(1, snapshotPresentation(1, { launchProgram: 'pwsh.exe' })), {
					automaticLabel: true,
				}),
				snapshotTab(
					2,
					snapshotPanel(
						2,
						snapshotPresentation(2, {
							launchFailure: {
								executable: 'C:\\tools\\node.exe',
								reason: 'terminal-create-failed',
							},
						}),
					),
					{ automaticLabel: true },
				),
				snapshotTab(3, snapshotPanel(3, snapshotPresentation(3, { binding: null })), {
					automaticLabel: true,
				}),
				snapshotTab(4, snapshotPanel(4, snapshotPresentation(4, { launchProgram: 'cmd.exe' })), {
					label: 'build',
				}),
			]),
		);

		const titles = activeTerminalContext(runtime)
			.tabModel.getSnapshot()
			.tabs.map((candidate) => candidate.title);
		expect(titles).toEqual(['PowerShell', 'node', 'Tab 3', 'build']);
	});
});

describe('applyPublishedWorkbench', () => {
	it('keeps the live models of a context that did not change', () => {
		const navigation = {
			directories: [snapshotDirectory(1), snapshotDirectory(2)],
			workspaces: [snapshotWorkspace(1), snapshotWorkspace(2)],
		};
		const runtime = createSnapshotRuntime(
			workbenchSnapshot(1, navigation, [
				snapshotContext(1, [snapshotTab(1)]),
				snapshotContext(2, [snapshotTab(2)]),
			]),
		);
		const untouched = runtime.getTerminalContext(fixtureId(0x501));
		const changed = runtime.getTerminalContext(fixtureId(0x502));

		publishSnapshot(
			runtime,
			workbenchSnapshot(2, navigation, [
				snapshotContext(1, [snapshotTab(1)]),
				snapshotContext(2, [snapshotTab(2), snapshotTab(3)]),
			]),
		);

		expect(runtime.getTerminalContext(fixtureId(0x501))).toBe(untouched);
		expect(runtime.getTerminalContext(fixtureId(0x502))).not.toBe(changed);
	});

	it('keeps this Client selection and what a terminal reported when its context changes', () => {
		const runtime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [snapshotTab(1), snapshotTab(2)]),
		);
		const secondTabId = fixtureId(0x602);
		activeTerminalContext(runtime).tabModel.selectTab(secondTabId);
		const presentationId = activeLeaf(runtime, 1).primaryPresentation.presentationId;
		runtime.applyTerminalMetadataObservation(presentationId, {
			currentDirectory: { kind: 'ignored' },
			naturalTitle: 'vim',
		});

		publishSnapshot(
			runtime,
			oneWorkspaceSnapshot(2, [snapshotTab(1), snapshotTab(2), snapshotTab(3)]),
		);

		const tabs = activeTerminalContext(runtime).tabModel.getSnapshot();
		expect(tabs.activeTabId).toBe(secondTabId);
		expect(tabs.tabs).toHaveLength(3);
		expect(activeLeaf(runtime, 1).primaryPresentation.naturalTitle).toBe('vim');
	});

	it('ends a terminal once this window sees its Host settle it, and not the one bound after it', () => {
		let published = oneWorkspaceSnapshot(1, [snapshotTab(1)]);
		const runtime = createSnapshotRuntime(published);
		const terminalEnds = new ServerTerminalEndObserver(
			() => published,
			() => undefined,
		);
		const settled = snapshotPresentation(1);
		const failed = { failure: { code: 'host-lost', retryable: false }, kind: 'failed' } as const;

		terminalEnds.observe(settled.presentationId, fixtureBinding(1), failed);
		// The Server publishes nothing new when a terminal ends, so only the window's own sight changes.
		publishWithEnds(runtime, published, terminalEnds);
		expect(activeLeaf(runtime).primaryPresentation.lifecycle).toEqual(failed);

		const rebound = snapshotPresentation(1, {
			binding: { terminalId: fixtureId(0x9ff), workerId: FIXTURE_WORKER_ID },
		});
		published = oneWorkspaceSnapshot(2, [snapshotTab(1, snapshotPanel(1, rebound))]);
		publishWithEnds(runtime, published, terminalEnds);
		expect(activeLeaf(runtime).primaryPresentation.lifecycle.kind).toBe('running');
	});

	it('sees a clean exit of all a Panel holds after a second, unless the Server closed it', () => {
		vi.useFakeTimers();
		try {
			let published = oneWorkspaceSnapshot(1, [snapshotTab(1), snapshotTab(2)]);
			const runtime = createSnapshotRuntime(published);
			let changes = 0;
			const terminalEnds = new ServerTerminalEndObserver(
				() => published,
				() => {
					changes += 1;
				},
			);
			const exited = { exitCode: 0, kind: 'exited', signal: null } as const;
			const [closing, kept] = [snapshotPresentation(1), snapshotPresentation(2)];

			terminalEnds.observe(closing.presentationId, fixtureBinding(1), exited);
			terminalEnds.observe(kept.presentationId, fixtureBinding(2), exited);
			vi.advanceTimersByTime(999);
			publishWithEnds(runtime, published, terminalEnds);
			expect(activeLeaf(runtime, 1).primaryPresentation.lifecycle.kind).toBe('running');

			// The Server's Close removes the first Tab within the hold, and the second one stays.
			published = oneWorkspaceSnapshot(2, [snapshotTab(2)]);
			publishWithEnds(runtime, published, terminalEnds);
			vi.advanceTimersByTime(1);
			expect(changes).toBe(1);
			publishWithEnds(runtime, published, terminalEnds);
			expect(activeLeaf(runtime).primaryPresentation.lifecycle).toEqual(exited);
		} finally {
			vi.useRealTimers();
		}
	});

	it('expands a Project it has not seen and leaves a collapsed one collapsed', () => {
		const firstProject = { label: 'First', projectId: fixtureId(0x201) };
		const runtime = createSnapshotRuntime(
			workbenchSnapshot(
				1,
				{
					directories: [snapshotDirectory(1)],
					projects: [firstProject],
					workspaces: [snapshotWorkspace(1, { projectId: firstProject.projectId })],
				},
				[snapshotContext(1, [snapshotTab(1)])],
			),
		);
		expect(runtime.getSnapshot().expandedProjectIds).toEqual([firstProject.projectId]);
		runtime.transition({ projectId: firstProject.projectId, type: 'project-expansion-toggled' });

		const secondProject = { label: 'Second', projectId: fixtureId(0x202) };
		publishSnapshot(
			runtime,
			workbenchSnapshot(
				2,
				{
					directories: [snapshotDirectory(1)],
					projects: [firstProject, secondProject],
					workspaces: [snapshotWorkspace(1, { projectId: firstProject.projectId })],
				},
				[snapshotContext(1, [snapshotTab(1)])],
			),
		);

		expect(runtime.getSnapshot().expandedProjectIds).toEqual([secondProject.projectId]);
	});

	it('expands a Project once when the recovered view of a reloaded document expands it', () => {
		const project = { label: 'First', projectId: fixtureId(0x201) };
		const runtime = createSnapshotRuntime(
			workbenchSnapshot(
				1,
				{
					directories: [snapshotDirectory(1)],
					projects: [project],
					workspaces: [snapshotWorkspace(1, { projectId: project.projectId })],
				},
				[snapshotContext(1, [snapshotTab(1)])],
			),
			{ expandedProjectIds: [project.projectId] },
		);

		expect(runtime.getClientView().expandedProjectIds).toEqual([project.projectId]);
	});
});
