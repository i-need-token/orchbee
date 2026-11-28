import {
	EMPTY_FILE_TABS,
	restoreFileTabs,
	snapshotFileTabs,
	transitionFileTabs,
	type FileTabs,
	type FileTabsEffect,
	type FileTabsEvent,
	type FileTabsSnapshot,
} from './file-tabs';
import {
	closeFileOverlayMode,
	createFileOverlayMode,
	openFileOverlayMode,
	transitionFileOverlayMode,
	type FileOverlayMode,
	type FileOverlayModeEvent,
	type FileOverlayNarrowTier,
	type FileOverlayTier,
} from './overlay-mode';

/** One Workspace's file overlay: closed exactly when it has no tabs. */
export interface FileOverlayState {
	readonly mode: FileOverlayMode;
	readonly tabs: FileTabs;
}

export type FileOverlayEvent = FileOverlayModeEvent | FileTabsEvent;

/** A command for whoever runs the overlay; its outcome, if any, returns as a later event. */
export type FileOverlayEffect =
	/** Give the keyboard to the current terminal, as dismissing the overlay does. */
	{ readonly kind: 'focus-terminal' } | FileTabsEffect;

interface FileOverlayTransition {
	readonly effects: readonly FileOverlayEffect[];
	readonly state: FileOverlayState;
}

const NO_EFFECTS: readonly FileOverlayEffect[] = [];
const FOCUS_TERMINAL: readonly FileOverlayEffect[] = [{ kind: 'focus-terminal' }];

export function createFileOverlayState(leftExpandOffered: boolean): FileOverlayState {
	return { mode: createFileOverlayMode(leftExpandOffered), tabs: EMPTY_FILE_TABS };
}

/**
 * A file shown from any source, by opening, reopening or activating one, brings a closed or
 * picture-in-picture overlay up at its tier, as does a close that asks the save question about the
 * tab it activates; closing its last tab closes it. Dismissing the open overlay hands the keyboard to
 * the terminal; a replaced terminal has already taken it.
 */
export function transitionFileOverlay(
	state: FileOverlayState,
	event: FileOverlayEvent,
): FileOverlayTransition {
	switch (event.type) {
		case 'dismissed':
		case 'full-toggled':
		case 'layout-changed':
		case 'pip-restored':
		case 'terminal-replaced':
		case 'tier-chosen': {
			const mode = transitionFileOverlayMode(state.mode, event);
			if (mode === state.mode) return { effects: NO_EFFECTS, state };
			return {
				effects: event.type === 'dismissed' ? FOCUS_TERMINAL : NO_EFFECTS,
				state: { ...state, mode },
			};
		}
		default: {
			const { effects, tabs } = transitionFileTabs(state.tabs, event);
			let mode = state.mode;
			if (tabs.tabs.length === 0) mode = closeFileOverlayMode(mode);
			else if (shows(event, state.tabs, tabs)) mode = openFileOverlayMode(mode);
			if (tabs === state.tabs && mode === state.mode) return { effects, state };
			return { effects, state: { mode, tabs } };
		}
	}
}

/**
 * Whether the event puts a file in front: any open, even of the tab already active, an activation of
 * an open tab, a reopen that brought a tab back, or a close that waits on the save question.
 */
function shows(event: FileTabsEvent, before: FileTabs, after: FileTabs): boolean {
	switch (event.type) {
		case 'file-opened':
			return true;
		case 'tab-activated':
			return after.tabs.some((tab) => tab.id === event.id);
		case 'reopen-requested':
			return after.tabs.length > before.tabs.length;
		case 'close-requested':
			return before.pendingClose === null && after.pendingClose !== null;
		default:
			return false;
	}
}

/**
 * What a Workspace's overlay keeps from one run to the next: its tabs, whether it showed them open or
 * shrunk to picture-in-picture, and its tier.
 */
export interface FileOverlaySnapshot {
	readonly kind: 'open' | 'pip';
	readonly restoreTier: FileOverlayNarrowTier;
	readonly tabs: FileTabsSnapshot;
	readonly tier: FileOverlayTier;
}

/** The overlay as a later run restores it, with its `limit` most recently active tabs; null while closed. */
export function snapshotFileOverlay(
	state: FileOverlayState,
	limit: number,
): FileOverlaySnapshot | null {
	const { kind, restoreTier, tier } = state.mode;
	const tabs = snapshotFileTabs(state.tabs, limit);
	if (kind === 'closed' || tabs === null) return null;
	return { kind, restoreTier, tabs, tier };
}

/**
 * The overlay a snapshot restores in a window that offers left-expand or not. It comes back shrunk to
 * picture-in-picture even if it was open: the terminal takes the keys as a Workspace first shows,
 * which would shrink an open overlay over it at once. A tier the window does not offer falls back to
 * default, as when the window stops offering it. Null for tabs that break the row's rules.
 */
export function restoreFileOverlay(
	snapshot: FileOverlaySnapshot,
	leftExpandOffered: boolean,
): FileOverlayState | null {
	const tabs = restoreFileTabs(snapshot.tabs);
	if (tabs === null) return null;
	const remembered: FileOverlayMode = {
		kind: 'pip',
		leftExpandOffered: true,
		restoreTier: snapshot.restoreTier,
		tier: snapshot.tier,
	};
	return {
		mode: transitionFileOverlayMode(remembered, { leftExpandOffered, type: 'layout-changed' }),
		tabs,
	};
}
