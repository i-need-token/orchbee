import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	activeTerminalContext,
	createSnapshotRuntime,
	fixtureContextId,
	fixturePanelId,
	fixturePresentationId,
	fixtureTabId,
	oneWorkspaceSnapshot,
	snapshotPanel,
	snapshotPresentation,
	snapshotSplit,
	snapshotTab,
} from '../../../__tests__/server-workbench-snapshot';
import type { WorkbenchProjectWorkspaceRuntime } from '../WorkbenchProjectWorkspaceRuntime';

describe('current-directory terminal target resolution', () => {
	it('uses the reported current CWD and treats an empty report as none', () => {
		const runtime = createSnapshotRuntime(oneWorkspaceSnapshot(1, [snapshotTab(1)]));
		expect(runtime.resolveActiveTerminalCurrentDirectoryTarget()).toEqual({
			kind: 'unavailable',
			reason: 'terminal-current-directory-unavailable',
		});

		reportCurrentDirectory(runtime, 1, '/reported');
		expect(runtime.resolveActiveTerminalCurrentDirectoryTarget()).toEqual({
			cwd: '/reported',
			kind: 'ready',
			target: {
				panelId: fixturePanelId(1),
				presentationId: presentation(1),
				tabId: fixtureTabId(1),
				terminalContextId: fixtureContextId(1),
			},
		});
		for (const currentCwd of [null, '']) {
			reportCurrentDirectory(runtime, 1, currentCwd);
			expect(runtime.resolveActiveTerminalCurrentDirectoryTarget()).toEqual({
				kind: 'unavailable',
				reason: 'terminal-current-directory-unavailable',
			});
		}
	});

	it('uses the displayed child and gives an explicit Panel target precedence over focus', () => {
		const runtime = createSnapshotRuntime(
			oneWorkspaceSnapshot(1, [
				snapshotTab(
					1,
					snapshotSplit(
						1,
						'horizontal',
						snapshotPanel(1),
						snapshotPanel(2, snapshotPresentation(2), [snapshotPresentation(3)]),
					),
				),
			]),
		);
		reportCurrentDirectory(runtime, 1, '/primary');
		reportCurrentDirectory(runtime, 2, '/second');
		reportCurrentDirectory(runtime, 3, '/child');
		const store = activeTerminalContext(runtime).panelWorkspaceStore;
		store.transition({ panelId: fixturePanelId(2), tabId: fixtureTabId(1), type: 'panel-focused' });
		store.transition({
			childPresentationId: presentation(3),
			panelId: fixturePanelId(2),
			tabId: fixtureTabId(1),
			type: 'child-terminal-activated',
		});

		expect(runtime.resolveActiveTerminalCurrentDirectoryTarget()).toMatchObject({
			cwd: '/child',
			target: { panelId: fixturePanelId(2), presentationId: presentation(3) },
		});
		expect(runtime.resolveActiveTerminalCurrentDirectoryTarget(fixturePanelId(1))).toMatchObject({
			cwd: '/primary',
			target: { panelId: fixturePanelId(1), presentationId: presentation(1) },
		});
	});

	it('rejects a missing active Tab and a missing Panel', () => {
		expect(
			createSnapshotRuntime(
				oneWorkspaceSnapshot(1, []),
			).resolveActiveTerminalCurrentDirectoryTarget(),
		).toEqual({ kind: 'unavailable', reason: 'active-tab-unavailable' });
		expect(
			createSnapshotRuntime(
				oneWorkspaceSnapshot(1, [snapshotTab(1)]),
			).resolveActiveTerminalCurrentDirectoryTarget('missing'),
		).toEqual({ kind: 'unavailable', reason: 'panel-unavailable' });
	});

	it('revalidates every captured identity while accepting a newer CWD from the same terminal', () => {
		const runtime = createSnapshotRuntime(oneWorkspaceSnapshot(1, [snapshotTab(1)]));
		reportCurrentDirectory(runtime, 1, '/launch');
		const captured = runtime.resolveActiveTerminalCurrentDirectoryTarget();
		if (captured.kind !== 'ready') throw new Error('Expected a current-directory target.');
		reportCurrentDirectory(runtime, 1, '/latest');

		expect(runtime.resolveTerminalCurrentDirectoryTarget(captured.target)).toMatchObject({
			cwd: '/latest',
			kind: 'ready',
		});
		for (const target of [
			{ ...captured.target, terminalContextId: fixtureContextId(2) },
			{ ...captured.target, tabId: fixtureTabId(2) },
			{ ...captured.target, panelId: fixturePanelId(2) },
			{ ...captured.target, presentationId: presentation(2) },
		]) {
			expect(runtime.resolveTerminalCurrentDirectoryTarget(target)).toEqual({
				kind: 'unavailable',
				reason: 'terminal-target-stale',
			});
		}
	});
});

function presentation(sequence: number) {
	return terminalPresentationId(fixturePresentationId(sequence));
}

function reportCurrentDirectory(
	runtime: WorkbenchProjectWorkspaceRuntime,
	sequence: number,
	currentCwd: string | null,
): void {
	runtime.applyTerminalMetadataObservation(presentation(sequence), {
		currentDirectory: { currentCwd, kind: 'reported' },
		naturalTitle: null,
	});
}
