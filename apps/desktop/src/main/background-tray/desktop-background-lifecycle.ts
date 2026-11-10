/**
 * Whether the Windows Desktop keeps running in the notification area after its window closes.
 *
 * Closing the window while terminals run hides it instead, and the notification-area icon shows
 * until the window shows again or the last terminal ends, which quits Desktop. Nothing intercepts a
 * close once the application is quitting, a development rebuild restarts it, or Windows ends the
 * person's session, so Desktop never holds up a quit, a restart, logoff, or shutdown.
 *
 * Invariant, while the application is not ending: the icon shows exactly when the window is hidden
 * and terminals run, so a hidden window always has an icon to reopen it from.
 */
import type { DesktopBackgroundStatusSummary } from '@orchbee/desktop-client-protocol';

/** The window as Main last saw it; `absent` before the first one shows and after it closes. */
type DesktopBackgroundWindow = 'absent' | 'hidden' | 'shown';

/**
 * `quitting` covers every quit, including the development runtime's restart after a rebuild;
 * `session-ending` is Windows logging off or shutting down. Neither ever returns to `none`.
 */
type DesktopBackgroundEnding = 'none' | 'quitting' | 'session-ending';

export interface DesktopBackgroundState {
	readonly ending: DesktopBackgroundEnding;
	/** Whether the one-time notice that Orchbee keeps running has been shown, in this or an earlier run. */
	readonly noticeShown: boolean;
	/** The page's latest summary; all zero until it reports one. */
	readonly summary: DesktopBackgroundStatusSummary;
	readonly window: DesktopBackgroundWindow;
}

export type DesktopBackgroundEvent =
	/** The person closed the window: its close button, the taskbar, or `Alt+F4`. */
	| { readonly kind: 'close-requested' }
	| { readonly kind: 'notice-shown' }
	| { readonly kind: 'quit-requested' }
	/** Windows asked whether its session may end, or said it is ending. */
	| { readonly kind: 'session-ending' }
	| { readonly kind: 'summary-changed'; readonly summary: DesktopBackgroundStatusSummary }
	| { readonly kind: 'window-closed' }
	/** The window showed, however it was asked to: the icon, another start, a notification. */
	| { readonly kind: 'window-shown' };

type DesktopBackgroundCommand =
	/** Cancel the close in progress and hide the window. */
	| { readonly kind: 'hide-window' }
	| { readonly kind: 'quit' }
	/** Persist that the notice has been shown, so later runs do not show it again. */
	| { readonly kind: 'record-notice' }
	| { readonly kind: 'show-notice' };

export interface DesktopBackgroundTransition {
	readonly commands: readonly DesktopBackgroundCommand[];
	readonly state: DesktopBackgroundState;
}

/** The Agent state the icon marks, most urgent first; `none` marks nothing. */
export type DesktopTrayStatus = 'attention' | 'done' | 'failed' | 'none';

export interface DesktopTrayView {
	readonly status: DesktopTrayStatus;
	readonly summary: DesktopBackgroundStatusSummary;
}

const EMPTY_SUMMARY: DesktopBackgroundStatusSummary = {
	attention: 0,
	doneUnseen: 0,
	failed: 0,
	runningTerminals: 0,
	workingAgents: 0,
};

export function createDesktopBackgroundState(noticeShown: boolean): DesktopBackgroundState {
	return { ending: 'none', noticeShown, summary: EMPTY_SUMMARY, window: 'absent' };
}

export function transitionDesktopBackground(
	state: DesktopBackgroundState,
	event: DesktopBackgroundEvent,
): DesktopBackgroundTransition {
	switch (event.kind) {
		case 'close-requested': {
			// A close while quitting or ending the session always proceeds, and so does one with
			// nothing left running, which quits as a close always has.
			if (
				state.ending !== 'none' ||
				state.window !== 'shown' ||
				state.summary.runningTerminals === 0
			) {
				return unchanged(state);
			}
			return {
				commands: state.noticeShown
					? [{ kind: 'hide-window' }]
					: [{ kind: 'hide-window' }, { kind: 'show-notice' }],
				state: { ...state, window: 'hidden' },
			};
		}
		case 'notice-shown':
			return state.noticeShown
				? unchanged(state)
				: { commands: [{ kind: 'record-notice' }], state: { ...state, noticeShown: true } };
		case 'quit-requested':
			return state.ending === 'none'
				? { commands: [], state: { ...state, ending: 'quitting' } }
				: unchanged(state);
		case 'session-ending':
			return { commands: [], state: { ...state, ending: 'session-ending' } };
		case 'summary-changed': {
			const next = { ...state, summary: event.summary };
			// The last terminal ended while only the icon was left: nothing remains to reopen.
			if (state.ending === 'none' && state.window === 'hidden' && !hasRunningTerminals(next)) {
				return { commands: [{ kind: 'quit' }], state: { ...next, ending: 'quitting' } };
			}
			return { commands: [], state: next };
		}
		case 'window-closed':
			return { commands: [], state: { ...state, window: 'absent' } };
		case 'window-shown':
			return { commands: [], state: { ...state, window: 'shown' } };
	}
}

/** What the notification-area icon shows, or null while it should not exist. */
export function projectDesktopTray(state: DesktopBackgroundState): DesktopTrayView | null {
	if (state.ending !== 'none' || state.window !== 'hidden' || !hasRunningTerminals(state)) {
		return null;
	}
	const { summary } = state;
	let status: DesktopTrayStatus = 'none';
	if (summary.failed > 0) status = 'failed';
	else if (summary.attention > 0) status = 'attention';
	else if (summary.doneUnseen > 0) status = 'done';
	return { status, summary };
}

function hasRunningTerminals(state: DesktopBackgroundState): boolean {
	return state.summary.runningTerminals > 0;
}

function unchanged(state: DesktopBackgroundState): DesktopBackgroundTransition {
	return { commands: [], state };
}
