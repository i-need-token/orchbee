import { TAB_STRIP_ACTION_IDS, type TabCloseInput } from '@orchbee/tab-strip';

import {
	PANEL_ACTION_IDS,
	type PanelCloseInput,
	type PanelSplitInput,
	type PanelSwapInput,
	type PanelTargetInput,
} from '../components/workbench-panel-workspace/actions/panel-action-contracts';
import {
	collectPanelLeaves,
	findPanel,
	type PanelWorkspaceState,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import {
	resolveDirectionalPanelFocus,
	type PanelFocusDirection,
} from '../components/workbench-panel-workspace/model/resolveDirectionalPanelFocus';

export type ActiveTerminalCommand =
	| 'close'
	| 'equalize'
	| 'focus-down'
	| 'focus-left'
	| 'focus-right'
	| 'focus-up'
	| 'move-down'
	| 'move-left'
	| 'move-right'
	| 'move-up'
	| 'start-resize'
	| 'split-below'
	| 'split-right'
	| 'toggle-maximization';

const PANEL_FOCUS_DIRECTION_BY_COMMAND = {
	'focus-down': 'down',
	'focus-left': 'left',
	'focus-right': 'right',
	'focus-up': 'up',
} as const satisfies Readonly<
	Record<Extract<ActiveTerminalCommand, `focus-${string}`>, PanelFocusDirection>
>;

const PANEL_MOVE_DIRECTION_BY_COMMAND = {
	'move-down': 'down',
	'move-left': 'left',
	'move-right': 'right',
	'move-up': 'up',
} as const satisfies Readonly<
	Record<Extract<ActiveTerminalCommand, `move-${string}`>, PanelFocusDirection>
>;

interface ResolveActiveTerminalCommandOptions {
	readonly activeTabId: string | null;
	readonly command: ActiveTerminalCommand;
	readonly state: PanelWorkspaceState;
	readonly targetPanelId: string | null;
}

export type ActiveTerminalCommandResolution =
	| {
			readonly actionId: typeof PANEL_ACTION_IDS.equalize;
			readonly input: PanelTargetInput;
			readonly kind: 'ready';
			readonly panelId: string;
	  }
	| {
			readonly actionId: typeof PANEL_ACTION_IDS.focus;
			readonly input: PanelTargetInput;
			readonly kind: 'ready';
			readonly panelId: string;
	  }
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
			readonly actionId: typeof PANEL_ACTION_IDS.startResize;
			readonly input: PanelTargetInput;
			readonly kind: 'ready';
			readonly panelId: string;
	  }
	| {
			readonly actionId: typeof PANEL_ACTION_IDS.swap;
			readonly input: PanelSwapInput;
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
			readonly reason:
				| 'active-tab-unavailable'
				| 'directional-neighbor-unavailable'
				| 'panel-maximized'
				| 'panel-unavailable'
				| 'requires-multiple-panels';
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
		case 'equalize':
			if (activeTab.maximizedPanelId !== null) {
				return { kind: 'unavailable', reason: 'panel-maximized' };
			}
			return collectPanelLeaves(activeTab.root).length < 2
				? { kind: 'unavailable', reason: 'requires-multiple-panels' }
				: {
						actionId: PANEL_ACTION_IDS.equalize,
						input: { panelId },
						kind: 'ready',
						panelId,
					};
		case 'start-resize':
			return {
				actionId: PANEL_ACTION_IDS.startResize,
				input: { panelId },
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
		case 'focus-down':
		case 'focus-left':
		case 'focus-right':
		case 'focus-up': {
			if (activeTab.maximizedPanelId !== null) {
				return { kind: 'unavailable', reason: 'panel-maximized' };
			}
			const targetId = resolveDirectionalPanelFocus(
				activeTab.root,
				panelId,
				PANEL_FOCUS_DIRECTION_BY_COMMAND[command],
			);
			return targetId === null
				? { kind: 'unavailable', reason: 'directional-neighbor-unavailable' }
				: {
						actionId: PANEL_ACTION_IDS.focus,
						input: { panelId: targetId },
						kind: 'ready',
						panelId: targetId,
					};
		}
		case 'move-down':
		case 'move-left':
		case 'move-right':
		case 'move-up': {
			if (activeTab.maximizedPanelId !== null) {
				return { kind: 'unavailable', reason: 'panel-maximized' };
			}
			const directionalTargetPanelId = resolveDirectionalPanelFocus(
				activeTab.root,
				panelId,
				PANEL_MOVE_DIRECTION_BY_COMMAND[command],
			);
			return directionalTargetPanelId === null
				? { kind: 'unavailable', reason: 'directional-neighbor-unavailable' }
				: {
						actionId: PANEL_ACTION_IDS.swap,
						input: { panelId, targetPanelId: directionalTargetPanelId },
						kind: 'ready',
						panelId,
					};
		}
	}
}
