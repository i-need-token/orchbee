import type { ComponentType } from 'react';

import type { FileBarViewId } from './file-bar-layout';

/** What the Workbench tells the File Bar's column, which owns the views and their toggles. */
export interface FileBarColumnProps {
	/** The view the column shows. */
	readonly activeView: FileBarViewId;
	/** Selecting a view's toggle: the active view takes focus, another one shows. */
	readonly onSelectView: (view: FileBarViewId) => void;
	/** Each increase moves focus into the active view; zero asks for nothing. */
	readonly viewFocusRequest: number;
}

/** What the File Bar's chunk gives the Workbench, which places it and decides when it shows. */
export interface FileBarModule {
	/** The column's content inside the Workbench's File Bar landmark, from its 36px row down. */
	readonly FileBarColumn: ComponentType<FileBarColumnProps>;
}
