import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import type { ComponentType } from 'react';

import type { FileExplorerProps } from '../file-explorer';
import type { FileResource, FileWorkspaceSession } from '../file-session';
import type { SourceControlNotifications, SourceControlQuickInput } from '../source-control';
import type { UserSettingsControl } from '../user-settings';
import type { FileBarViewId } from './file-bar-layout';
import type { FileBarLayoutStorage } from './file-bar-layout-storage';

/** What the Workbench tells the File Bar's column, which owns the views and their toggles. */
export interface FileBarColumnProps {
	readonly actionRuntime: ActionRuntime;
	/** The scope the views' Actions nest in while the column shows. */
	readonly actionScope: ActionScope;
	/** The file the overlay's active tab shows, as VS Code's active editor; null without tabs. */
	readonly activeFile: FileResource | null;
	/** The view the column shows. */
	readonly activeView: FileBarViewId;
	/** Where the views report what VS Code would notify. */
	readonly notifications: FileExplorerProps['notifications'] & SourceControlNotifications;
	/** Selecting a view's toggle: the active view takes focus, another one shows. */
	readonly onSelectView: (view: FileBarViewId) => void;
	/** Which platform's keys the views answer to, as VS Code's lists do on it. */
	readonly platform: KeyboardShortcutPlatform;
	/** The window's quick input, where the views ask what VS Code asks there. */
	readonly quickInput: SourceControlQuickInput;
	/** The active Workspace's file session the File Bar holds; null while it holds none. */
	readonly session: FileWorkspaceSession | null;
	/** The user's settings, which Source Control reads and answers into; without them it keeps defaults. */
	readonly settings: UserSettingsControl | undefined;
	/** Device-local storage for what the views remember between runs; without it they remember nothing. */
	readonly storage: FileBarLayoutStorage | undefined;
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
