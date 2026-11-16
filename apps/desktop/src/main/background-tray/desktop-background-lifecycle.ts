/**
 * Whether Desktop keeps running after its window closes: in the notification area on Windows, and
 * in the Dock on macOS.
 *
 * Closing the window while terminals run hides it instead. On Windows the notification-area icon
 * shows until the window shows again or the last terminal ends, which quits Desktop; on macOS the
 * Dock stays, as it does for every macOS application, and the last terminal ending closes the hidden
 * window instead of quitting. Nothing intercepts a close once the application is quitting, a
 * development rebuild restarts it, or the operating system ends the person's session, so Desktop
 * never holds up a quit, a restart, logoff, or shutdown.
 *
 * Quit All, from the icon's menu or the Dock and application menus, stops the Session and then
 * quits, but only once the person confirms it and the stop has completed; a stop that fails shows
 * the window with the reason and nothing quits.
 *
 * Invariant, while the application is not ending: a hidden window has terminals running or a Quit
 * All stopping or reporting its failure, and on Windows the icon shows exactly then, so a hidden
 * window always has an icon to reopen it from.
 */
import type { DesktopBackgroundStatusSummary } from '@orchbee/desktop-client-protocol';

/**
 * The window as Main last saw it; `absent` before the first one shows, after it closes, and from the
 * moment Main closes a hidden window that nothing is left to reopen for.
 */
type DesktopBackgroundWindow = 'absent' | 'hidden' | 'shown';

/**
 * `quitting` covers every quit, including the development runtime's restart after a rebuild;
 * `session-ending` is Windows logging off or shutting down. Neither ever returns to `none`.
 */
type DesktopBackgroundEnding = 'none' | 'quitting' | 'session-ending';

/** Why a Quit All left Desktop running, as its report names it. */
export type DesktopQuitAllFailure =
	| 'busy'
	| 'not-sent'
	| 'refused'
	| 'stop-failed'
	| 'unconfirmed'
	| 'unmanaged';

/**
 * Quit All's progress: `confirming` while the person decides, `stopping` from their confirmation
 * until the Session's stop completes or fails, and `failed` while the shown window reports why.
 */
type DesktopQuitAll =
	| { readonly kind: 'confirming' }
	| { readonly kind: 'failed'; readonly reason: DesktopQuitAllFailure }
	| { readonly kind: 'idle' }
	| { readonly kind: 'stopping' };

/**
 * What Desktop does once a hidden window has nothing left to reopen for: Windows quits, because the
 * icon is all that would remain; macOS closes the window and stays in the Dock.
 */
export type DesktopBackgroundRelease = 'close-window' | 'quit';

export interface DesktopBackgroundState {
	readonly ending: DesktopBackgroundEnding;
	/** Whether the one-time notice that Orchbee keeps running has been shown, in this or an earlier run. */
	readonly noticeShown: boolean;
	readonly quitAll: DesktopQuitAll;
	readonly release: DesktopBackgroundRelease;
	/** The page's latest summary; all zero until it reports one. */
	readonly summary: DesktopBackgroundStatusSummary;
	readonly window: DesktopBackgroundWindow;
}

export type DesktopBackgroundEvent =
	/** The person closed the window: its close button, the taskbar, or `Alt+F4`. */
	| { readonly kind: 'close-requested' }
	| { readonly kind: 'notice-shown' }
	/** The person chose Quit All from the icon's menu, the Dock menu, or the application menu. */
	| { readonly kind: 'quit-all-requested' }
	| { readonly kind: 'quit-all-cancelled' }
	| { readonly kind: 'quit-all-confirmed' }
	/** The Session's stop completed. */
	| { readonly kind: 'quit-all-stopped' }
	/** The Session's stop failed. */
	| { readonly kind: 'quit-all-failed'; readonly reason: DesktopQuitAllFailure }
	/** The person dismissed the failure report. */
	| { readonly kind: 'quit-all-failure-dismissed' }
	| { readonly kind: 'quit-requested' }
	/** Windows asked whether its session may end, or said it is ending. */
	| { readonly kind: 'session-ending' }
	| { readonly kind: 'summary-changed'; readonly summary: DesktopBackgroundStatusSummary }
	| { readonly kind: 'window-closed' }
	/** The window showed, however it was asked to: the icon, another start, a notification. */
	| { readonly kind: 'window-shown' };

type DesktopBackgroundCommand =
	/** Ask the person to confirm Quit All, naming the terminals and Agents it stops. */
	| { readonly kind: 'confirm-quit-all' }
	/** Close the hidden window, which nothing is left to reopen for; Desktop keeps running. */
	| { readonly kind: 'close-window' }
	/** Cancel the close in progress and hide the window. */
	| { readonly kind: 'hide-window' }
	| { readonly kind: 'quit' }
	/** Persist that the notice has been shown, so later runs do not show it again. */
	| { readonly kind: 'record-notice' }
	/** Show the window and report why Quit All left Desktop running. */
	| { readonly kind: 'report-quit-all-failure'; readonly reason: DesktopQuitAllFailure }
	| { readonly kind: 'show-notice' }
	/** Stop the Session; its completion or failure returns as an event. */
	| { readonly kind: 'stop-session' };

export interface DesktopBackgroundTransition {
	readonly commands: readonly DesktopBackgroundCommand[];
	readonly state: DesktopBackgroundState;
}

/** The Agent state the icon marks, most urgent first; `none` marks nothing. */
export type DesktopTrayStatus = 'attention' | 'done' | 'failed' | 'none';

export interface DesktopTrayView {
	readonly status: DesktopTrayStatus;
	/** Whether Quit All is stopping the Session, when the menu offers only to reopen the window. */
	readonly stopping: boolean;
	readonly summary: DesktopBackgroundStatusSummary;
}

const EMPTY_SUMMARY: DesktopBackgroundStatusSummary = {
	attention: 0,
	doneUnseen: 0,
	failed: 0,
	runningTerminals: 0,
	workingAgents: 0,
};

/** The status line of the Dock menu. */
type DesktopDockStatus =
	/** No window's page reports, so nothing is known. */
	| { readonly kind: 'none' }
	| { readonly kind: 'stopping' }
	| { readonly kind: 'summary'; readonly summary: DesktopBackgroundStatusSummary };

/** What the macOS Dock menu and the application menu's background items offer. */
export interface DesktopDockView {
	/** Whether Quit Orchbee, Keep Terminals Running is offered; not while Quit All stops the Session. */
	readonly quit: boolean;
	/** Whether Quit All can be asked for: a window's page reports running terminals and none is under way. */
	readonly quitAll: boolean;
	/** Whether Reopen is offered: no window shows. */
	readonly reopen: boolean;
	readonly status: DesktopDockStatus;
}

export function createDesktopBackgroundState(
	noticeShown: boolean,
	release: DesktopBackgroundRelease,
): DesktopBackgroundState {
	return {
		ending: 'none',
		noticeShown,
		quitAll: { kind: 'idle' },
		release,
		summary: EMPTY_SUMMARY,
		window: 'absent',
	};
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
		case 'quit-all-requested':
			return state.ending === 'none' && state.quitAll.kind === 'idle'
				? {
						commands: [{ kind: 'confirm-quit-all' }],
						state: { ...state, quitAll: { kind: 'confirming' } },
					}
				: unchanged(state);
		case 'quit-all-cancelled':
			return state.quitAll.kind === 'confirming'
				? { commands: [], state: { ...state, quitAll: { kind: 'idle' } } }
				: unchanged(state);
		case 'quit-all-confirmed':
			if (state.quitAll.kind !== 'confirming') return unchanged(state);
			// A quit, or the end of the person's session, while they decided has settled it already.
			return state.ending === 'none'
				? {
						commands: [{ kind: 'stop-session' }],
						state: { ...state, quitAll: { kind: 'stopping' } },
					}
				: { commands: [], state: { ...state, quitAll: { kind: 'idle' } } };
		case 'quit-all-stopped':
			return state.quitAll.kind === 'stopping' && state.ending === 'none'
				? { commands: [{ kind: 'quit' }], state: { ...state, ending: 'quitting' } }
				: unchanged(state);
		case 'quit-all-failed':
			if (state.quitAll.kind !== 'stopping' || state.ending !== 'none') return unchanged(state);
			return {
				commands: [{ kind: 'report-quit-all-failure', reason: event.reason }],
				state: { ...state, quitAll: { kind: 'failed', reason: event.reason } },
			};
		case 'quit-all-failure-dismissed': {
			if (state.quitAll.kind !== 'failed') return unchanged(state);
			const next: DesktopBackgroundState = { ...state, quitAll: { kind: 'idle' } };
			// The report shows over the window; if the window stayed hidden and no terminal runs, the
			// window has nothing left to reopen for, as when the last terminal ends.
			return state.ending === 'none' && state.window === 'hidden' && !hasRunningTerminals(state)
				? releaseHiddenWindow(next)
				: { commands: [], state: next };
		}
		case 'quit-requested':
			return state.ending === 'none'
				? { commands: [], state: { ...state, ending: 'quitting' } }
				: unchanged(state);
		case 'session-ending':
			return { commands: [], state: { ...state, ending: 'session-ending' } };
		case 'summary-changed': {
			const next = { ...state, summary: event.summary };
			// The last terminal ended while the window was hidden: nothing remains to reopen. A Quit
			// All that is stopping the Session ends every terminal first and quits only once the stop
			// completes; one that failed leaves Desktop running.
			if (
				state.ending === 'none' &&
				state.window === 'hidden' &&
				!hasRunningTerminals(next) &&
				!quitAllUnderWay(state)
			) {
				return releaseHiddenWindow(next);
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
	if (
		state.ending !== 'none' ||
		state.window !== 'hidden' ||
		(!hasRunningTerminals(state) && !quitAllUnderWay(state))
	) {
		return null;
	}
	const { summary } = state;
	const stopping = state.quitAll.kind === 'stopping';
	return { status: stopping ? 'none' : agentStatus(summary), stopping, summary };
}

/** What the Dock menu offers; the application menu's Reopen and Quit All follow the same view. */
export function projectDesktopDock(state: DesktopBackgroundState): DesktopDockView {
	const stopping = state.quitAll.kind === 'stopping';
	const reporting = state.window !== 'absent';
	let status: DesktopDockStatus = { kind: 'none' };
	if (stopping) status = { kind: 'stopping' };
	else if (reporting) status = { kind: 'summary', summary: state.summary };
	return {
		quit: !stopping,
		quitAll:
			reporting &&
			state.ending === 'none' &&
			state.quitAll.kind === 'idle' &&
			hasRunningTerminals(state),
		reopen: state.window !== 'shown',
		status,
	};
}

/** The most urgent Agent state among those the summary counts. */
function agentStatus(summary: DesktopBackgroundStatusSummary): DesktopTrayStatus {
	if (summary.failed > 0) return 'failed';
	if (summary.attention > 0) return 'attention';
	return summary.doneUnseen > 0 ? 'done' : 'none';
}

function hasRunningTerminals(state: DesktopBackgroundState): boolean {
	return state.summary.runningTerminals > 0;
}

/** Whether a confirmed Quit All is stopping the Session or reporting why it could not. */
function quitAllUnderWay(state: DesktopBackgroundState): boolean {
	return state.quitAll.kind === 'stopping' || state.quitAll.kind === 'failed';
}

/** Ends a hidden window that nothing is left to reopen for, as the platform's release says. */
function releaseHiddenWindow(state: DesktopBackgroundState): DesktopBackgroundTransition {
	return state.release === 'quit'
		? { commands: [{ kind: 'quit' }], state: { ...state, ending: 'quitting' } }
		: { commands: [{ kind: 'close-window' }], state: { ...state, window: 'absent' } };
}

function unchanged(state: DesktopBackgroundState): DesktopBackgroundTransition {
	return { commands: [], state };
}
