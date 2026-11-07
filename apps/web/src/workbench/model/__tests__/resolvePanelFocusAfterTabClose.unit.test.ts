import { describe, expect, it } from 'vitest';

import { resolvePanelFocusAfterTabClose } from '../resolvePanelFocusAfterTabClose';

const panel = (panelId: string, tabId: string, workspaceId = 'w1') => ({
	panelId,
	tabId,
	workspaceId,
});
const everyOpen = () => true;

describe('resolvePanelFocusAfterTabClose', () => {
	it('never moves focus into another Workspace, even when that Panel is the most recent one', () => {
		const focused = panel('B', 't2');
		const history = [focused, panel('X', 't9', 'w2'), panel('A', 't1')];

		expect(
			resolvePanelFocusAfterTabClose({
				closedTabIds: new Set(['t2']),
				focused,
				history,
				isOpen: everyOpen,
			}),
		).toEqual(panel('A', 't1'));
	});

	it('returns the most recent open Panel outside the closed Tab when the closed Tab holds the focused Panel', () => {
		const focused = panel('B', 't2');
		const history = [focused, panel('B2', 't2'), panel('A', 't1'), panel('C', 't3')];

		expect(
			resolvePanelFocusAfterTabClose({
				closedTabIds: new Set(['t2']),
				focused,
				history,
				isOpen: (entry) => entry.panelId !== 'A',
			}),
		).toEqual(panel('C', 't3'));
		expect(
			resolvePanelFocusAfterTabClose({
				closedTabIds: new Set(['t2']),
				focused,
				history,
				isOpen: everyOpen,
			}),
		).toEqual(panel('A', 't1'));
	});

	it('moves nothing when the closed Tabs do not hold the focused Panel', () => {
		expect(
			resolvePanelFocusAfterTabClose({
				closedTabIds: new Set(['t2']),
				focused: panel('A', 't1'),
				history: [panel('A', 't1'), panel('B', 't2')],
				isOpen: everyOpen,
			}),
		).toBeNull();
	});

	it('returns null when every other Panel is inside the closed Tabs, leaving the post-close fallback', () => {
		const focused = panel('B', 't2');

		expect(
			resolvePanelFocusAfterTabClose({
				closedTabIds: new Set(['t1', 't2']),
				focused,
				history: [focused, panel('A', 't1')],
				isOpen: everyOpen,
			}),
		).toBeNull();
		expect(
			resolvePanelFocusAfterTabClose({
				closedTabIds: new Set(['t2']),
				focused,
				history: [focused],
				isOpen: everyOpen,
			}),
		).toBeNull();
	});
});
