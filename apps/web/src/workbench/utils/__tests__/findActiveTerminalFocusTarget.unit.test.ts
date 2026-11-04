// @vitest-environment jsdom

import { beforeEach, describe, expect, it } from 'vitest';

import {
	activeTerminalContext,
	createSnapshotRuntime,
	oneWorkspaceSnapshot,
	snapshotTab,
} from '../../../__tests__/server-workbench-snapshot';
import type { WorkbenchProjectWorkspaceRuntime } from '../../model/WorkbenchProjectWorkspaceRuntime';
import { findActiveTerminalFocusTarget } from '../findActiveTerminalFocusTarget';

function createRuntime(): WorkbenchProjectWorkspaceRuntime {
	return createSnapshotRuntime(oneWorkspaceSnapshot(1, [snapshotTab(1)]));
}

describe('findActiveTerminalFocusTarget', () => {
	beforeEach(() => document.body.replaceChildren());

	it('prefers the terminal textbox in the active Tab focused Panel', () => {
		const runtime = createRuntime();
		const activeTab = activeTerminalContext(runtime).panelWorkspaceStore.getSnapshot().tabs[0];
		if (activeTab === undefined) throw new Error('Expected the initial Tab.');
		const viewport = document.createElement('section');
		viewport.dataset['slot'] = 'workbench-terminal-viewport';
		viewport.dataset['panelId'] = activeTab.focusedPanelId;
		const textbox = document.createElement('textarea');
		viewport.append(textbox);
		document.body.append(viewport);

		expect(findActiveTerminalFocusTarget(runtime)).toBe(textbox);
	});

	it('falls back to the matching viewport and rejects another Panel', () => {
		const runtime = createRuntime();
		const activeTab = activeTerminalContext(runtime).panelWorkspaceStore.getSnapshot().tabs[0];
		if (activeTab === undefined) throw new Error('Expected the initial Tab.');
		const otherViewport = document.createElement('section');
		otherViewport.dataset['slot'] = 'workbench-terminal-viewport';
		otherViewport.dataset['panelId'] = 'another-panel';
		const activeViewport = document.createElement('section');
		activeViewport.dataset['slot'] = 'workbench-terminal-viewport';
		activeViewport.dataset['panelId'] = activeTab.focusedPanelId;
		document.body.append(otherViewport, activeViewport);

		expect(findActiveTerminalFocusTarget(runtime)).toBe(activeViewport);
		activeViewport.remove();
		expect(findActiveTerminalFocusTarget(runtime)).toBeNull();
	});
});
