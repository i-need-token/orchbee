import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { fileExplorerMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { ContextMenu, ContextMenuTrigger } from '@orchbee/ui/components';
import { useMemo, useRef, type KeyboardEvent, type ReactNode, type Ref } from 'react';

import type { FileOverlayOpenFile } from '../file-overlay';
import type { FileResource, FileWorkspaceSession } from '../file-session';
import { VirtualTree, type VirtualTreeRow, type VirtualTreeRowState } from '../virtual-tree';
import { ExplorerContextMenu } from './components/explorer-context-menu/ExplorerContextMenu';
import { ExplorerHeader } from './components/explorer-header/ExplorerHeader';
import { ExplorerProgress } from './components/explorer-progress/ExplorerProgress';
import { ExplorerRow } from './components/explorer-row/ExplorerRow';
import { ExplorerUnresolvedRow } from './components/explorer-unresolved-row/ExplorerUnresolvedRow';
import { EXPLORER_ROW_HEIGHT } from './constants/explorer-row-height';
import { useExplorerActiveFile } from './hooks/useExplorerActiveFile';
import { useExplorerDrag } from './hooks/useExplorerDrag';
import { useExplorerFocus } from './hooks/useExplorerFocus';
import { useExplorerGitDecorations } from './hooks/useExplorerGitDecorations';
import { useExplorerMenu } from './hooks/useExplorerMenu';
import { useExplorerOpen } from './hooks/useExplorerOpen';
import { useExplorerShown } from './hooks/useExplorerShown';
import { useExplorerTree } from './hooks/useExplorerTree';
import { useFileExplorerActions } from './hooks/useFileExplorerActions';
import { useHasExpandedFolder } from './hooks/useHasExpandedFolder';
import type { ExplorerItem } from './model/explorer-item';
import type { ExplorerExpansionStorage } from './model/explorer-expansion-storage';
import type { ExplorerMenuTarget } from './model/explorer-menus';
import type { FileExplorerReportActionOutcome } from './types/explorer-menu';
import type { FileExplorerFocus } from './types/file-explorer-focus';
import type { FileExplorerNotifications } from './types/file-explorer-notifications';
import { buildExplorerMenu } from './utils/buildExplorerMenu';
import styles from './FileExplorer.module.css';

export interface FileExplorerProps {
	readonly actionRuntime: ActionRuntime;
	/** The scope the Explorer's own Actions nest in while it shows. */
	readonly actionScope: ActionScope;
	/** The file the overlay's active tab shows, as VS Code's active editor; null without tabs. */
	readonly activeFile: FileResource | null;
	/** Where a folder that could not be listed is reported. */
	readonly notifications: FileExplorerNotifications;
	/** Opens a file in the file overlay, where the tree opens its files. */
	readonly openFile: FileOverlayOpenFile;
	/** Which platform's keys the tree answers to. */
	readonly platform: KeyboardShortcutPlatform;
	/** Receives what moves focus into the Explorer. */
	readonly ref?: Ref<FileExplorerFocus> | undefined;
	/** Runs a command chosen from the context menu and says its refusal or failure. */
	readonly reportActionOutcome: FileExplorerReportActionOutcome;
	/** Gives the keys back to the current terminal, as Escape does once the tree has nothing to clear. */
	readonly returnToTerminal: () => void;
	/** The active Workspace's file session; null while there is none to show. */
	readonly session: FileWorkspaceSession | null;
	/** The keys that run an Action now, which the context menu shows beside its command. */
	readonly shortcutLabelForAction: (actionId: string) => string | undefined;
	/** Where each Workspace's expanded folders are saved between runs; without it they are not. */
	readonly storage?: ExplorerExpansionStorage | undefined;
}

/**
 * The tree's widget in the shortcut catalog: bindings declared with the scope `widget:explorer`
 * apply while it has focus, and no wider binding takes the tree's own keys.
 */
const EXPLORER_WIDGET = 'explorer';

// Typing moves to the next entry whose name starts with what was typed, as in VS Code's Explorer,
// which names a compact row by its folders' names joined with `/`.
function navigationLabel(row: VirtualTreeRow<string, ExplorerItem>): string {
	return row.segments.map((segment) => segment.element.name).join('/');
}

// Seti draws icons for files and none for folders, so a file's icon takes the place of its twistie
// and lines up with the folders' twisties, as VS Code's `align-icons-and-twisties` sets for such a
// theme. A folder that cannot open has no icon to put there, so it keeps the room and its name stays
// in line with the other folders' names.
function hasFileIcon(row: VirtualTreeRow<string, ExplorerItem>): boolean {
	return !row.element.directory;
}

/**
 * The File Bar's Explorer: VS Code's Folders view over the active Workspace's current directory,
 * with its title Actions, and the one sentence VS Code's welcome view becomes when there is no
 * directory. Its tree answers keys, clicks and type navigation as VS Code's Explorer does, opens
 * files in the file overlay and follows the overlay's active tab, its rows carry VS Code's Git
 * decorations from the status Source Control reads, and they drag as the File Bar's files and folders.
 * Its context menu holds the commands of VS Code's that Orchbee offers, and Escape returns the keys
 * to the current terminal once VS Code's Escape has nothing left to do in the tree.
 */
export function FileExplorer({
	actionRuntime,
	actionScope,
	activeFile,
	notifications,
	openFile,
	platform,
	ref,
	reportActionOutcome,
	returnToTerminal,
	session,
	shortcutLabelForAction,
	storage,
}: FileExplorerProps): ReactNode {
	const intl = useOrchbeeIntl();
	const rootReference = useRef<HTMLDivElement>(null);
	const treeReference = useRef<HTMLDivElement>(null);
	const { progress, state, tree } = useExplorerTree({ notifications, session, storage });
	const decorations = useExplorerGitDecorations(session);
	// Rows follow the decorations themselves, so the renderer changes only when they do.
	const renderRow = useMemo(
		() =>
			(row: VirtualTreeRow<string, ExplorerItem>, rowState: VirtualTreeRowState): ReactNode => (
				<ExplorerRow decorations={decorations} row={row} state={rowState} />
			),
		[decorations],
	);
	const canCollapse = useHasExpandedFolder(state.kind === 'tree' ? state.model : null);
	const dragAndDrop = useExplorerDrag(state.kind === 'tree' ? state.rules : null);
	const commands = useFileExplorerActions({
		openFile,
		parentScope: actionScope,
		runtime: actionRuntime,
		session,
		tree,
	});
	const buildMenu = useMemoizedFn((target: ExplorerMenuTarget) =>
		buildExplorerMenu({
			formatMessage: (descriptor) => intl.formatMessage(descriptor),
			report: reportActionOutcome,
			scope: commands.scope,
			shortcutLabelForAction,
			target,
			typedRuntime: commands.actions,
		}),
	);
	const menu = useExplorerMenu(
		state.kind === 'tree' ? state.model : null,
		treeReference,
		buildMenu,
	);
	const open = useExplorerOpen(
		openFile,
		commands.scope,
		session,
		state.kind === 'tree' ? state.rules : null,
	);
	const shown = useExplorerShown(rootReference);
	useExplorerActiveFile(tree, activeFile, shown);
	// VS Code's Explorer brings its focused row to the middle when the view takes focus.
	const revealFocus = useMemoizedFn(() => {
		if (state.kind !== 'tree') return;
		const { model } = state;
		if (model.focus !== null) model.reveal(model.focus, { relativeTop: 0.5 });
	});
	useExplorerFocus(ref, rootReference, treeReference, state.kind === 'tree', revealFocus);
	// VS Code's Escape closes the tree's find, then clears its selection and focus, and the tree takes
	// the key while it does. Then the key reaches the view, which gives the keys back to the terminal,
	// as the Sidebar does; a control or a menu inside the view keeps its own Escape.
	const onKeyDown = useMemoizedFn((event: KeyboardEvent<HTMLDivElement>) => {
		if (
			event.key !== 'Escape' ||
			event.defaultPrevented ||
			event.nativeEvent.isComposing ||
			event.altKey ||
			event.ctrlKey ||
			event.metaKey ||
			event.shiftKey ||
			(event.target !== treeReference.current && event.target !== event.currentTarget)
		) {
			return;
		}
		event.preventDefault();
		returnToTerminal();
	});
	return (
		<div
			className={styles['root']}
			data-slot='file-explorer'
			onKeyDown={onKeyDown}
			ref={rootReference}
			tabIndex={-1}
		>
			{state.kind === 'loading' && progress ? <ExplorerProgress /> : null}
			{state.kind === 'none' ? (
				<p className={styles['empty']}>{intl.formatMessage(fileExplorerMessages.noDirectory)}</p>
			) : null}
			{state.kind === 'opening' || state.kind === 'tree' || state.kind === 'unresolved' ? (
				<>
					<ExplorerHeader
						canCollapse={canCollapse}
						name={state.name}
						path={state.root}
						onCollapse={commands.collapseFolders}
						onRefresh={commands.refresh}
					/>
					<div className={styles['content']}>
						{progress ? <ExplorerProgress /> : null}
						{state.kind === 'tree' ? (
							<ContextMenu>
								<ContextMenuTrigger
									className={styles['menuTrigger']}
									onContextMenu={menu.onTriggerContextMenu}
								>
									<VirtualTree
										alignIconWithTwistie={hasFileIcon}
										dragAndDrop={dragAndDrop}
										keyboardNavigationLabel={navigationLabel}
										keyboardScope={{ actionScope: commands.scope, widget: EXPLORER_WIDGET }}
										label={intl.formatMessage(fileExplorerMessages.tree)}
										model={state.model}
										onContextMenu={menu.onTreeContextMenu}
										onOpen={open}
										paddingBottom={EXPLORER_ROW_HEIGHT}
										platform={platform}
										renderRow={renderRow}
										rowHeight={EXPLORER_ROW_HEIGHT}
										treeReference={treeReference}
									/>
								</ContextMenuTrigger>
								{menu.groups.length === 0 ? null : (
									<ExplorerContextMenu
										finalFocus={menu.finalFocus}
										groups={menu.groups}
										onChoose={menu.choose}
									/>
								)}
							</ContextMenu>
						) : null}
						{state.kind === 'unresolved' ? (
							<ExplorerUnresolvedRow error={state.error} name={state.name} />
						) : null}
					</div>
				</>
			) : null}
		</div>
	);
}
