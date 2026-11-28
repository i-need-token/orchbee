import type { ActionScope } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';

import type { FileOverlayOpenFile } from '../../file-overlay';
import { filePathKey, type FilePathRules, type FileWorkspaceSession } from '../../file-session';
import type { VirtualTreeOpenEvent } from '../../virtual-tree';
import { canOpenExplorerItem } from '../model/canOpenExplorerItem';
import type { ExplorerItem } from '../model/explorer-item';

/**
 * Opens a file the tree asks to open in the file overlay, as VS Code's Explorer opens it in the
 * editor: a click or Space as a preview that leaves the keys in the tree, a double click, Enter or a
 * middle click pinned, and Open to the Side pinned with the keys, since the overlay has one group. A
 * folder's open only expands it, which the tree has done, and an entry that is neither a file nor a
 * link to one, or whose name no request can carry, opens nothing.
 */
export function useExplorerOpen(
	openFile: FileOverlayOpenFile,
	scope: ActionScope,
	session: FileWorkspaceSession | null,
	rules: FilePathRules | null,
): (event: VirtualTreeOpenEvent<string, ExplorerItem>) => void {
	return useMemoizedFn((event: VirtualTreeOpenEvent<string, ExplorerItem>) => {
		const item = event.row.element;
		if (session === null || rules === null || !canOpenExplorerItem(item)) return;
		const { path } = item;
		openFile(
			{
				focus: event.sideBySide || !event.preserveFocus,
				preview: !event.sideBySide && !event.pinned,
				resource: { key: filePathKey(path, rules), path },
				workspaceId: session.workspaceId,
			},
			// The tree turned a click or a key into the open, so the view runs the Action itself.
			{ scope, source: 'programmatic' },
		);
	});
}
