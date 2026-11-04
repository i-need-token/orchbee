import { describe, expect, it } from 'vitest';

import {
	PANEL_ACTION_IDS,
	panelChildTerminalInput,
	panelCloseInput,
	panelMoveInput,
	panelResizeInput,
	panelSplitInput,
	panelSwapInput,
	panelTargetInput,
	panelVisualSelectionInput,
} from '../panel-action-contracts';

describe('panel action contracts', () => {
	it('owns stable semantic action ids', () => {
		expect(PANEL_ACTION_IDS).toEqual({
			activateChildTerminal: 'workbench.panel.child-terminal.activate',
			close: 'workbench.panel.close',
			equalize: 'workbench.panel.equalize',
			focus: 'workbench.panel.focus',
			move: 'workbench.panel.move',
			resize: 'workbench.panel.resize',
			showPrimaryTerminal: 'workbench.panel.primary-terminal.show',
			split: 'workbench.panel.split',
			startBroadcastInput: 'workbench.panel.broadcast-input.start',
			startResize: 'workbench.panel.resize.start',
			startVisualSelection: 'workbench.panel.visual-selection.start',
			swap: 'workbench.panel.swap',
			toggleMaximization: 'workbench.panel.maximization.toggle',
		});
	});

	it('parses target, child, split, and move inputs at the dynamic boundary', () => {
		expect(panelTargetInput.parse({ panelId: 'panel-1' })).toEqual({
			success: true,
			data: { panelId: 'panel-1' },
		});
		expect(
			panelChildTerminalInput.parse({
				childPresentationId: 'terminal-2',
				panelId: 'panel-1',
			}),
		).toEqual({
			success: true,
			data: { childPresentationId: 'terminal-2', panelId: 'panel-1' },
		});
		expect(panelCloseInput.parse({ panelId: 'panel-1', tabId: 'tab-1' })).toEqual({
			success: true,
			data: { panelId: 'panel-1', tabId: 'tab-1' },
		});
		expect(panelSplitInput.parse({ direction: 'left', panelId: 'panel-1' })).toMatchObject({
			success: true,
		});
		expect(
			panelMoveInput.parse({
				direction: 'right',
				panelId: 'panel-1',
				targetPanelId: 'panel-2',
			}),
		).toMatchObject({ success: true });
		expect(
			panelResizeInput.parse({ adjustment: 'wider', multiplier: 2, panelId: 'panel-1' }),
		).toEqual({
			data: { adjustment: 'wider', multiplier: 2, panelId: 'panel-1' },
			success: true,
		});
		expect(panelSwapInput.parse({ panelId: 'panel-1', targetPanelId: 'panel-2' })).toEqual({
			data: { panelId: 'panel-1', targetPanelId: 'panel-2' },
			success: true,
		});
		expect(panelVisualSelectionInput.parse({ mode: 'swap', panelId: 'panel-1' })).toEqual({
			data: { mode: 'swap', panelId: 'panel-1' },
			success: true,
		});
	});

	it('rejects stale shapes and unsupported directions', () => {
		expect(panelTargetInput.parse(null)).toMatchObject({ success: false });
		expect(panelCloseInput.parse({ panelId: 'panel-1' })).toMatchObject({ success: false });
		expect(
			panelResizeInput.parse({ adjustment: 'wider', multiplier: 3, panelId: 'panel-1' }),
		).toMatchObject({ success: false });
		expect(panelChildTerminalInput.parse({ panelId: 'panel-1' })).toMatchObject({
			success: false,
		});
		expect(panelSplitInput.parse({ direction: 'center', panelId: 'panel-1' })).toMatchObject({
			success: false,
		});
		expect(
			panelMoveInput.parse({
				direction: 'right',
				panelId: 'panel-1',
			}),
		).toMatchObject({ success: false });
		expect(panelSwapInput.parse({ panelId: 'panel-1' })).toMatchObject({ success: false });
		expect(panelVisualSelectionInput.parse({ mode: 'move', panelId: 'panel-1' })).toMatchObject({
			success: false,
		});
	});
});
