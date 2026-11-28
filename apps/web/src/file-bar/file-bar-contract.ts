import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import type { ComponentType, ReactElement } from 'react';

import type { FileExplorerProps } from '../file-explorer';
import type { FileResource, FileWorkspaceSession } from '../file-session';
import type { SourceControlNotifications, SourceControlQuickInput } from '../source-control';
import type { UserSettingsControl } from '../user-settings';
import type { FileBarViewId } from './file-bar-layout';
import type { FileBarLayoutStorage } from './file-bar-layout-storage';

/** One row of the toggles row's menu. */
export type FileBarRowMenuEntry =
	| {
			/** Checked while the view's toggle shows in the row. */
			readonly checked: boolean;
			/** The last toggle shown cannot be hidden. */
			readonly disabled: boolean;
			readonly id: FileBarViewId;
			readonly kind: 'toggle';
			readonly label: string;
			readonly run: () => void;
			readonly shortcut: string | undefined;
	  }
	| {
			readonly disabled: boolean;
			readonly id: string;
			readonly kind: 'command';
			readonly label: string;
			readonly run: () => void;
			readonly shortcut: string | undefined;
	  }
	| { readonly id: string; readonly kind: 'separator' };

/** What the Workbench tells the File Bar's column, which owns the views and their toggles. */
export interface FileBarColumnProps {
	readonly actionRuntime: ActionRuntime;
	/** The scope the views' Actions nest in while the column shows. */
	readonly actionScope: ActionScope;
	/** The file the overlay's active tab shows, as VS Code's active editor; null without tabs. */
	readonly activeFile: FileResource | null;
	/** The view the column shows. */
	readonly activeView: FileBarViewId;
	/**
	 * Whether the column touches the window's end, as it does on the right of the terminal area or
	 * open across it, so its row keeps the places of the File Bar button and the caption buttons that
	 * float over it.
	 */
	readonly atWindowEnd: boolean;
	/**
	 * What starts the toggles row while the column touches the window's start: the place of the
	 * Sidebar button, and of the traffic lights before it, that float over it. Null elsewhere.
	 */
	readonly leadingPlace: ReactElement | null;
	/** Where the views report what VS Code would notify. */
	readonly notifications: FileExplorerProps['notifications'] & SourceControlNotifications;
	/** Selecting a view's toggle: the active view takes focus, another one shows. */
	readonly onSelectView: (view: FileBarViewId) => void;
	/** Opens a file in the shown Workspace's overlay, where the views open their files. */
	readonly openFile: FileExplorerProps['openFile'];
	/** Which platform's keys the views answer to, as VS Code's lists do on it. */
	readonly platform: KeyboardShortcutPlatform;
	/** The window's quick input, where the views ask what VS Code asks there. */
	readonly quickInput: SourceControlQuickInput;
	/** Runs an Action chosen from a view's context menu and says its refusal or failure. */
	readonly reportActionOutcome: FileExplorerProps['reportActionOutcome'];
	/** Gives the keys back to the current terminal, which Escape in the Explorer does. */
	readonly returnToTerminal: FileExplorerProps['returnToTerminal'];
	/**
	 * The toggles row's menu as it opens, VS Code's activity bar menu with File Bar wording: a checkbox
	 * for each view's toggle, then moving and hiding the File Bar.
	 */
	readonly rowMenu: () => readonly FileBarRowMenuEntry[];
	/** The active Workspace's file session the File Bar holds; null while it holds none. */
	readonly session: FileWorkspaceSession | null;
	/** The user's settings, which Source Control reads and answers into; without them it keeps defaults. */
	readonly settings: UserSettingsControl | undefined;
	/** The keys that run an Action now, which the views' context menus show beside their commands. */
	readonly shortcutLabelForAction: FileExplorerProps['shortcutLabelForAction'];
	/** Device-local storage for what the views remember between runs; without it they remember nothing. */
	readonly storage: FileBarLayoutStorage | undefined;
	/** The view toggles the row shows, in its order. */
	readonly toggles: readonly FileBarViewId[];
	/** Each increase moves focus into the active view; zero asks for nothing. */
	readonly viewFocusRequest: number;
}

/** What the Workbench tells the count badge it places on the File Bar's own button. */
export interface FileBarToggleBadgeProps {
	/** The id the button's description points to, which says how many changes the badge counts. */
	readonly descriptionId: string;
	/** The active Workspace's file session, held because the File Bar has shown it in this run. */
	readonly session: FileWorkspaceSession;
}

/** What the File Bar's chunk gives the Workbench, which places it and decides when it shows. */
export interface FileBarModule {
	/** The column's content inside the Workbench's File Bar landmark, from its 36px row down. */
	readonly FileBarColumn: ComponentType<FileBarColumnProps>;
	/** Source Control's count badge, which the File Bar's button carries while the column is hidden. */
	readonly FileBarToggleBadge: ComponentType<FileBarToggleBadgeProps>;
}
