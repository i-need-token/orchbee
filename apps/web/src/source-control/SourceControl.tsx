import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import { sourceControlMessages, useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import type { FileWorkspaceSession } from '../file-session';
import { VirtualTree, type VirtualTreeRow } from '../virtual-tree';
import { ScmChangeRow } from './components/scm-change-row/ScmChangeRow';
import { ScmChangesMenu } from './components/scm-changes-menu/ScmChangesMenu';
import { ScmProgress } from './components/scm-progress/ScmProgress';
import { ScmRepositoryRow } from './components/scm-repository-row/ScmRepositoryRow';
import { ScmSash } from './components/scm-sash/ScmSash';
import { ScmSectionHeader } from './components/scm-section-header/ScmSectionHeader';
import { ScmWelcome } from './components/scm-welcome/ScmWelcome';
import { SCM_ROW_HEIGHT, SCM_VISIBLE_REPOSITORIES } from './constants/scm-rows';
import { useGitClientFocus } from './hooks/useGitClientFocus';
import { useScmSections } from './hooks/useScmSections';
import { useSourceControlActions } from './hooks/useSourceControlActions';
import { useSourceControlModel } from './hooks/useSourceControlModel';
import { scmCheckoutItem, scmRepositoryNames, scmSyncItem } from './model/scm-repository-row';
import type { ScmTreeItem } from './model/scm-tree';
import type { SourceControlState } from './model/SourceControlModel';
import type { SourceControlViewStateStorage } from './model/source-control-view-state-storage';
import { scmWelcomeItems } from './model/scmWelcomeItems';
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
	/** Where VS Code's Git pickers ask, such as which parent repository to open. */
	readonly quickInput: SourceControlQuickInput;
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
	quickInput,
	session,
	storage,
}: SourceControlProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const rootReference = React.useRef<HTMLDivElement>(null);
	const dragOrigin = React.useRef(0);
	const { model, state } = useSourceControlModel({ session, storage });
	useGitClientFocus(session?.git ?? null);
	const commands = useSourceControlActions({
		model,
		notifications,
		parentScope: actionScope,
		quickInput,
		runtime: actionRuntime,
		session,
	});
	const { drag, dragging, sections, toggleChanges, toggleRepositories } = useScmSections(storage);
	const renderRepository = useRepositoryRenderer(state);
	const renderRepositoryRow = React.useMemo(
		() =>
			(row: VirtualTreeRow<string, string>): React.ReactNode =>
				renderRepository(row.element),
		[renderRepository],
	);
	const mode = state?.view.mode ?? 'list';
	const separator = state?.rules?.separator ?? '/';
	const statusText = model?.statusText;
	const renderChangeRow = React.useMemo(
		() =>
			(row: VirtualTreeRow<string, ScmTreeItem>): React.ReactNode => {
				const item = row.element;
				switch (item.kind) {
					case 'repository':
						return renderRepository(item.repository);
					case 'workspace':
						return null;
					default:
						return statusText === undefined ? null : (
							<ScmChangeRow item={item} mode={mode} separator={separator} statusText={statusText} />
						);
				}
			},
		[mode, renderRepository, separator, statusText],
	);
	if (model === null || state === null) {
		return <div className={styles['root']} data-slot='source-control' />;
	}
	const busy = state.repositories.some(
		(repository) => repository.summary.busy || repository.loading,
	);
	const welcome = scmWelcomeItems({
		closed: state.closed,
		displayed: state.repositories.length,
		root: state.root,
		summary: state.workspace.summary,
	});
	if (welcome.length > 0) {
		return (
			<div className={styles['root']} data-slot='source-control'>
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
		<div className={styles['root']} data-slot='source-control' ref={rootReference}>
			{busy ? <ScmProgress /> : null}
			<section
				className={styles['section']}
				data-collapsed={sections.repositoriesCollapsed ? '' : undefined}
				data-fill={sections.changesCollapsed ? '' : undefined}
				data-slot='scm-section'
			>
				<ScmSectionHeader
					collapsed={sections.repositoriesCollapsed}
					title={intl.formatMessage(sourceControlMessages.repositories)}
					onToggle={toggleRepositories}
				/>
				{sections.repositoriesCollapsed ? null : (
					<div
						className={styles['body']}
						style={{ '--ob-comp-source-control-repositories-height': `${repositoriesHeight}px` }}
					>
						<VirtualTree
							label={intl.formatMessage(sourceControlMessages.repositories)}
							maxStickyRows={0}
							model={model.repositories}
							renderRow={renderRepositoryRow}
							rowHeight={SCM_ROW_HEIGHT}
						/>
					</div>
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
			>
				<ScmSectionHeader
					collapsed={sections.changesCollapsed}
					title={intl.formatMessage(sourceControlMessages.changes)}
					onToggle={toggleChanges}
				>
					<ScmChangesMenu viewAndSort={commands.viewAndSort} />
				</ScmSectionHeader>
				{sections.changesCollapsed ? null : (
					<div className={styles['body']}>
						<VirtualTree
							label={intl.formatMessage(sourceControlMessages.tree)}
							model={model.changes}
							renderRow={renderChangeRow}
							rowHeight={SCM_ROW_HEIGHT}
						/>
					</div>
				)}
			</section>
		</div>
	);
}
