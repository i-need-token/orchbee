import {
	defineAction,
	noActionInput,
	type ActionAvailability,
	type ActionAvailabilityContext,
	type ActionDisposable,
	type ActionResult,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { fileExplorerMessages, workbenchMessages } from '@orchbee/i18n';

import {
	FILE_EXPLORER_ACTION_IDS,
	type FileExplorerActionCatalog,
} from '../../file-explorer-actions';
import type { FileOverlayOpenFile } from '../../file-overlay';
import { filePathKey, type FileWorkspaceSession } from '../../file-session';
import { canOpenExplorerItem } from '../model/canOpenExplorerItem';
import type { ExplorerItem } from '../model/explorer-item';
import { explorerContext } from '../model/explorerContext';
import { explorerRelativePath } from '../model/explorerRelativePath';
import type { ExplorerTree, ExplorerTreeState } from '../model/ExplorerTree';
import { hasExpandedFolder } from '../model/hasExpandedFolder';
import type { ExplorerClipboard } from '../types/explorer-clipboard';

const VIEW_CATEGORY = { id: 'view', title: workbenchMessages.actionCategoryView };
const ENABLED: ActionAvailability = { kind: 'enabled' };
const HIDDEN: ActionAvailability = { kind: 'hidden' };
const TREE_CHANGED: ActionResult<void> = { code: 'explorer-tree-changed', kind: 'blocked' };

type ShownTree = Extract<ExplorerTreeState, { readonly kind: 'tree' }>;

interface FileExplorerActionsOptions {
	/** Where Copy Path and Copy Relative Path write. */
	readonly clipboard: ExplorerClipboard;
	/** Opens a file in the file overlay, where Open to the Side opens the files. */
	readonly openFile: FileOverlayOpenFile;
	/** The Explorer's own scope. */
	readonly scope: ActionScope;
	/** The Workspace whose overlay the Explorer's files open in; null while there is none. */
	readonly session: FileWorkspaceSession | null;
	/** The tree the Actions act on; null while there is none. */
	readonly tree: ExplorerTree | null;
	/** The adapter the Actions register through, which alone runs them. */
	readonly typedRuntime: TypedActionRuntime<FileExplorerActionCatalog>;
}

/**
 * The Explorer's Actions: its title Actions, Refresh Explorer and Collapse Folders in Explorer, and
 * its rows' commands, which act on what VS Code's Explorer commands act on, the selected rows or the
 * focused one. The rows' commands stay out of Action search, as VS Code's command palette offers
 * them only for the active editor's file.
 */
export function registerFileExplorerActions({
	clipboard,
	openFile,
	scope,
	session,
	tree,
	typedRuntime,
}: FileExplorerActionsOptions): readonly ActionDisposable[] {
	const execution = {
		concurrency: 'parallel',
		concurrencyKey: () => 'file-explorer',
		mode: 'foreground',
	} as const;
	const shownTree = (): ShownTree | null => (tree?.state.kind === 'tree' ? tree.state : null);
	const rowAvailability = (context: ActionAvailabilityContext): ActionAvailability =>
		context.source === 'command-surface' || shownTree() === null ? HIDDEN : ENABLED;
	const copy = async (
		pathOf: (item: ExplorerItem, shown: ShownTree) => string,
	): Promise<ActionResult<void>> => {
		const shown = shownTree();
		if (shown === null) return TREE_CHANGED;
		const text = explorerContext(shown)
			.map((item) => pathOf(item, shown))
			.join(clipboard.lineBreak);
		try {
			await clipboard.writeText(text);
		} catch {
			return { failure: { code: 'clipboard-write-failed' }, kind: 'failed' };
		}
		return { kind: 'completed' };
	};
	const open = (item: ExplorerItem, shown: ShownTree, workspaceId: string, focus: boolean) =>
		openFile(
			{
				focus,
				preview: false,
				resource: { key: filePathKey(item.path, shown.rules), path: item.path },
				workspaceId,
			},
			{ scope, source: 'programmatic' },
		).completion;
	return [
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution,
				// A root that cannot be listed refreshes too, which retries it, and so does one whose
				// first listing runs, as VS Code's Refresh stays enabled while the title shows.
				getAvailability: () => ({
					kind:
						tree?.state.kind === 'opening' ||
						tree?.state.kind === 'tree' ||
						tree?.state.kind === 'unresolved'
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
				getAvailability: () => {
					const shown = shownTree();
					return {
						kind: shown !== null && hasExpandedFolder(shown.model) ? 'enabled' : 'disabled',
					};
				},
				handler: {
					execute: (context) => {
						context.markCommitted();
						shownTree()?.model.collapseAll();
						return { kind: 'completed' };
					},
				},
				// VS Code parity: workbench.files.action.collapseExplorerFolders.
				id: FILE_EXPLORER_ACTION_IDS.collapseFolders,
				input: noActionInput,
				title: fileExplorerMessages.collapse,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution,
				getAvailability: rowAvailability,
				handler: {
					// The overlay has one group, so the files open pinned there with the keys, the first
					// active and the others after it, as VS Code opens them in the group to the side.
					execute: async (context) => {
						const shown = shownTree();
						if (shown === null || session === null) return TREE_CHANGED;
						const files = explorerContext(shown).filter(canOpenExplorerItem);
						const [first] = files;
						if (first === undefined) return { kind: 'unchanged' };
						context.markCommitted();
						if (files.length > 1) {
							for (const file of files) {
								const result = await open(file, shown, session.workspaceId, false);
								if (result.kind !== 'completed' && result.kind !== 'unchanged') return result;
							}
						}
						return open(first, shown, session.workspaceId, true);
					},
				},
				// VS Code parity: explorer.openToSide.
				id: FILE_EXPLORER_ACTION_IDS.openToSide,
				input: noActionInput,
				title: fileExplorerMessages.openToSide,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution,
				getAvailability: rowAvailability,
				handler: { execute: () => copy((item) => item.path) },
				// VS Code parity: copyFilePath.
				id: FILE_EXPLORER_ACTION_IDS.copyPath,
				input: noActionInput,
				title: fileExplorerMessages.copyPath,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution,
				getAvailability: rowAvailability,
				handler: {
					execute: () =>
						copy((item, shown) => explorerRelativePath(item.path, shown.root, shown.rules)),
				},
				// VS Code parity: copyRelativeFilePath.
				id: FILE_EXPLORER_ACTION_IDS.copyRelativePath,
				input: noActionInput,
				title: fileExplorerMessages.copyRelativePath,
			}),
		),
	];
}
