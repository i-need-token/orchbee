import { TAB_STRIP_ACTION_IDS, type TabCloseInput } from '@orchbee/tab-strip';

import {
	PANEL_ACTION_IDS,
	type PanelCloseInput,
	type PanelSplitInput,
	type PanelTargetInput,
} from '../components/workbench-panel-workspace/actions/panel-action-contracts';
import {
	collectPanelLeaves,
	findPanel,
	type PanelWorkspaceState,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';

export type ActiveTerminalCommand = 'close' | 'split-below' | 'split-right' | 'toggle-maximization';

interface ResolveActiveTerminalCommandOptions {
	readonly activeTabId: string | null;
	readonly command: ActiveTerminalCommand;
	readonly state: PanelWorkspaceState;
	readonly targetPanelId: string | null;
}

export type ActiveTerminalCommandResolution =
	| {
			readonly actionId: typeof PANEL_ACTION_IDS.close;
			readonly input: PanelCloseInput;
			readonly kind: 'ready';
			readonly panelId: string;
	  }
	| {
			readonly actionId: typeof PANEL_ACTION_IDS.split;
			readonly input: PanelSplitInput;
			readonly kind: 'ready';
			readonly panelId: string;
	  }
	| {
			readonly actionId: typeof PANEL_ACTION_IDS.toggleMaximization;
			readonly input: PanelTargetInput;
			readonly kind: 'ready';
			readonly panelId: string;
	  }
	| {
			readonly actionId: typeof TAB_STRIP_ACTION_IDS.closeTabs;
			readonly input: TabCloseInput;
			readonly kind: 'ready';
			readonly panelId: string;
	  }
	| {
			readonly kind: 'unavailable';
			readonly reason: 'active-tab-unavailable' | 'panel-unavailable' | 'requires-multiple-panels';
	  };

export function resolveActiveTerminalCommand({
	activeTabId,
	command,
	state,
	targetPanelId,
}: ResolveActiveTerminalCommandOptions): ActiveTerminalCommandResolution {
	if (activeTabId === null) {
		return { kind: 'unavailable', reason: 'active-tab-unavailable' };
	}
	const activeTab = state.tabs.find((tab) => tab.tabId === activeTabId);
	if (activeTab === undefined) {
		return { kind: 'unavailable', reason: 'active-tab-unavailable' };
	}
	const panelId = targetPanelId ?? activeTab.focusedPanelId;
	const panel = findPanel(activeTab.root, panelId);
	if (panel === null) {
		return { kind: 'unavailable', reason: 'panel-unavailable' };
	}

	switch (command) {
		case 'close':
			if (
				panel.activeChildPresentationId !== null ||
				collectPanelLeaves(activeTab.root).length > 1
			) {
				return {
					actionId: PANEL_ACTION_IDS.close,
					input: {
						childPresentationId: panel.activeChildPresentationId,
						panelId,
						tabId: activeTabId,
					},
					kind: 'ready',
					panelId,
				};
			}
			return {
				actionId: TAB_STRIP_ACTION_IDS.closeTabs,
				input: { closeSource: 'non-ui', tabIds: [activeTabId] },
				kind: 'ready',
				panelId,
			};
		case 'split-below':
			return {
				actionId: PANEL_ACTION_IDS.split,
				input: { direction: 'below', panelId },
				kind: 'ready',
				panelId,
			};
		case 'split-right':
			return {
				actionId: PANEL_ACTION_IDS.split,
				input: { direction: 'right', panelId },
				kind: 'ready',
				panelId,
			};
		case 'toggle-maximization':
			return collectPanelLeaves(activeTab.root).length < 2
				? { kind: 'unavailable', reason: 'requires-multiple-panels' }
				: {
						actionId: PANEL_ACTION_IDS.toggleMaximization,
						input: { panelId },
						kind: 'ready',
						panelId,
					};
	}
}
