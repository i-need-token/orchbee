import {
	PANEL_ACTION_IDS,
	type PanelCloseInput,
	type PanelSplitInput,
	type PanelSwapInput,
	type PanelTargetInput,
	type PanelVisualSelectionInput,
} from '../components/workbench-panel-workspace/actions/panel-action-contracts';
import {
	collectPanelLeaves,
	findPanel,
	type PanelSplitDirection,
	type PanelWorkspaceState,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import {
	resolveDirectionalPanelFocus,
	type PanelFocusDirection,
} from '../components/workbench-panel-workspace/model/resolveDirectionalPanelFocus';
import {
	resolveCyclicPanelFocus,
	type PanelFocusCycleDirection,
} from '../components/workbench-panel-workspace/model/resolveCyclicPanelFocus';
import {
	startPanelVisualSelection,
	type PanelVisualSelectionMode,
	type PanelVisualSelectionStartResult,
} from '../components/workbench-panel-workspace/model/panel-visual-selection';

export type ActiveTerminalCommand =
	| 'close'
	| 'equalize'
	| 'focus-down'
	| 'focus-left'
	| 'focus-next'
	| 'focus-previous'
	| 'focus-right'
	| 'focus-up'
	| 'focus-visible'
	| 'move-down'
	| 'move-left'
	| 'move-right'
	| 'move-up'
	| 'start-resize'
	| 'split-above'
	| 'split-below'
	| 'split-left'
	| 'split-right'
	| 'swap-visible'
	| 'toggle-maximization';

const PANEL_SPLIT_DIRECTION_BY_COMMAND = {
	'split-above': 'above',
	'split-below': 'below',
	'split-left': 'left',
	'split-right': 'right',
} as const satisfies Readonly<
	Record<'split-above' | 'split-below' | 'split-left' | 'split-right', PanelSplitDirection>
>;

const PANEL_FOCUS_DIRECTION_BY_COMMAND = {
	'focus-down': 'down',
	'focus-left': 'left',
	'focus-right': 'right',
	'focus-up': 'up',
} as const satisfies Readonly<
	Record<'focus-down' | 'focus-left' | 'focus-right' | 'focus-up', PanelFocusDirection>
>;

const PANEL_MOVE_DIRECTION_BY_COMMAND = {
	'move-down': 'down',
	'move-left': 'left',
	'move-right': 'right',
	'move-up': 'up',
} as const satisfies Readonly<
	Record<Extract<ActiveTerminalCommand, `move-${string}`>, PanelFocusDirection>
>;

const PANEL_FOCUS_CYCLE_DIRECTION_BY_COMMAND = {
	'focus-next': 'next',
	'focus-previous': 'previous',
} as const satisfies Readonly<Record<'focus-next' | 'focus-previous', PanelFocusCycleDirection>>;

const PANEL_VISUAL_SELECTION_UNAVAILABLE_REASON_BY_REASON = {
	'invalid-panel-order': 'panel-unavailable',
	'panel-maximized': 'panel-maximized',
	'requires-multiple-panels': 'requires-multiple-panels',
	'source-panel-unavailable': 'panel-unavailable',
} as const satisfies Readonly<
	Record<
		Extract<PanelVisualSelectionStartResult, { kind: 'unavailable' }>['reason'],
		'panel-maximized' | 'panel-unavailable' | 'requires-multiple-panels'
	>
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
			readonly actionId: typeof PANEL_ACTION_IDS.startVisualSelection;
			readonly input: PanelVisualSelectionInput;
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
	if (findPanel(activeTab.root, panelId) === null) {
		return { kind: 'unavailable', reason: 'panel-unavailable' };
	}

	switch (command) {
		// As in the TUI, Close ends the whole Panel, and the Server closes a last Panel's Tab with it.
		case 'close':
			return {
				actionId: PANEL_ACTION_IDS.close,
				input: { panelId, tabId: activeTabId },
				kind: 'ready',
				panelId,
			};
		case 'split-above':
		case 'split-below':
		case 'split-left':
		case 'split-right':
			return {
				actionId: PANEL_ACTION_IDS.split,
				input: { direction: PANEL_SPLIT_DIRECTION_BY_COMMAND[command], panelId },
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
		case 'focus-next':
		case 'focus-previous': {
			if (activeTab.maximizedPanelId !== null) {
				return { kind: 'unavailable', reason: 'panel-maximized' };
			}
			const targetId = resolveCyclicPanelFocus({
				direction: PANEL_FOCUS_CYCLE_DIRECTION_BY_COMMAND[command],
				maximizedPanelId: activeTab.maximizedPanelId,
				root: activeTab.root,
				sourcePanelId: panelId,
			});
			return targetId === null
				? { kind: 'unavailable', reason: 'requires-multiple-panels' }
				: {
						actionId: PANEL_ACTION_IDS.focus,
						input: { panelId: targetId },
						kind: 'ready',
						panelId: targetId,
					};
		}
		case 'focus-visible':
		case 'swap-visible': {
			const mode: PanelVisualSelectionMode = command === 'focus-visible' ? 'focus' : 'swap';
			const selection = startPanelVisualSelection({
				maximizedPanelId: activeTab.maximizedPanelId,
				mode,
				root: activeTab.root,
				sourcePanelId: panelId,
				tabId: activeTabId,
			});
			if (selection.kind === 'unavailable') {
				return {
					kind: 'unavailable',
					reason: PANEL_VISUAL_SELECTION_UNAVAILABLE_REASON_BY_REASON[selection.reason],
				};
			}
			if (selection.kind === 'direct') {
				return selection.command.kind === 'focus'
					? {
							actionId: PANEL_ACTION_IDS.focus,
							input: { panelId: selection.command.targetPanelId },
							kind: 'ready',
							panelId: selection.command.targetPanelId,
						}
					: {
							actionId: PANEL_ACTION_IDS.swap,
							input: {
								panelId: selection.command.sourcePanelId,
								targetPanelId: selection.command.targetPanelId,
							},
							kind: 'ready',
							panelId: selection.command.sourcePanelId,
						};
			}
			return {
				actionId: PANEL_ACTION_IDS.startVisualSelection,
				input: { mode, panelId },
				kind: 'ready',
				panelId,
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
