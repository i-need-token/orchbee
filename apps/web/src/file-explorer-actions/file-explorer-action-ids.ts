import type { ActionSignature } from '@orchbee/actions';

/**
 * The Explorer's Actions. The Explorer registers them in its own scope while it shows, where they act
 * on its rows as VS Code's Explorer commands do. The Workbench's shortcut table binds their keys, so
 * the ids live apart from the Explorer, whose code loads with the File Bar.
 */
export const FILE_EXPLORER_ACTION_IDS = {
	collapseFolders: 'file-explorer.folders.collapse',
	copyPath: 'file-explorer.path.copy',
	copyRelativePath: 'file-explorer.relative-path.copy',
	insertPaths: 'file-explorer.paths.insert',
	openInTerminal: 'file-explorer.terminal.open',
	openToSide: 'file-explorer.file.open-to-side',
	refresh: 'file-explorer.tree.refresh',
} as const;

/** Each Action runs without input: a row's commands act on the Explorer's selection and focus. */
export interface FileExplorerActionCatalog {
	readonly [FILE_EXPLORER_ACTION_IDS.collapseFolders]: ActionSignature<void>;
	readonly [FILE_EXPLORER_ACTION_IDS.copyPath]: ActionSignature<void>;
	readonly [FILE_EXPLORER_ACTION_IDS.copyRelativePath]: ActionSignature<void>;
	readonly [FILE_EXPLORER_ACTION_IDS.insertPaths]: ActionSignature<void>;
	readonly [FILE_EXPLORER_ACTION_IDS.openInTerminal]: ActionSignature<void>;
	readonly [FILE_EXPLORER_ACTION_IDS.openToSide]: ActionSignature<void>;
	readonly [FILE_EXPLORER_ACTION_IDS.refresh]: ActionSignature<void>;
}
