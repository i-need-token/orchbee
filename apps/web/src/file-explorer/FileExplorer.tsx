import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import { fileExplorerMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { useRef, type ReactNode, type Ref } from 'react';

import type { FileWorkspaceSession } from '../file-session';
import { VirtualTree, type VirtualTreeRow } from '../virtual-tree';
import { ExplorerHeader } from './components/explorer-header/ExplorerHeader';
import { ExplorerProgress } from './components/explorer-progress/ExplorerProgress';
import { ExplorerRow } from './components/explorer-row/ExplorerRow';
import { ExplorerUnresolvedRow } from './components/explorer-unresolved-row/ExplorerUnresolvedRow';
import { EXPLORER_ROW_HEIGHT } from './constants/explorer-row-height';
import { useExplorerFocus } from './hooks/useExplorerFocus';
import { useExplorerTree } from './hooks/useExplorerTree';
import { useFileExplorerActions } from './hooks/useFileExplorerActions';
import { useHasExpandedFolder } from './hooks/useHasExpandedFolder';
import type { ExplorerItem } from './model/explorer-item';
import type { ExplorerExpansionStorage } from './model/explorer-expansion-storage';
import type { FileExplorerFocus } from './types/file-explorer-focus';
import type { FileExplorerNotifications } from './types/file-explorer-notifications';
import styles from './FileExplorer.module.css';

export interface FileExplorerProps {
	readonly actionRuntime: ActionRuntime;
	/** The scope the Explorer's own Actions nest in while it shows. */
	readonly actionScope: ActionScope;
	/** Where a folder that could not be listed is reported. */
	readonly notifications: FileExplorerNotifications;
	/** Which platform's keys the tree answers to. */
	readonly platform: KeyboardShortcutPlatform;
	/** Receives what moves focus into the Explorer. */
	readonly ref?: Ref<FileExplorerFocus> | undefined;
	/** The active Workspace's file session; null while there is none to show. */
	readonly session: FileWorkspaceSession | null;
	/** Where each Workspace's expanded folders are saved between runs; without it they are not. */
	readonly storage?: ExplorerExpansionStorage | undefined;
}

/**
 * The tree's widget in the shortcut catalog: bindings declared with the scope `widget:explorer`
 * apply while it has focus, and no wider binding takes the tree's own keys.
 */
const EXPLORER_WIDGET = 'explorer';

function renderRow(row: VirtualTreeRow<string, ExplorerItem>): ReactNode {
	return <ExplorerRow item={row.element} />;
}

// Typing moves to the next entry whose name starts with what was typed, as in VS Code's Explorer.
function navigationLabel(row: VirtualTreeRow<string, ExplorerItem>): string {
	return row.element.name;
}

/**
 * The File Bar's Explorer: VS Code's Folders view over the active Workspace's current directory,
 * with its title Actions, and the one sentence VS Code's welcome view becomes when there is no
 * directory. Its tree answers keys, clicks and type navigation as VS Code's Explorer does.
 */
export function FileExplorer({
	actionRuntime,
	actionScope,
	notifications,
	platform,
	ref,
	session,
	storage,
}: FileExplorerProps): ReactNode {
	const intl = useOrchbeeIntl();
	const rootReference = useRef<HTMLDivElement>(null);
	const treeReference = useRef<HTMLDivElement>(null);
	const { progress, state, tree } = useExplorerTree({ notifications, session, storage });
	const canCollapse = useHasExpandedFolder(state.kind === 'tree' ? state.model : null);
	const commands = useFileExplorerActions({
		parentScope: actionScope,
		runtime: actionRuntime,
		tree,
	});
	useExplorerFocus(ref, rootReference, treeReference, state.kind === 'tree');
	return (
		<div className={styles['root']} data-slot='file-explorer' ref={rootReference} tabIndex={-1}>
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
							<VirtualTree
								keyboardNavigationLabel={navigationLabel}
								keyboardScope={{ actionScope: commands.scope, widget: EXPLORER_WIDGET }}
								label={intl.formatMessage(fileExplorerMessages.tree)}
								model={state.model}
								platform={platform}
								renderRow={renderRow}
								rowHeight={EXPLORER_ROW_HEIGHT}
								treeReference={treeReference}
							/>
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
