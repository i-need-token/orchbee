import {
	createTypedActionRuntime,
	type ActionCatalogId,
	type ActionRuntime,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import * as React from 'react';

import {
	FILE_EXPLORER_ACTION_IDS,
	type FileExplorerActionCatalog,
} from '../../file-explorer-actions';
import type { FileOverlayOpenFile } from '../../file-overlay';
import type { FileWorkspaceSession } from '../../file-session';
import { registerFileExplorerActions } from '../actions/registerFileExplorerActions';
import type { ExplorerTree } from '../model/ExplorerTree';
import type { ExplorerClipboard } from '../types/explorer-clipboard';
import type { ExplorerTerminals } from '../types/explorer-terminals';

interface UseFileExplorerActionsOptions {
	/** Opens a file in the file overlay. */
	readonly openFile: FileOverlayOpenFile;
	/** The scope the Explorer's own scope nests in. */
	readonly parentScope: ActionScope;
	readonly runtime: ActionRuntime;
	/** The Workspace whose overlay the Explorer's files open in; null while there is none. */
	readonly session: FileWorkspaceSession | null;
	/** Where the terminal commands act. */
	readonly terminals: ExplorerTerminals;
	/** The tree the Actions act on; null while there is none. */
	readonly tree: ExplorerTree | null;
}

/** The Explorer's own Action scope, and the header's buttons, each running its Action as a press. */
interface FileExplorerCommands {
	/** The adapter the Explorer's Actions register through, which alone runs them. */
	readonly actions: TypedActionRuntime<FileExplorerActionCatalog>;
	readonly collapseFolders: () => void;
	readonly refresh: () => void;
	/** Where the Explorer's Actions register, which its tree's keys and menu resolve in too. */
	readonly scope: ActionScope;
}

/**
 * The window's clipboard. A browser offers none to a page served without a secure origin, which then
 * fails the write as a refused one does.
 */
function windowClipboard(): ExplorerClipboard {
	return {
		lineBreak: navigator.platform.startsWith('Win') ? '\r\n' : '\n',
		writeText: (text) => navigator.clipboard.writeText(text),
	};
}

/**
 * The Explorer's Actions, registered in the Explorer's own scope while it shows, so Action search
 * finds its title Actions only then.
 */
export function useFileExplorerActions({
	openFile,
	parentScope,
	runtime,
	session,
	terminals,
	tree,
}: UseFileExplorerActionsOptions): FileExplorerCommands {
	const scope = React.useMemo(
		() => runtime.createScope('file-explorer', parentScope),
		[parentScope, runtime],
	);
	const typedRuntime = React.useMemo(
		() => createTypedActionRuntime<FileExplorerActionCatalog>(runtime),
		[runtime],
	);
	React.useEffect(() => {
		const registrations = registerFileExplorerActions({
			clipboard: windowClipboard(),
			openFile,
			scope,
			session,
			terminals,
			tree,
			typedRuntime,
		});
		return () => {
			for (const registration of registrations) registration.dispose();
		};
	}, [openFile, scope, session, terminals, tree, typedRuntime]);
	return React.useMemo(() => {
		// The handlers cannot fail, and an unavailable Action is a disabled button, so no outcome shows.
		const press = (actionId: ActionCatalogId<FileExplorerActionCatalog>): void => {
			typedRuntime.execute(actionId, undefined, { scope, source: 'button' });
		};
		return {
			actions: typedRuntime,
			collapseFolders: () => press(FILE_EXPLORER_ACTION_IDS.collapseFolders),
			refresh: () => press(FILE_EXPLORER_ACTION_IDS.refresh),
			scope,
		};
	}, [scope, typedRuntime]);
}
