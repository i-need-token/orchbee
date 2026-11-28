import { fileExplorerMessages, type MessageDescriptor } from '@orchbee/i18n';

import { FILE_EXPLORER_ACTION_IDS } from '../../file-explorer-actions';

/**
 * A command of the Explorer's context menu, named after the VS Code command it stands for:
 * `open-to-side` is `explorer.openToSide`, `open-in-terminal` is `openInIntegratedTerminal`,
 * `copy-path` is `copyFilePath` and `copy-relative-path` is `copyRelativeFilePath`. `insert-paths`,
 * Insert Path into Terminal, is Orchbee's own.
 */
export type ExplorerMenuCommand =
	| 'copy-path'
	| 'copy-relative-path'
	| 'insert-paths'
	| 'open-in-terminal'
	| 'open-to-side';

/**
 * What a menu is for: a file's row, a folder's, or the empty space below the rows, which stands for
 * the root, as in VS Code.
 */
export type ExplorerMenuTarget = 'file' | 'folder' | 'root';

/** The Action each command runs, and the title the Action and its menu row show. */
export const EXPLORER_MENU_COMMANDS = {
	'copy-path': {
		actionId: FILE_EXPLORER_ACTION_IDS.copyPath,
		title: fileExplorerMessages.copyPath,
	},
	'copy-relative-path': {
		actionId: FILE_EXPLORER_ACTION_IDS.copyRelativePath,
		title: fileExplorerMessages.copyRelativePath,
	},
	'insert-paths': {
		actionId: FILE_EXPLORER_ACTION_IDS.insertPaths,
		title: fileExplorerMessages.insertPaths,
	},
	'open-in-terminal': {
		actionId: FILE_EXPLORER_ACTION_IDS.openInTerminal,
		title: fileExplorerMessages.openInTerminal,
	},
	'open-to-side': {
		actionId: FILE_EXPLORER_ACTION_IDS.openToSide,
		title: fileExplorerMessages.openToSide,
	},
} as const satisfies Readonly<
	Record<ExplorerMenuCommand, { readonly actionId: string; readonly title: MessageDescriptor }>
>;

/**
 * The Explorer's context menu for a target, group by group, as VS Code's shows the commands Orchbee
 * offers at its default settings: Open to the Side for a file and Open in Integrated Terminal, then
 * Copy Path and Copy Relative Path. VS Code sorts the groups by its menu rule, `navigation` first and
 * then by name. Insert Path into Terminal follows VS Code's terminal commands in their group.
 */
export function explorerMenu(
	target: ExplorerMenuTarget,
): readonly (readonly ExplorerMenuCommand[])[] {
	const terminal: readonly ExplorerMenuCommand[] = ['open-in-terminal', 'insert-paths'];
	const copy: readonly ExplorerMenuCommand[] = ['copy-path', 'copy-relative-path'];
	return [target === 'file' ? ['open-to-side', ...terminal] : terminal, copy];
}
