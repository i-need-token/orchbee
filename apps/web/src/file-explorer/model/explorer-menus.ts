import { fileExplorerMessages, type MessageDescriptor } from '@orchbee/i18n';

import { FILE_EXPLORER_ACTION_IDS } from '../../file-explorer-actions';

/**
 * A command of the Explorer's context menu, named after the VS Code command it stands for:
 * `open-to-side` is `explorer.openToSide`, `copy-path` is `copyFilePath` and `copy-relative-path`
 * is `copyRelativeFilePath`.
 */
export type ExplorerMenuCommand = 'copy-path' | 'copy-relative-path' | 'open-to-side';

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
	'open-to-side': {
		actionId: FILE_EXPLORER_ACTION_IDS.openToSide,
		title: fileExplorerMessages.openToSide,
	},
} as const satisfies Readonly<
	Record<ExplorerMenuCommand, { readonly actionId: string; readonly title: MessageDescriptor }>
>;

/**
 * The Explorer's context menu for a target, group by group, as VS Code's shows the commands Orchbee
 * offers at its default settings: Open to the Side for a file, then Copy Path and Copy Relative
 * Path. VS Code sorts the groups by its menu rule, `navigation` first and then by name.
 */
export function explorerMenu(
	target: ExplorerMenuTarget,
): readonly (readonly ExplorerMenuCommand[])[] {
	const copy: readonly ExplorerMenuCommand[] = ['copy-path', 'copy-relative-path'];
	return target === 'file' ? [['open-to-side'], copy] : [copy];
}
