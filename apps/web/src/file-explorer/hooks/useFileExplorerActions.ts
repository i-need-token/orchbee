import {
	createTypedActionRuntime,
	defineAction,
	noActionInput,
	type ActionRuntime,
	type ActionScope,
	type ActionSignature,
} from '@orchbee/actions';
import { fileExplorerMessages, workbenchMessages } from '@orchbee/i18n';
import * as React from 'react';

import type { ExplorerTree } from '../model/ExplorerTree';
import { hasExpandedFolder } from '../model/hasExpandedFolder';

const FILE_EXPLORER_ACTION_IDS = {
	collapseFolders: 'file-explorer.folders.collapse',
	refresh: 'file-explorer.tree.refresh',
} as const;

interface FileExplorerActionCatalog {
	readonly [FILE_EXPLORER_ACTION_IDS.collapseFolders]: ActionSignature<void>;
	readonly [FILE_EXPLORER_ACTION_IDS.refresh]: ActionSignature<void>;
}

type FileExplorerActionId = keyof FileExplorerActionCatalog;

const VIEW_CATEGORY = { id: 'view', title: workbenchMessages.actionCategoryView };

interface UseFileExplorerActionsOptions {
	/** The scope the Explorer's own scope nests in. */
	readonly parentScope: ActionScope;
	readonly runtime: ActionRuntime;
	/** The tree the Actions refresh and collapse; null while there is none. */
	readonly tree: ExplorerTree | null;
}

/** The Explorer header's buttons, each running its Action as a press. */
interface FileExplorerCommands {
	readonly collapseFolders: () => void;
	readonly refresh: () => void;
}

/**
 * The Explorer's title Actions, Refresh Explorer and Collapse Folders in Explorer. They register in
 * the Explorer's own scope while it shows, so Action search finds them only then.
 */
export function useFileExplorerActions({
	parentScope,
	runtime,
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
		const execution = {
			concurrency: 'parallel',
			concurrencyKey: () => 'file-explorer',
			mode: 'foreground',
		} as const;
		const registrations = [
			typedRuntime.register(
				scope,
				defineAction({
					category: VIEW_CATEGORY,
					execution,
					// A root that cannot be listed refreshes too, which retries it.
					getAvailability: () => ({
						kind:
							tree?.state.kind === 'tree' || tree?.state.kind === 'unresolved'
								? 'enabled'
								: 'disabled',
					}),
					handler: {
						execute: (context) => {
							context.markCommitted();
							tree?.refresh();
							return { kind: 'completed' };
						},
					},
					// VS Code parity: workbench.files.action.refreshFilesExplorer.
					id: FILE_EXPLORER_ACTION_IDS.refresh,
					input: noActionInput,
					title: fileExplorerMessages.refresh,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category: VIEW_CATEGORY,
					execution,
					getAvailability: () => ({
						kind:
							tree?.state.kind === 'tree' && hasExpandedFolder(tree.state.model)
								? 'enabled'
								: 'disabled',
					}),
					handler: {
						execute: (context) => {
							context.markCommitted();
							if (tree?.state.kind === 'tree') tree.state.model.collapseAll();
							return { kind: 'completed' };
						},
					},
					// VS Code parity: workbench.files.action.collapseExplorerFolders.
					id: FILE_EXPLORER_ACTION_IDS.collapseFolders,
					input: noActionInput,
					title: fileExplorerMessages.collapse,
				}),
			),
		];
		return () => {
			for (const registration of registrations) registration.dispose();
		};
	}, [scope, tree, typedRuntime]);
	return React.useMemo(() => {
		// The handlers cannot fail, and an unavailable Action is a disabled button, so no outcome shows.
		const press = (actionId: FileExplorerActionId): void => {
			typedRuntime.execute(actionId, undefined, { scope, source: 'button' });
		};
		return {
			collapseFolders: () => press(FILE_EXPLORER_ACTION_IDS.collapseFolders),
			refresh: () => press(FILE_EXPLORER_ACTION_IDS.refresh),
		};
	}, [scope, typedRuntime]);
}
