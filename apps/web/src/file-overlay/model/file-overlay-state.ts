import {
	EMPTY_FILE_TABS,
	transitionFileTabs,
	type FileTabs,
	type FileTabsEffect,
	type FileTabsEvent,
} from './file-tabs';
import {
	closeFileOverlayMode,
	createFileOverlayMode,
	openFileOverlayMode,
	transitionFileOverlayMode,
	type FileOverlayMode,
	type FileOverlayModeEvent,
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
 * picture-in-picture overlay up at its tier, and closing its last tab closes it. Dismissing the open
 * overlay hands the keyboard to the terminal; a replaced terminal has already taken it.
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
 * an open tab, or a reopen that brought a tab back.
 */
function shows(event: FileTabsEvent, before: FileTabs, after: FileTabs): boolean {
	switch (event.type) {
		case 'file-opened':
			return true;
		case 'tab-activated':
			return after.tabs.some((tab) => tab.id === event.id);
		case 'reopen-requested':
			return after.tabs.length > before.tabs.length;
		default:
			return false;
	}
}
