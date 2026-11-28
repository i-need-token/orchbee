import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import { fileExplorerMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import type { ReactNode } from 'react';

import type { FileWorkspaceSession } from '../file-session';
import { VirtualTree, type VirtualTreeRow } from '../virtual-tree';
import { ExplorerHeader } from './components/explorer-header/ExplorerHeader';
import { ExplorerRow } from './components/explorer-row/ExplorerRow';
import { ExplorerUnresolvedRow } from './components/explorer-unresolved-row/ExplorerUnresolvedRow';
import { EXPLORER_ROW_HEIGHT } from './constants/explorer-row-height';
import { useExplorerTree } from './hooks/useExplorerTree';
import { useFileExplorerActions } from './hooks/useFileExplorerActions';
import { useHasExpandedFolder } from './hooks/useHasExpandedFolder';
import type { ExplorerItem } from './model/explorer-item';
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
	/** The active Workspace's file session; null while there is none to show. */
	readonly session: FileWorkspaceSession | null;
}

function renderRow(row: VirtualTreeRow<string, ExplorerItem>): ReactNode {
	return <ExplorerRow item={row.element} />;
}

/**
 * The File Bar's Explorer: VS Code's Folders view over the active Workspace's current directory,
 * with its title Actions, and the one sentence VS Code's welcome view becomes when there is no
 * directory.
 */
export function FileExplorer({
	actionRuntime,
	actionScope,
	notifications,
	platform,
	session,
}: FileExplorerProps): ReactNode {
	const intl = useOrchbeeIntl();
	const { state, tree } = useExplorerTree({ notifications, session });
	const canCollapse = useHasExpandedFolder(state.kind === 'tree' ? state.model : null);
	const commands = useFileExplorerActions({
		parentScope: actionScope,
		runtime: actionRuntime,
		tree,
	});
	switch (state.kind) {
		case 'loading':
			return <div className={styles['root']} data-slot='file-explorer' />;
		case 'none':
			return (
				<div className={styles['root']} data-slot='file-explorer'>
					<p className={styles['empty']}>{intl.formatMessage(fileExplorerMessages.noDirectory)}</p>
				</div>
			);
		case 'tree':
		case 'unresolved':
			return (
				<div className={styles['root']} data-slot='file-explorer'>
					<ExplorerHeader
						canCollapse={canCollapse}
						name={state.name}
						path={state.root}
						onCollapse={commands.collapseFolders}
						onRefresh={commands.refresh}
					/>
					{state.kind === 'tree' ? (
						<div className={styles['tree']}>
							<VirtualTree
								label={intl.formatMessage(fileExplorerMessages.tree)}
								model={state.model}
								platform={platform}
								renderRow={renderRow}
								rowHeight={EXPLORER_ROW_HEIGHT}
							/>
						</div>
					) : (
						<ExplorerUnresolvedRow error={state.error} name={state.name} />
					)}
				</div>
			);
	}
}
