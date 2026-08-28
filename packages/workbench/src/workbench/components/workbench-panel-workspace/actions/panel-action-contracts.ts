import type { ActionInputValidation, ActionSignature } from '@orchbee/actions';

import {
	TerminalPresentationIdSchema,
	type TerminalPresentationId,
} from '../../../../terminal-presentation';
import type { PanelSplitDirection } from '../model/panel-workspace-state';
import type { PanelVisualSelectionMode } from '../model/panel-visual-selection';
import type { PanelResizeAdjustment, PanelResizeMultiplier } from '../model/resolvePanelResize';

export const PANEL_ACTION_IDS = {
	activateChildTerminal: 'workbench.panel.child-terminal.activate',
	close: 'workbench.panel.close',
	createChildTerminal: 'workbench.panel.child-terminal.create',
	equalize: 'workbench.panel.equalize',
	focus: 'workbench.panel.focus',
	move: 'workbench.panel.move',
	resize: 'workbench.panel.resize',
	startBroadcastInput: 'workbench.panel.broadcast-input.start',
	showPrimaryTerminal: 'workbench.panel.primary-terminal.show',
	split: 'workbench.panel.split',
	startResize: 'workbench.panel.resize.start',
	startVisualSelection: 'workbench.panel.visual-selection.start',
	swap: 'workbench.panel.swap',
	toggleMaximization: 'workbench.panel.maximization.toggle',
} as const;

export interface PanelTargetInput {
	readonly panelId: string;
}

export interface PanelCloseInput extends PanelTargetInput {
	readonly childPresentationId: TerminalPresentationId | null;
	readonly tabId: string;
}

export interface PanelChildTerminalInput extends PanelTargetInput {
	readonly childPresentationId: TerminalPresentationId;
}

export interface PanelSplitInput extends PanelTargetInput {
	readonly direction: PanelSplitDirection;
}

export interface PanelMoveInput extends PanelTargetInput {
	readonly direction: PanelSplitDirection;
	readonly targetPanelId: string;
}

export interface PanelResizeInput extends PanelTargetInput {
	readonly adjustment: PanelResizeAdjustment;
	readonly multiplier: PanelResizeMultiplier;
}

export interface PanelSwapInput extends PanelTargetInput {
	readonly targetPanelId: string;
}

export interface PanelVisualSelectionInput extends PanelTargetInput {
	readonly mode: PanelVisualSelectionMode;
}

export interface PanelActionCatalog {
	readonly [PANEL_ACTION_IDS.activateChildTerminal]: ActionSignature<PanelChildTerminalInput>;
	readonly [PANEL_ACTION_IDS.close]: ActionSignature<PanelCloseInput>;
	readonly [PANEL_ACTION_IDS.createChildTerminal]: ActionSignature<PanelTargetInput>;
	readonly [PANEL_ACTION_IDS.equalize]: ActionSignature<PanelTargetInput>;
	readonly [PANEL_ACTION_IDS.focus]: ActionSignature<PanelTargetInput>;
	readonly [PANEL_ACTION_IDS.move]: ActionSignature<PanelMoveInput>;
	readonly [PANEL_ACTION_IDS.resize]: ActionSignature<PanelResizeInput>;
	readonly [PANEL_ACTION_IDS.startBroadcastInput]: ActionSignature<PanelTargetInput>;
	readonly [PANEL_ACTION_IDS.showPrimaryTerminal]: ActionSignature<PanelTargetInput>;
	readonly [PANEL_ACTION_IDS.split]: ActionSignature<PanelSplitInput, string>;
	readonly [PANEL_ACTION_IDS.startResize]: ActionSignature<PanelTargetInput>;
	readonly [PANEL_ACTION_IDS.startVisualSelection]: ActionSignature<PanelVisualSelectionInput>;
	readonly [PANEL_ACTION_IDS.swap]: ActionSignature<PanelSwapInput>;
	readonly [PANEL_ACTION_IDS.toggleMaximization]: ActionSignature<PanelTargetInput>;
}

function isRecord(input: unknown): input is Readonly<Record<string, unknown>> {
	return typeof input === 'object' && input !== null && !Array.isArray(input);
}

function isDirection(input: unknown): input is PanelSplitDirection {
	return input === 'above' || input === 'below' || input === 'left' || input === 'right';
}

function isResizeAdjustment(input: unknown): input is PanelResizeAdjustment {
	return (
		input === 'narrower' ||
		input === 'reset' ||
		input === 'shorter' ||
		input === 'taller' ||
		input === 'wider'
	);
}

export const panelTargetInput: ActionInputValidation<PanelTargetInput> = {
	name: 'PanelTargetInput',
	parse: (input) =>
		isRecord(input) && typeof input['panelId'] === 'string'
			? { success: true, data: { panelId: input['panelId'] } }
			: { success: false, code: 'expected-panel-target' },
};

export const panelVisualSelectionInput: ActionInputValidation<PanelVisualSelectionInput> = {
	name: 'PanelVisualSelectionInput',
	parse: (input) =>
		isRecord(input) &&
		typeof input['panelId'] === 'string' &&
		(input['mode'] === 'focus' || input['mode'] === 'swap')
			? { success: true, data: { mode: input['mode'], panelId: input['panelId'] } }
			: { success: false, code: 'expected-panel-visual-selection' },
};

export const panelCloseInput: ActionInputValidation<PanelCloseInput> = {
	name: 'PanelCloseInput',
	parse: (input) => {
		if (
			!isRecord(input) ||
			typeof input['panelId'] !== 'string' ||
			typeof input['tabId'] !== 'string'
		) {
			return { success: false, code: 'expected-panel-close-target' };
		}
		const childPresentation =
			input['childPresentationId'] === null
				? null
				: TerminalPresentationIdSchema.safeParse(input['childPresentationId']);
		return childPresentation === null || childPresentation.success
			? {
					success: true,
					data: {
						childPresentationId: childPresentation === null ? null : childPresentation.data,
						panelId: input['panelId'],
						tabId: input['tabId'],
					},
				}
			: { success: false, code: 'expected-panel-close-target' };
	},
};

export const panelChildTerminalInput: ActionInputValidation<PanelChildTerminalInput> = {
	name: 'PanelChildTerminalInput',
	parse: (input) => {
		if (!isRecord(input) || typeof input['panelId'] !== 'string') {
			return { success: false, code: 'expected-panel-child-terminal' };
		}
		const childPresentation = TerminalPresentationIdSchema.safeParse(input['childPresentationId']);
		return childPresentation.success
			? {
					success: true,
					data: {
						childPresentationId: childPresentation.data,
						panelId: input['panelId'],
					},
				}
			: { success: false, code: 'expected-panel-child-terminal' };
	},
};

export const panelSplitInput: ActionInputValidation<PanelSplitInput> = {
	name: 'PanelSplitInput',
	parse: (input) =>
		isRecord(input) && typeof input['panelId'] === 'string' && isDirection(input['direction'])
			? {
					success: true,
					data: {
						direction: input['direction'],
						panelId: input['panelId'],
					},
				}
			: { success: false, code: 'expected-panel-split' },
};

export const panelMoveInput: ActionInputValidation<PanelMoveInput> = {
	name: 'PanelMoveInput',
	parse: (input) =>
		isRecord(input) &&
		typeof input['panelId'] === 'string' &&
		typeof input['targetPanelId'] === 'string' &&
		isDirection(input['direction'])
			? {
					success: true,
					data: {
						direction: input['direction'],
						panelId: input['panelId'],
						targetPanelId: input['targetPanelId'],
					},
				}
			: { success: false, code: 'expected-panel-move' },
};

export const panelResizeInput: ActionInputValidation<PanelResizeInput> = {
	name: 'PanelResizeInput',
	parse: (input) =>
		isRecord(input) &&
		typeof input['panelId'] === 'string' &&
		isResizeAdjustment(input['adjustment']) &&
		(input['multiplier'] === 1 || input['multiplier'] === 2)
			? {
					data: {
						adjustment: input['adjustment'],
						multiplier: input['multiplier'],
						panelId: input['panelId'],
					},
					success: true,
				}
			: { success: false, code: 'expected-panel-resize' },
};

export const panelSwapInput: ActionInputValidation<PanelSwapInput> = {
	name: 'PanelSwapInput',
	parse: (input) =>
		isRecord(input) &&
		typeof input['panelId'] === 'string' &&
		typeof input['targetPanelId'] === 'string'
			? {
					data: {
						panelId: input['panelId'],
						targetPanelId: input['targetPanelId'],
					},
					success: true,
				}
			: { success: false, code: 'expected-panel-swap' },
};
