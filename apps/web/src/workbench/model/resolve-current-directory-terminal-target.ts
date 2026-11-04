import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	findPanel,
	getDisplayedTerminal,
	type PanelWorkspaceState,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';

export interface CurrentDirectoryTerminalTarget {
	readonly panelId: string;
	readonly presentationId: TerminalPresentationId;
	readonly tabId: string;
	readonly terminalContextId: string;
}

export type CurrentDirectoryTerminalTargetResolution =
	| {
			readonly cwd: string;
			readonly kind: 'ready';
			readonly target: CurrentDirectoryTerminalTarget;
	  }
	| {
			readonly kind: 'unavailable';
			readonly reason:
				| 'active-tab-unavailable'
				| 'panel-unavailable'
				| 'terminal-current-directory-unavailable'
				| 'terminal-target-stale';
	  };

interface ResolveCurrentDirectoryTerminalTargetOptions {
	readonly terminalContextId: string;
	readonly activeTabId: string | null;
	readonly state: PanelWorkspaceState;
	readonly targetPanelId: string | null;
}

interface RevalidateCurrentDirectoryTerminalTargetOptions {
	readonly activeTabId: string | null;
	readonly state: PanelWorkspaceState;
	readonly target: CurrentDirectoryTerminalTarget;
	readonly terminalContextId: string;
}

export function resolveCurrentDirectoryTerminalTarget({
	activeTabId,
	state,
	targetPanelId,
	terminalContextId,
}: ResolveCurrentDirectoryTerminalTargetOptions): CurrentDirectoryTerminalTargetResolution {
	if (activeTabId === null) {
		return { kind: 'unavailable', reason: 'active-tab-unavailable' };
	}
	const tab = state.tabs.find((candidate) => candidate.tabId === activeTabId);
	if (tab === undefined) {
		return { kind: 'unavailable', reason: 'active-tab-unavailable' };
	}
	const panelId = targetPanelId ?? tab.focusedPanelId;
	const panel = findPanel(tab.root, panelId);
	if (panel === null) {
		return { kind: 'unavailable', reason: 'panel-unavailable' };
	}
	const terminal = getDisplayedTerminal(panel);
	const cwd = terminal.currentCwd;
	if (cwd === null || cwd.length === 0) {
		return { kind: 'unavailable', reason: 'terminal-current-directory-unavailable' };
	}
	return {
		cwd,
		kind: 'ready',
		target: {
			panelId,
			presentationId: terminal.presentationId,
			tabId: tab.tabId,
			terminalContextId,
		},
	};
}

export function revalidateCurrentDirectoryTerminalTarget({
	activeTabId,
	state,
	target,
	terminalContextId,
}: RevalidateCurrentDirectoryTerminalTargetOptions): CurrentDirectoryTerminalTargetResolution {
	if (target.terminalContextId !== terminalContextId || target.tabId !== activeTabId) {
		return { kind: 'unavailable', reason: 'terminal-target-stale' };
	}
	const resolution = resolveCurrentDirectoryTerminalTarget({
		activeTabId,
		state,
		targetPanelId: target.panelId,
		terminalContextId,
	});
	if (
		resolution.kind === 'unavailable' ||
		resolution.target.presentationId !== target.presentationId
	) {
		return { kind: 'unavailable', reason: 'terminal-target-stale' };
	}
	return resolution;
}
