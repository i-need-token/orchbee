import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import { sourceControlMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { ContextMenu, ContextMenuTrigger } from '@orchbee/ui/components';
import * as React from 'react';
import type { Ref } from 'react';

import type { FileWorkspaceSession } from '../file-session';
import { VirtualTree, type VirtualTreeRow } from '../virtual-tree';
import { ScmChangesMenu } from './components/scm-changes-menu/ScmChangesMenu';
import { ScmContextMenuContent } from './components/scm-context-menu-content/ScmContextMenuContent';
import { ScmProgress } from './components/scm-progress/ScmProgress';
import { ScmRepositoryRow } from './components/scm-repository-row/ScmRepositoryRow';
import { ScmSash } from './components/scm-sash/ScmSash';
import { ScmSectionHeader } from './components/scm-section-header/ScmSectionHeader';
import { ScmWelcome } from './components/scm-welcome/ScmWelcome';
import { SCM_ROW_HEIGHT, SCM_VISIBLE_REPOSITORIES } from './constants/scm-rows';
import { useGitClientFocus } from './hooks/useGitClientFocus';
import { useScmChangeRowRenderer } from './hooks/useScmChangeRowRenderer';
import { useScmMenuGroups } from './hooks/useScmMenuGroups';
import { useScmRowActions } from './hooks/useScmRowActions';
import { useScmSections } from './hooks/useScmSections';
import { useScmTreeMenu } from './hooks/useScmTreeMenu';
import { useSourceControlActions } from './hooks/useSourceControlActions';
import { useSourceControlFocus } from './hooks/useSourceControlFocus';
import { useSourceControlModel } from './hooks/useSourceControlModel';
import { scmCheckoutItem, scmRepositoryNames, scmSyncItem } from './model/scm-repository-row';
import type { SourceControlState } from './model/SourceControlModel';
import type { SourceControlViewStateStorage } from './model/source-control-view-state-storage';
import { scmWelcomeItems } from './model/scmWelcomeItems';
import type { SourceControlFocus } from './types/source-control-focus';
import type {
	SourceControlNotifications,
	SourceControlQuickInput,
} from './types/source-control-ports';
import styles from './SourceControl.module.css';

interface SourceControlProps {
	readonly actionRuntime: ActionRuntime;
	/** The scope Source Control's own Actions nest in while it shows. */
	readonly actionScope: ActionScope;
	/** Where Git failures and VS Code's other Git messages are told. */
	readonly notifications: SourceControlNotifications;
	/** Which platform's keys and clicks the trees answer to. */
	readonly platform: KeyboardShortcutPlatform;
	/** Where VS Code's Git pickers ask, such as which parent repository to open. */
	readonly quickInput: SourceControlQuickInput;
	/** Receives what moves focus into Source Control. */
	readonly ref?: Ref<SourceControlFocus> | undefined;
	/** The active Workspace's file session; null while there is none to show. */
	readonly session: FileWorkspaceSession | null;
	/** Where the view mode, sort and sections are saved between runs; without it they are not. */
	readonly storage?: SourceControlViewStateStorage | undefined;
}

/** The rows a section's header and the Changes list keep at least, so neither closes by a drag. */
const SECTIONS_CHROME = 3 * SCM_ROW_HEIGHT;

/** Draws a repository row from the state of the moment the tree draws it. */
function useRepositoryRenderer(
	state: SourceControlState | null,
): (repository: string) => React.ReactNode {
	return React.useMemo(() => {
		const repositories = state?.repositories ?? [];
		const names = scmRepositoryNames(
			repositories.map(({ summary }) => summary.repository),
			state?.rules?.separator ?? '/',
			state?.root?.state === 'present' ? state.root.path : null,
		);
		return (repository: string): React.ReactNode => {
			const summary = repositories.find(
				(entry) => entry.summary.repository === repository,
			)?.summary;
			if (summary === undefined || state === null) return null;
			const name = names.get(repository);
			return (
				<ScmRepositoryRow
					checkout={scmCheckoutItem(summary, 'none')}
					description={name?.description ?? null}
					name={name?.name ?? repository}
					path={repository}
					sync={scmSyncItem(summary, state.remotes.get(repository) ?? [], 'none')}
					onSync={null}
				/>
			);
		};
	}, [state]);
}

/**
 * The File Bar's Source Control: VS Code's Git view over the active Workspace. Its Repositories
 * section lists every repository with its branch and sync state, even a single one; its Changes
 * section lists each repository's groups of changed files, as a list or a tree. While no repository
 * shows, VS Code's welcome says why.
 */
export function SourceControl({
	actionRuntime,
	actionScope,
	notifications,
	platform,
	quickInput,
	ref,
	session,
	storage,
}: SourceControlProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const rootReference = React.useRef<HTMLDivElement>(null);
	const repositoriesTree = React.useRef<HTMLDivElement>(null);
	const changesTree = React.useRef<HTMLDivElement>(null);
	const dragOrigin = React.useRef(0);
	const { model, state } = useSourceControlModel({ session, storage });
	useGitClientFocus(session?.git ?? null);
	const scope = React.useMemo(
		() => actionRuntime.createScope('source-control', actionScope),
		[actionRuntime, actionScope],
	);
	const commands = useSourceControlActions({
		model,
		notifications,
		quickInput,
		runtime: actionRuntime,
		scope,
		session,
	});
	const rowCommands = useScmRowActions({
		model,
		notifications,
		runtime: actionRuntime,
		scope,
		session,
	});
	const menus = useScmMenuGroups({ commands, model, rowCommands });
	const repositoriesMenu = useScmTreeMenu();
	const changesMenu = useScmTreeMenu();
	const { drag, dragging, sections, toggleChanges, toggleRepositories } = useScmSections(storage);
	const renderRepository = useRepositoryRenderer(state);
	const renderRepositoryRow = React.useMemo(
		() =>
			(row: VirtualTreeRow<string, string>): React.ReactNode =>
				renderRepository(row.element),
		[renderRepository],
	);
	const renderChangeRow = useScmChangeRowRenderer({ model, renderRepository, rowCommands, state });
	const welcome =
		state === null
			? []
			: scmWelcomeItems({
					closed: state.closed,
					displayed: state.repositories.length,
					root: state.root,
					summary: state.workspace.summary,
				});
	const focusTracking = useSourceControlFocus({
		changesTree,
		reference: ref,
		repositoriesTree,
		rootReference,
		treeShown: state !== null && welcome.length === 0,
	});
	if (model === null || state === null) {
		return (
			<div
				className={styles['root']}
				data-slot='source-control'
				ref={rootReference}
				tabIndex={-1}
			/>
		);
	}
	const busy = state.repositories.some(
		(repository) => repository.summary.busy || repository.loading,
	);
	if (welcome.length > 0) {
		return (
			<div className={styles['root']} data-slot='source-control' ref={rootReference} tabIndex={-1}>
				{busy ? <ScmProgress /> : null}
				<ScmWelcome
					items={welcome}
					os={state.rules?.os ?? null}
					onInitialize={commands.initializeRepository}
					onManageUnsafe={commands.manageUnsafeRepositories}
					onOpenParent={commands.openParentRepositories}
					onReopenClosed={commands.reopenClosedRepositories}
				/>
			</div>
		);
	}
	const fitted = Math.min(state.repositories.length, SCM_VISIBLE_REPOSITORIES) * SCM_ROW_HEIGHT;
	const repositoriesHeight = dragging ?? sections.repositoriesSize ?? fitted;
	const bothExpanded = !sections.repositoriesCollapsed && !sections.changesCollapsed;
	return (
		<div className={styles['root']} data-slot='source-control' ref={rootReference} tabIndex={-1}>
			{busy ? <ScmProgress /> : null}
			<section
				className={styles['section']}
				data-collapsed={sections.repositoriesCollapsed ? '' : undefined}
				data-fill={sections.changesCollapsed ? '' : undefined}
				data-slot='scm-section'
				onFocus={focusTracking.onRepositoriesFocus}
			>
				<ScmSectionHeader
					collapsed={sections.repositoriesCollapsed}
					title={intl.formatMessage(sourceControlMessages.repositories)}
					onToggle={toggleRepositories}
				/>
				{sections.repositoriesCollapsed ? null : (
					<ContextMenu>
						<ContextMenuTrigger
							className={styles['body']}
							style={{ '--ob-comp-source-control-repositories-height': `${repositoriesHeight}px` }}
							onContextMenu={repositoriesMenu.onTriggerContextMenu}
						>
							<VirtualTree
								label={intl.formatMessage(sourceControlMessages.repositories)}
								maxStickyRows={0}
								model={model.repositories}
								platform={platform}
								renderRow={renderRepositoryRow}
								rowHeight={SCM_ROW_HEIGHT}
								treeReference={repositoriesTree}
								onContextMenu={({ row }) => repositoriesMenu.show(menus.repositories(row))}
							/>
						</ContextMenuTrigger>
						{repositoriesMenu.groups.length === 0 ? null : (
							<ScmContextMenuContent
								finalFocus={repositoriesTree}
								groups={repositoriesMenu.groups}
							/>
						)}
					</ContextMenu>
				)}
			</section>
			{bothExpanded ? (
				<ScmSash
					onDrag={(offset) => {
						const available = (rootReference.current?.clientHeight ?? 0) - SECTIONS_CHROME;
						drag.move(Math.max(SCM_ROW_HEIGHT, Math.min(dragOrigin.current + offset, available)));
					}}
					onDragEnd={drag.end}
					onDragStart={() => {
						dragOrigin.current = repositoriesHeight;
						drag.start();
					}}
				/>
			) : null}
			<section
				className={styles['section']}
				data-collapsed={sections.changesCollapsed ? '' : undefined}
				data-fill={sections.changesCollapsed ? undefined : ''}
				data-slot='scm-section'
				onFocus={focusTracking.onChangesFocus}
			>
				<ScmSectionHeader
					collapsed={sections.changesCollapsed}
					title={intl.formatMessage(sourceControlMessages.changes)}
					onToggle={toggleChanges}
				>
					<ScmChangesMenu viewAndSort={commands.viewAndSort} />
				</ScmSectionHeader>
				{sections.changesCollapsed ? null : (
					<ContextMenu>
						<ContextMenuTrigger
							className={styles['body']}
							onContextMenu={changesMenu.onTriggerContextMenu}
						>
							<VirtualTree
								label={intl.formatMessage(sourceControlMessages.tree)}
								model={model.changes}
								platform={platform}
								renderRow={renderChangeRow}
								rowHeight={SCM_ROW_HEIGHT}
								treeReference={changesTree}
								onContextMenu={({ row }) => changesMenu.show(menus.changes(row))}
							/>
						</ContextMenuTrigger>
						{changesMenu.groups.length === 0 ? null : (
							<ScmContextMenuContent finalFocus={changesTree} groups={changesMenu.groups} />
						)}
					</ContextMenu>
				)}
			</section>
		</div>
	);
}
