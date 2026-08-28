import { TAB_STRIP_ACTION_IDS } from '@orchbee/tab-strip';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it } from 'vitest';

import { PANEL_ACTION_IDS } from '../../components/workbench-panel-workspace/actions/panel-action-contracts';
import {
	initializePanelWorkspace,
	transitionPanelWorkspace,
	type PanelWorkspaceState,
} from '../../components/workbench-panel-workspace/model/panel-workspace-state';
import { resolveActiveTerminalCommand } from '../resolveActiveTerminalCommand';

describe('resolveActiveTerminalCommand', () => {
	it('routes one primary terminal through panel close or Tab close by panel count', () => {
		const single = initialState();
		expect(resolve(single, 'close')).toMatchObject({
			actionId: TAB_STRIP_ACTION_IDS.closeTabs,
			input: { closeSource: 'non-ui', tabIds: ['tab-1'] },
			kind: 'ready',
			panelId: 'panel-1',
		});

		const split = splitRight(single);
		expect(resolve(split, 'close')).toMatchObject({
			actionId: PANEL_ACTION_IDS.close,
			input: {
				childPresentationId: null,
				panelId: 'panel-2',
				tabId: 'tab-1',
			},
			kind: 'ready',
		});
	});

	it('closes an active child terminal before its singleton panel or Tab', () => {
		const initial = initialState();
		const created = transitionPanelWorkspace(initial, {
			newTerminalTitle: 'Child',
			panelId: 'panel-1',
			presentation: {
				kind: 'preview',
				presentationId: terminalPresentationId('terminal-child'),
			},
			tabId: 'tab-1',
			type: 'child-terminal-created',
		});
		const activated = transitionPanelWorkspace(created.state, {
			childPresentationId: terminalPresentationId('terminal-child'),
			panelId: 'panel-1',
			tabId: 'tab-1',
			type: 'child-terminal-activated',
		});

		expect(resolve(activated.state, 'close')).toMatchObject({
			actionId: PANEL_ACTION_IDS.close,
			input: {
				childPresentationId: terminalPresentationId('terminal-child'),
				panelId: 'panel-1',
				tabId: 'tab-1',
			},
			kind: 'ready',
		});
	});

	it('gives an explicit menu target precedence over the focused panel', () => {
		const state = splitRight(initialState());
		expect(
			resolveActiveTerminalCommand({
				activeTabId: 'tab-1',
				command: 'split-below',
				state,
				targetPanelId: 'panel-1',
			}),
		).toEqual({
			actionId: PANEL_ACTION_IDS.split,
			input: { direction: 'below', panelId: 'panel-1' },
			kind: 'ready',
			panelId: 'panel-1',
		});
	});

	it('resolves directional split and guarded maximization operations', () => {
		const single = initialState();
		expect(resolve(single, 'split-right')).toMatchObject({
			actionId: PANEL_ACTION_IDS.split,
			input: { direction: 'right', panelId: 'panel-1' },
		});
		expect(resolve(single, 'toggle-maximization')).toEqual({
			kind: 'unavailable',
			reason: 'requires-multiple-panels',
		});

		const split = splitRight(single);
		expect(resolve(split, 'toggle-maximization')).toEqual({
			actionId: PANEL_ACTION_IDS.toggleMaximization,
			input: { panelId: 'panel-2' },
			kind: 'ready',
			panelId: 'panel-2',
		});
	});

	it('opens resize mode for the exact active or explicit panel target', () => {
		expect(resolve(initialState(), 'start-resize')).toEqual({
			actionId: PANEL_ACTION_IDS.startResize,
			input: { panelId: 'panel-1' },
			kind: 'ready',
			panelId: 'panel-1',
		});
		const split = splitRight(initialState());
		expect(
			resolveActiveTerminalCommand({
				activeTabId: 'tab-1',
				command: 'start-resize',
				state: split,
				targetPanelId: 'panel-1',
			}),
		).toMatchObject({ input: { panelId: 'panel-1' }, panelId: 'panel-1' });
	});

	it('resolves whole-Tab equalization through one exact live Panel target', () => {
		const single = initialState();
		expect(resolve(single, 'equalize')).toEqual({
			kind: 'unavailable',
			reason: 'requires-multiple-panels',
		});

		const split = splitRight(single);
		expect(resolve(split, 'equalize')).toEqual({
			actionId: PANEL_ACTION_IDS.equalize,
			input: { panelId: 'panel-2' },
			kind: 'ready',
			panelId: 'panel-2',
		});
		expect(
			resolveActiveTerminalCommand({
				activeTabId: 'tab-1',
				command: 'equalize',
				state: split,
				targetPanelId: 'panel-1',
			}),
		).toMatchObject({ input: { panelId: 'panel-1' }, kind: 'ready', panelId: 'panel-1' });

		const maximized = transitionPanelWorkspace(split, {
			panelId: 'panel-2',
			tabId: 'tab-1',
			type: 'panel-maximization-toggled',
		}).state;
		expect(resolve(maximized, 'equalize')).toEqual({
			kind: 'unavailable',
			reason: 'panel-maximized',
		});
	});

	it('resolves directional focus to the neighboring Panel and rejects outer edges', () => {
		const split = splitRight(initialState());
		expect(resolve(split, 'focus-left')).toEqual({
			actionId: PANEL_ACTION_IDS.focus,
			input: { panelId: 'panel-1' },
			kind: 'ready',
			panelId: 'panel-1',
		});
		expect(resolve(split, 'focus-right')).toEqual({
			kind: 'unavailable',
			reason: 'directional-neighbor-unavailable',
		});
	});

	it('keeps directional focus unavailable while a Panel is maximized', () => {
		const split = splitRight(initialState());
		const maximized = transitionPanelWorkspace(split, {
			panelId: 'panel-2',
			tabId: 'tab-1',
			type: 'panel-maximization-toggled',
		}).state;

		expect(resolve(maximized, 'focus-left')).toEqual({
			kind: 'unavailable',
			reason: 'panel-maximized',
		});
	});

	it('resolves directional movement to an exact source and neighboring Panel swap', () => {
		const split = splitRight(initialState());

		expect(resolve(split, 'move-left')).toEqual({
			actionId: PANEL_ACTION_IDS.swap,
			input: { panelId: 'panel-2', targetPanelId: 'panel-1' },
			kind: 'ready',
			panelId: 'panel-2',
		});
		expect(resolve(split, 'move-right')).toEqual({
			kind: 'unavailable',
			reason: 'directional-neighbor-unavailable',
		});
	});

	it('keeps directional movement unavailable while a Panel is maximized', () => {
		const split = splitRight(initialState());
		const maximized = transitionPanelWorkspace(split, {
			panelId: 'panel-2',
			tabId: 'tab-1',
			type: 'panel-maximization-toggled',
		}).state;

		expect(resolve(maximized, 'move-left')).toEqual({
			kind: 'unavailable',
			reason: 'panel-maximized',
		});
	});

	it('rejects absent, stale, and cross-Tab targets without preparing an operation', () => {
		const state = initialState();
		expect(
			resolveActiveTerminalCommand({
				activeTabId: null,
				command: 'close',
				state,
				targetPanelId: null,
			}),
		).toEqual({ kind: 'unavailable', reason: 'active-tab-unavailable' });
		expect(
			resolveActiveTerminalCommand({
				activeTabId: 'missing-tab',
				command: 'close',
				state,
				targetPanelId: null,
			}),
		).toEqual({ kind: 'unavailable', reason: 'active-tab-unavailable' });
		expect(
			resolveActiveTerminalCommand({
				activeTabId: 'tab-1',
				command: 'close',
				state,
				targetPanelId: 'missing-panel',
			}),
		).toEqual({ kind: 'unavailable', reason: 'panel-unavailable' });
	});
});

function initialState(): PanelWorkspaceState {
	return initializePanelWorkspace([
		{
			id: 'tab-1',
			presentation: {
				kind: 'preview',
				presentationId: terminalPresentationId('terminal-1'),
			},
			title: 'Terminal',
		},
	]).state;
}

function resolve(
	state: PanelWorkspaceState,
	command: Parameters<typeof resolveActiveTerminalCommand>[0]['command'],
) {
	return resolveActiveTerminalCommand({
		activeTabId: 'tab-1',
		command,
		state,
		targetPanelId: null,
	});
}

function splitRight(state: PanelWorkspaceState): PanelWorkspaceState {
	return transitionPanelWorkspace(state, {
		direction: 'right',
		newTerminalTitle: 'Terminal 2',
		panelId: 'panel-1',
		presentation: {
			kind: 'preview',
			presentationId: terminalPresentationId('terminal-2'),
		},
		tabId: 'tab-1',
		type: 'panel-split',
	}).state;
}
