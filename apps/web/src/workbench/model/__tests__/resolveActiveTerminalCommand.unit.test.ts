import { TAB_STRIP_ACTION_IDS } from '@orchbee/tab-strip';
import { describe, expect, it } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

import { PANEL_ACTION_IDS } from '../../components/workbench-panel-workspace/actions/panel-action-contracts';
import {
	transitionPanelWorkspace,
	type PanelWorkspaceState,
} from '../../components/workbench-panel-workspace/model/panel-workspace-state';
import { resolveActiveTerminalCommand } from '../resolveActiveTerminalCommand';
import {
	activeTerminalContext,
	createSnapshotRuntime,
	fixturePanelId,
	fixturePresentationId,
	fixtureTabId,
	oneWorkspaceSnapshot,
	snapshotPanel,
	snapshotPresentation,
	snapshotSplit,
	snapshotTab,
	type FixtureRecord,
} from '../../../__tests__/server-workbench-snapshot';

const TAB_ID = fixtureTabId(1);
const PANEL_1 = fixturePanelId(1);
const PANEL_2 = fixturePanelId(2);
const PANEL_3 = fixturePanelId(3);
const CHILD_ID = terminalPresentationId(fixturePresentationId(2));

describe('resolveActiveTerminalCommand', () => {
	it('routes one primary terminal through panel close or Tab close by panel count', () => {
		const single = initialState();
		expect(resolve(single, 'close')).toMatchObject({
			actionId: TAB_STRIP_ACTION_IDS.closeTabs,
			input: { closeSource: 'non-ui', tabIds: [TAB_ID] },
			kind: 'ready',
			panelId: PANEL_1,
		});

		const split = twoPanels();
		expect(resolve(split, 'close')).toMatchObject({
			actionId: PANEL_ACTION_IDS.close,
			input: {
				childPresentationId: null,
				panelId: PANEL_2,
				tabId: TAB_ID,
			},
			kind: 'ready',
		});
	});

	it('closes an active child terminal before its singleton panel or Tab', () => {
		const activated = transitionPanelWorkspace(
			panelState(snapshotPanel(1, snapshotPresentation(1), [snapshotPresentation(2)])),
			{
				childPresentationId: CHILD_ID,
				panelId: PANEL_1,
				tabId: TAB_ID,
				type: 'child-terminal-activated',
			},
		);

		expect(resolve(activated.state, 'close')).toMatchObject({
			actionId: PANEL_ACTION_IDS.close,
			input: {
				childPresentationId: CHILD_ID,
				panelId: PANEL_1,
				tabId: TAB_ID,
			},
			kind: 'ready',
		});
	});

	it('gives an explicit menu target precedence over the focused panel', () => {
		const state = twoPanels();
		expect(
			resolveActiveTerminalCommand({
				activeTabId: TAB_ID,
				command: 'split-below',
				state,
				targetPanelId: PANEL_1,
			}),
		).toEqual({
			actionId: PANEL_ACTION_IDS.split,
			input: { direction: 'below', panelId: PANEL_1 },
			kind: 'ready',
			panelId: PANEL_1,
		});
	});

	it('resolves directional split and guarded maximization operations', () => {
		const single = initialState();
		expect(resolve(single, 'split-right')).toMatchObject({
			actionId: PANEL_ACTION_IDS.split,
			input: { direction: 'right', panelId: PANEL_1 },
		});
		expect(resolve(single, 'toggle-maximization')).toEqual({
			kind: 'unavailable',
			reason: 'requires-multiple-panels',
		});

		const split = twoPanels();
		expect(resolve(split, 'toggle-maximization')).toEqual({
			actionId: PANEL_ACTION_IDS.toggleMaximization,
			input: { panelId: PANEL_2 },
			kind: 'ready',
			panelId: PANEL_2,
		});
	});

	it('opens resize mode for the exact active or explicit panel target', () => {
		expect(resolve(initialState(), 'start-resize')).toEqual({
			actionId: PANEL_ACTION_IDS.startResize,
			input: { panelId: PANEL_1 },
			kind: 'ready',
			panelId: PANEL_1,
		});
		const split = twoPanels();
		expect(
			resolveActiveTerminalCommand({
				activeTabId: TAB_ID,
				command: 'start-resize',
				state: split,
				targetPanelId: PANEL_1,
			}),
		).toMatchObject({ input: { panelId: PANEL_1 }, panelId: PANEL_1 });
	});

	it('resolves whole-Tab equalization through one exact live Panel target', () => {
		const single = initialState();
		expect(resolve(single, 'equalize')).toEqual({
			kind: 'unavailable',
			reason: 'requires-multiple-panels',
		});

		const split = twoPanels();
		expect(resolve(split, 'equalize')).toEqual({
			actionId: PANEL_ACTION_IDS.equalize,
			input: { panelId: PANEL_2 },
			kind: 'ready',
			panelId: PANEL_2,
		});
		expect(
			resolveActiveTerminalCommand({
				activeTabId: TAB_ID,
				command: 'equalize',
				state: split,
				targetPanelId: PANEL_1,
			}),
		).toMatchObject({ input: { panelId: PANEL_1 }, kind: 'ready', panelId: PANEL_1 });

		const maximized = transitionPanelWorkspace(split, {
			panelId: PANEL_2,
			tabId: TAB_ID,
			type: 'panel-maximization-toggled',
		}).state;
		expect(resolve(maximized, 'equalize')).toEqual({
			kind: 'unavailable',
			reason: 'panel-maximized',
		});
	});

	it('resolves directional focus to the neighboring Panel and rejects outer edges', () => {
		const split = twoPanels();
		expect(resolve(split, 'focus-left')).toEqual({
			actionId: PANEL_ACTION_IDS.focus,
			input: { panelId: PANEL_1 },
			kind: 'ready',
			panelId: PANEL_1,
		});
		expect(resolve(split, 'focus-right')).toEqual({
			kind: 'unavailable',
			reason: 'directional-neighbor-unavailable',
		});
	});

	it('keeps directional focus unavailable while a Panel is maximized', () => {
		const split = twoPanels();
		const maximized = transitionPanelWorkspace(split, {
			panelId: PANEL_2,
			tabId: TAB_ID,
			type: 'panel-maximization-toggled',
		}).state;

		expect(resolve(maximized, 'focus-left')).toEqual({
			kind: 'unavailable',
			reason: 'panel-maximized',
		});
	});

	it('cycles focus in canonical Panel order and wraps both boundaries', () => {
		const split = twoPanels();

		expect(resolve(split, 'focus-next')).toEqual({
			actionId: PANEL_ACTION_IDS.focus,
			input: { panelId: PANEL_1 },
			kind: 'ready',
			panelId: PANEL_1,
		});
		expect(resolve(split, 'focus-previous')).toEqual({
			actionId: PANEL_ACTION_IDS.focus,
			input: { panelId: PANEL_1 },
			kind: 'ready',
			panelId: PANEL_1,
		});

		const refocused = transitionPanelWorkspace(split, {
			panelId: PANEL_1,
			tabId: TAB_ID,
			type: 'panel-focused',
		}).state;
		expect(resolve(refocused, 'focus-next')).toMatchObject({ panelId: PANEL_2 });
		expect(resolve(refocused, 'focus-previous')).toMatchObject({ panelId: PANEL_2 });
	});

	it('keeps cyclic focus unavailable for singleton and maximized layouts', () => {
		expect(resolve(initialState(), 'focus-next')).toEqual({
			kind: 'unavailable',
			reason: 'requires-multiple-panels',
		});

		const split = twoPanels();
		const maximized = transitionPanelWorkspace(split, {
			panelId: PANEL_2,
			tabId: TAB_ID,
			type: 'panel-maximization-toggled',
		}).state;
		expect(resolve(maximized, 'focus-previous')).toEqual({
			kind: 'unavailable',
			reason: 'panel-maximized',
		});
	});

	it('focuses or swaps the sole other Panel directly without starting a visual session', () => {
		const split = twoPanels();

		expect(resolve(split, 'focus-visible')).toEqual({
			actionId: PANEL_ACTION_IDS.focus,
			input: { panelId: PANEL_1 },
			kind: 'ready',
			panelId: PANEL_1,
		});
		expect(resolve(split, 'swap-visible')).toEqual({
			actionId: PANEL_ACTION_IDS.swap,
			input: { panelId: PANEL_2, targetPanelId: PANEL_1 },
			kind: 'ready',
			panelId: PANEL_2,
		});
	});

	it('starts a labeled visual session for three or more Panels', () => {
		const layout = threePanels();

		expect(resolve(layout, 'focus-visible')).toEqual({
			actionId: PANEL_ACTION_IDS.startVisualSelection,
			input: { mode: 'focus', panelId: PANEL_3 },
			kind: 'ready',
			panelId: PANEL_3,
		});
		expect(resolve(layout, 'swap-visible')).toEqual({
			actionId: PANEL_ACTION_IDS.startVisualSelection,
			input: { mode: 'swap', panelId: PANEL_3 },
			kind: 'ready',
			panelId: PANEL_3,
		});
	});

	it('keeps visual selection unavailable for singleton and maximized layouts', () => {
		expect(resolve(initialState(), 'focus-visible')).toEqual({
			kind: 'unavailable',
			reason: 'requires-multiple-panels',
		});

		const split = twoPanels();
		const maximized = transitionPanelWorkspace(split, {
			panelId: PANEL_2,
			tabId: TAB_ID,
			type: 'panel-maximization-toggled',
		}).state;
		expect(resolve(maximized, 'swap-visible')).toEqual({
			kind: 'unavailable',
			reason: 'panel-maximized',
		});
	});

	it('resolves directional movement to an exact source and neighboring Panel swap', () => {
		const split = twoPanels();

		expect(resolve(split, 'move-left')).toEqual({
			actionId: PANEL_ACTION_IDS.swap,
			input: { panelId: PANEL_2, targetPanelId: PANEL_1 },
			kind: 'ready',
			panelId: PANEL_2,
		});
		expect(resolve(split, 'move-right')).toEqual({
			kind: 'unavailable',
			reason: 'directional-neighbor-unavailable',
		});
	});

	it('keeps directional movement unavailable while a Panel is maximized', () => {
		const split = twoPanels();
		const maximized = transitionPanelWorkspace(split, {
			panelId: PANEL_2,
			tabId: TAB_ID,
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
				activeTabId: TAB_ID,
				command: 'close',
				state,
				targetPanelId: 'missing-panel',
			}),
		).toEqual({ kind: 'unavailable', reason: 'panel-unavailable' });
	});
});

function panelState(root: FixtureRecord, focusedPanelId?: string): PanelWorkspaceState {
	const state = activeTerminalContext(
		createSnapshotRuntime(oneWorkspaceSnapshot(1, [snapshotTab(1, root)])),
	).panelWorkspaceStore.getSnapshot();
	return focusedPanelId === undefined
		? state
		: transitionPanelWorkspace(state, {
				panelId: focusedPanelId,
				tabId: TAB_ID,
				type: 'panel-focused',
			}).state;
}

function initialState(): PanelWorkspaceState {
	return panelState(snapshotPanel(1));
}

/** Panel 1 left of Panel 2, which has focus. */
function twoPanels(): PanelWorkspaceState {
	return panelState(snapshotSplit(1, 'horizontal', snapshotPanel(1), snapshotPanel(2)), PANEL_2);
}

/** Panels 1, 2, and 3 from left to right; Panel 3 has focus. */
function threePanels(): PanelWorkspaceState {
	return panelState(
		snapshotSplit(
			1,
			'horizontal',
			snapshotPanel(1),
			snapshotSplit(2, 'horizontal', snapshotPanel(2), snapshotPanel(3)),
		),
		PANEL_3,
	);
}

function resolve(
	state: PanelWorkspaceState,
	command: Parameters<typeof resolveActiveTerminalCommand>[0]['command'],
) {
	return resolveActiveTerminalCommand({
		activeTabId: TAB_ID,
		command,
		state,
		targetPanelId: null,
	});
}
