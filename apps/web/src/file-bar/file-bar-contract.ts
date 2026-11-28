import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import type { ComponentType } from 'react';

import type { FileExplorerProps } from '../file-explorer';
import type { FileWorkspaceSession } from '../file-session';
import type { FileBarViewId } from './file-bar-layout';

/** What the Workbench tells the File Bar's column, which owns the views and their toggles. */
export interface FileBarColumnProps {
	readonly actionRuntime: ActionRuntime;
	/** The scope the views' Actions nest in while the column shows. */
	readonly actionScope: ActionScope;
	/** The view the column shows. */
	readonly activeView: FileBarViewId;
	/** Where the views report what VS Code would notify. */
	readonly notifications: FileExplorerProps['notifications'];
	/** Selecting a view's toggle: the active view takes focus, another one shows. */
	readonly onSelectView: (view: FileBarViewId) => void;
	/** Which platform's keys the views answer to, as VS Code's lists do on it. */
	readonly platform: KeyboardShortcutPlatform;
	/** The active Workspace's file session the File Bar holds; null while it holds none. */
	readonly session: FileWorkspaceSession | null;
	/** Each increase moves focus into the active view; zero asks for nothing. */
	readonly viewFocusRequest: number;
}

/** What the File Bar's chunk gives the Workbench, which places it and decides when it shows. */
export interface FileBarModule {
	/** The column's content inside the Workbench's File Bar landmark, from its 36px row down. */
	readonly FileBarColumn: ComponentType<FileBarColumnProps>;
}
