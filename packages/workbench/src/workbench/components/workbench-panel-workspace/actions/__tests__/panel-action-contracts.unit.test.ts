import { describe, expect, it } from 'vitest';

import {
	PANEL_ACTION_IDS,
	panelChildTerminalInput,
	panelCloseInput,
	panelMoveInput,
	panelSplitInput,
	panelTargetInput,
} from '../panel-action-contracts';

describe('panel action contracts', () => {
	it('owns stable semantic action ids', () => {
		expect(PANEL_ACTION_IDS).toEqual({
			activateChildTerminal: 'workbench.panel.child-terminal.activate',
			close: 'workbench.panel.close',
			createChildTerminal: 'workbench.panel.child-terminal.create',
			focus: 'workbench.panel.focus',
			move: 'workbench.panel.move',
			showPrimaryTerminal: 'workbench.panel.primary-terminal.show',
			split: 'workbench.panel.split',
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
		expect(
			panelCloseInput.parse({
				childPresentationId: 'terminal-2',
				panelId: 'panel-1',
				tabId: 'tab-1',
			}),
		).toEqual({
			success: true,
			data: {
				childPresentationId: 'terminal-2',
				panelId: 'panel-1',
				tabId: 'tab-1',
			},
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
	});

	it('rejects stale shapes and unsupported directions', () => {
		expect(panelTargetInput.parse(null)).toMatchObject({ success: false });
		expect(
			panelCloseInput.parse({
				panelId: 'panel-1',
				tabId: 'tab-1',
			}),
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
	});
});
