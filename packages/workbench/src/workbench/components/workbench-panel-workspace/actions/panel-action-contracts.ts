import type { ActionInputValidation, ActionSignature } from '@orchbee/actions';

import type { PanelSplitDirection } from '../model/panel-workspace-state';

export const PANEL_ACTION_IDS = {
	activateChildTerminal: 'workbench.panel.child-terminal.activate',
	close: 'workbench.panel.close',
	createChildTerminal: 'workbench.panel.child-terminal.create',
	focus: 'workbench.panel.focus',
	move: 'workbench.panel.move',
	showPrimaryTerminal: 'workbench.panel.primary-terminal.show',
	split: 'workbench.panel.split',
	toggleMaximization: 'workbench.panel.maximization.toggle',
} as const;

export interface PanelTargetInput {
	readonly panelId: string;
}

export interface PanelCloseInput extends PanelTargetInput {
	readonly childTerminalId: string | null;
	readonly tabId: string;
}

export interface PanelChildTerminalInput extends PanelTargetInput {
	readonly childTerminalId: string;
}

export interface PanelSplitInput extends PanelTargetInput {
	readonly direction: PanelSplitDirection;
}

export interface PanelMoveInput extends PanelTargetInput {
	readonly direction: PanelSplitDirection;
	readonly targetPanelId: string;
}

export interface PanelActionCatalog {
	readonly [PANEL_ACTION_IDS.activateChildTerminal]: ActionSignature<PanelChildTerminalInput>;
	readonly [PANEL_ACTION_IDS.close]: ActionSignature<PanelCloseInput>;
	readonly [PANEL_ACTION_IDS.createChildTerminal]: ActionSignature<PanelTargetInput>;
	readonly [PANEL_ACTION_IDS.focus]: ActionSignature<PanelTargetInput>;
	readonly [PANEL_ACTION_IDS.move]: ActionSignature<PanelMoveInput>;
	readonly [PANEL_ACTION_IDS.showPrimaryTerminal]: ActionSignature<PanelTargetInput>;
	readonly [PANEL_ACTION_IDS.split]: ActionSignature<PanelSplitInput, string>;
	readonly [PANEL_ACTION_IDS.toggleMaximization]: ActionSignature<PanelTargetInput>;
}

function isRecord(input: unknown): input is Readonly<Record<string, unknown>> {
	return typeof input === 'object' && input !== null && !Array.isArray(input);
}

function isDirection(input: unknown): input is PanelSplitDirection {
	return input === 'above' || input === 'below' || input === 'left' || input === 'right';
}

export const panelTargetInput: ActionInputValidation<PanelTargetInput> = {
	name: 'PanelTargetInput',
	parse: (input) =>
		isRecord(input) && typeof input['panelId'] === 'string'
			? { success: true, data: { panelId: input['panelId'] } }
			: { success: false, code: 'expected-panel-target' },
};

export const panelCloseInput: ActionInputValidation<PanelCloseInput> = {
	name: 'PanelCloseInput',
	parse: (input) =>
		isRecord(input) &&
		typeof input['panelId'] === 'string' &&
		typeof input['tabId'] === 'string' &&
		(input['childTerminalId'] === null || typeof input['childTerminalId'] === 'string')
			? {
					success: true,
					data: {
						childTerminalId: input['childTerminalId'],
						panelId: input['panelId'],
						tabId: input['tabId'],
					},
				}
			: { success: false, code: 'expected-panel-close-target' },
};

export const panelChildTerminalInput: ActionInputValidation<PanelChildTerminalInput> = {
	name: 'PanelChildTerminalInput',
	parse: (input) =>
		isRecord(input) &&
		typeof input['panelId'] === 'string' &&
		typeof input['childTerminalId'] === 'string'
			? {
					success: true,
					data: {
						childTerminalId: input['childTerminalId'],
						panelId: input['panelId'],
					},
				}
			: { success: false, code: 'expected-panel-child-terminal' },
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
