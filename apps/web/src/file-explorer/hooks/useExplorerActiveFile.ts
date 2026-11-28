import * as React from 'react';

import type { FileResource } from '../../file-session';
import type { ExplorerTree } from '../model/ExplorerTree';

/**
 * Has the Explorer's tree follow the file the overlay's active tab shows, as VS Code's Explorer
 * follows the active editor: the entries `files.exclude` hides show on the way to it, and while the
 * Explorer shows, the file is revealed whenever it changes and whenever the Explorer comes to show,
 * as VS Code's `explorer.autoReveal` does by default. A reveal still expanding folders ends once the
 * file changes or the Explorer hides.
 */
export function useExplorerActiveFile(
	tree: ExplorerTree | null,
	activeFile: FileResource | null,
	shown: boolean,
): void {
	const path = activeFile?.path ?? null;
	React.useEffect(() => {
		tree?.setActiveFile(path);
	}, [path, tree]);
	React.useEffect(() => {
		if (tree === null || path === null || !shown) return;
		const controller = new AbortController();
		tree.revealFile(path, controller.signal);
		return () => {
			controller.abort();
		};
	}, [path, shown, tree]);
}
