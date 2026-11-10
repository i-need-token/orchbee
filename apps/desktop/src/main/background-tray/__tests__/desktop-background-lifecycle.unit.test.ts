import { describe, expect, it } from 'vitest';

import type { DesktopBackgroundStatusSummary } from '@orchbee/desktop-client-protocol';

import {
	createDesktopBackgroundState,
	projectDesktopTray,
	transitionDesktopBackground,
	type DesktopBackgroundEvent,
	type DesktopBackgroundState,
} from '../desktop-background-lifecycle';

function summary(
	runningTerminals: number,
	agents: Partial<DesktopBackgroundStatusSummary> = {},
): DesktopBackgroundStatusSummary {
	return { attention: 0, doneUnseen: 0, failed: 0, workingAgents: 0, ...agents, runningTerminals };
}

function run(
	state: DesktopBackgroundState,
	events: readonly DesktopBackgroundEvent[],
): DesktopBackgroundState {
	return events.reduce(
		(current, event) => transitionDesktopBackground(current, event).state,
		state,
	);
}

/** A shown window whose page reports `terminals` running terminals. */
function shown(terminals: number): DesktopBackgroundState {
	return run(createDesktopBackgroundState(true), [
		{ kind: 'window-shown' },
		{ kind: 'summary-changed', summary: summary(terminals) },
	]);
}

const close: DesktopBackgroundEvent = { kind: 'close-requested' };

describe('Desktop background lifecycle', () => {
	it('hides the window on close while terminals run, and lets the close proceed without them', () => {
		const hidden = transitionDesktopBackground(shown(2), close);
		expect(hidden.commands).toContainEqual({ kind: 'hide-window' });
		expect(hidden.state.window).toBe('hidden');
		expect(projectDesktopTray(hidden.state)).not.toBeNull();

		const closed = transitionDesktopBackground(shown(0), close);
		expect(closed.commands).toEqual([]);
		expect(closed.state.window).toBe('shown');
		expect(projectDesktopTray(closed.state)).toBeNull();
	});

	it('never intercepts a close once quitting, restarting for development, or ending the session', () => {
		// The development runtime restarts Main through an ordinary quit, so it is a quit request.
		for (const ending of [{ kind: 'quit-requested' }, { kind: 'session-ending' }] as const) {
			const fromShown = transitionDesktopBackground(run(shown(3), [ending]), close);
			expect(fromShown.commands).toEqual([]);

			const fromHidden = run(shown(3), [close, ending]);
			expect(projectDesktopTray(fromHidden)).toBeNull();
			expect(transitionDesktopBackground(fromHidden, close).commands).toEqual([]);
			// A later summary never turns the ending back into a hide or a second quit.
			const later = run(fromHidden, [{ kind: 'summary-changed', summary: summary(4) }, close]);
			expect(later.ending).not.toBe('none');
			expect(transitionDesktopBackground(later, close).commands).toEqual([]);
		}
	});

	it('quits when the last terminal ends while the window is hidden, and only then', () => {
		const hidden = run(shown(1), [close]);
		const ended = transitionDesktopBackground(hidden, {
			kind: 'summary-changed',
			summary: summary(0),
		});
		expect(ended.commands).toEqual([{ kind: 'quit' }]);
		expect(projectDesktopTray(ended.state)).toBeNull();

		const visible = transitionDesktopBackground(shown(1), {
			kind: 'summary-changed',
			summary: summary(0),
		});
		expect(visible.commands).toEqual([]);
	});

	it('shows the icon exactly while the window is hidden with terminals running', () => {
		const events: readonly DesktopBackgroundEvent[] = [
			close,
			{ kind: 'notice-shown' },
			{ kind: 'quit-requested' },
			{ kind: 'session-ending' },
			{ kind: 'summary-changed', summary: summary(0) },
			{ kind: 'summary-changed', summary: summary(1, { attention: 1 }) },
			{ kind: 'summary-changed', summary: summary(2, { failed: 1, doneUnseen: 1 }) },
			{ kind: 'window-closed' },
			{ kind: 'window-shown' },
		];
		const seen = new Set<string>();
		const violations: DesktopBackgroundState[] = [];
		const pending = [createDesktopBackgroundState(false), createDesktopBackgroundState(true)];
		for (let state = pending.pop(); state !== undefined; state = pending.pop()) {
			const key = JSON.stringify(state);
			if (seen.has(key)) continue;
			seen.add(key);
			const tray = projectDesktopTray(state) !== null;
			const hiddenWithTerminals = state.window === 'hidden' && state.summary.runningTerminals > 0;
			// No icon while the window shows, and no hidden window without an icon to reopen it from.
			const holds = state.ending === 'none' ? tray === hiddenWithTerminals : !tray;
			if (!holds || (state.ending === 'none' && state.window === 'hidden' && !tray)) {
				violations.push(state);
			}
			for (const event of events) pending.push(transitionDesktopBackground(state, event).state);
		}
		expect(violations).toEqual([]);
		expect(seen.size).toBeGreaterThan(20);
	});
});
