import {
	createTypedActionRuntime,
	defineAction,
	noActionInput,
	type ActionAvailability,
	type ActionResult,
	type ActionRuntime,
	type ActionScope,
	type ActionSignature,
} from '@orchbee/actions';
import {
	sourceControlMessages,
	useOrchbeeIntl,
	workbenchMessages,
	type MessageDescriptor,
} from '@orchbee/i18n';
import * as React from 'react';

import type { FileError, FileResult, FileWorkspaceSession } from '../../file-session';
import type { SourceControlModel } from '../model/SourceControlModel';
import type { ScmViewMode, ScmViewSortKey } from '../model/scm-view-state';
import type {
	SourceControlNotifications,
	SourceControlPickEntry,
	SourceControlQuickInput,
} from '../types/source-control-ports';
import { gitFailureMessage } from '../utils/gitFailureMessage';
import { gitRequestId } from '../utils/gitRequestId';

const SOURCE_CONTROL_ACTION_IDS = {
	initializeRepository: 'source-control.repository.initialize',
	manageUnsafeRepositories: 'source-control.unsafe-repositories.manage',
	openParentRepositories: 'source-control.parent-repositories.open',
	reopenClosedRepositories: 'source-control.closed-repositories.reopen',
	sortByName: 'source-control.changes.sort-by-name',
	sortByPath: 'source-control.changes.sort-by-path',
	sortByStatus: 'source-control.changes.sort-by-status',
	viewAsList: 'source-control.changes.view-as-list',
	viewAsTree: 'source-control.changes.view-as-tree',
} as const;

type SourceControlActionId =
	(typeof SOURCE_CONTROL_ACTION_IDS)[keyof typeof SOURCE_CONTROL_ACTION_IDS];

type SourceControlActionCatalog = { readonly [Id in SourceControlActionId]: ActionSignature<void> };

const GIT_CATEGORY = { id: 'git', title: sourceControlMessages.actionCategory };
const VIEW_CATEGORY = { id: 'view', title: workbenchMessages.actionCategoryView };

const VIEW_MODE_ACTIONS = [
	{
		id: SOURCE_CONTROL_ACTION_IDS.viewAsList,
		mode: 'list',
		title: sourceControlMessages.viewAsList,
	},
	{
		id: SOURCE_CONTROL_ACTION_IDS.viewAsTree,
		mode: 'tree',
		title: sourceControlMessages.viewAsTree,
	},
] as const satisfies readonly {
	readonly id: SourceControlActionId;
	readonly mode: ScmViewMode;
	readonly title: MessageDescriptor;
}[];

const SORT_ACTIONS = [
	{
		id: SOURCE_CONTROL_ACTION_IDS.sortByName,
		sortKey: 'name',
		title: sourceControlMessages.sortByName,
	},
	{
		id: SOURCE_CONTROL_ACTION_IDS.sortByPath,
		sortKey: 'path',
		title: sourceControlMessages.sortByPath,
	},
	{
		id: SOURCE_CONTROL_ACTION_IDS.sortByStatus,
		sortKey: 'status',
		title: sourceControlMessages.sortByStatus,
	},
] as const satisfies readonly {
	readonly id: SourceControlActionId;
	readonly sortKey: ScmViewSortKey;
	readonly title: MessageDescriptor;
}[];

/** One menu row as the View & Sort submenu draws it. */
export interface SourceControlMenuCommand {
	readonly checked: boolean;
	readonly disabled: boolean;
	readonly id: string;
	readonly label: string;
	readonly onSelect: () => void;
}

interface UseSourceControlActionsOptions {
	readonly model: SourceControlModel | null;
	readonly notifications: SourceControlNotifications;
	/** The scope Source Control's own scope nests in. */
	readonly parentScope: ActionScope;
	readonly quickInput: SourceControlQuickInput;
	readonly runtime: ActionRuntime;
	readonly session: FileWorkspaceSession | null;
}

/** What the view's controls run: the welcome's buttons and the View & Sort submenu's rows. */
export interface SourceControlCommands {
	readonly initializeRepository: () => void;
	readonly manageUnsafeRepositories: () => void;
	readonly openParentRepositories: () => void;
	readonly reopenClosedRepositories: () => void;
	/** The submenu's two groups, View as List or Tree, then the three sorts, resolved now. */
	readonly viewAndSort: () => readonly (readonly SourceControlMenuCommand[])[];
}

const EXECUTION = {
	concurrency: 'reject-duplicate',
	concurrencyKey: () => 'source-control',
	mode: 'foreground',
} as const;

function enabledWhen(condition: boolean, checked?: boolean): ActionAvailability {
	return { checked, kind: condition ? 'enabled' : 'disabled' };
}

/** The last path segment, the label VS Code's repository pickers show. */
function lastSegment(path: string, separator: string): string {
	const trimmed = path.endsWith(separator) ? path.slice(0, -separator.length) : path;
	return trimmed.slice(trimmed.lastIndexOf(separator) + 1) || path;
}

/**
 * Source Control's Actions while the view shows: VS Code's View & Sort commands, and the Git
 * commands its welcome offers, Initialize Repository, Open Repositories In Parent Folders, Manage
 * Unsafe Repositories and Reopen Closed Repositories. A Git failure shows as an error card in VS
 * Code's words.
 */
export function useSourceControlActions({
	model,
	notifications,
	parentScope,
	quickInput,
	runtime,
	session,
}: UseSourceControlActionsOptions): SourceControlCommands {
	const intl = useOrchbeeIntl();
	const scope = React.useMemo(
		() => runtime.createScope('source-control', parentScope),
		[parentScope, runtime],
	);
	const typedRuntime = React.useMemo(
		() => createTypedActionRuntime<SourceControlActionCatalog>(runtime),
		[runtime],
	);
	React.useEffect(() => {
		const report = (error: FileError): void => {
			if (error.code === 'cancelled') return;
			void notifications.notify({ message: gitFailureMessage(intl, error), severity: 'error' });
		};
		const settle = (result: FileResult<unknown>): ActionResult<void> => {
			if (result.kind === 'ok') return { kind: 'completed' };
			report(result.error);
			return result.error.code === 'cancelled'
				? { kind: 'cancelled' }
				: { failure: { code: result.error.code }, kind: 'failed' };
		};
		/**
		 * VS Code's repository picker: each repository by name with its path, sorted, and All
		 * Repositories after them when there are several. Resolves to the chosen paths, or null.
		 */
		const pickRepositories = async (
			repositories: readonly string[],
			title: string,
			placeholder: string,
		): Promise<readonly string[] | null> => {
			if (model === null) return null;
			const separator = model.state.rules?.separator ?? '/';
			const sorted = repositories
				.map((path) => ({ label: lastSegment(path, separator), path }))
				.sort((one, other) => model.compareNames(one.label, other.label));
			const entries: SourceControlPickEntry<readonly string[]>[] = sorted.map(
				({ label, path }) => ({
					description: path,
					id: path,
					kind: 'item',
					label,
					value: [path],
				}),
			);
			if (sorted.length > 1) {
				entries.push(
					{ kind: 'separator' },
					{
						id: '',
						kind: 'item',
						label: intl.formatMessage(sourceControlMessages.repositoryPickerAll),
						value: sorted.map(({ path }) => path),
					},
				);
			}
			const outcome = await quickInput.pick({ entries, placeholder, title });
			return outcome.kind === 'accepted' ? outcome.value : null;
		};
		const registrations = [
			...VIEW_MODE_ACTIONS.map(({ id, mode, title }) =>
				typedRuntime.register(
					scope,
					defineAction({
						category: VIEW_CATEGORY,
						execution: EXECUTION,
						getAvailability: () => enabledWhen(model !== null, model?.state.view.mode === mode),
						handler: {
							execute: (context) => {
								if (model === null) return { kind: 'unsupported' };
								context.markCommitted();
								model.setViewMode(mode);
								return { kind: 'completed' };
							},
						},
						// VS Code parity: workbench.scm.action.setListViewMode, setTreeViewMode.
						id,
						input: noActionInput,
						title,
					}),
				),
			),
			...SORT_ACTIONS.map(({ id, sortKey, title }) =>
				typedRuntime.register(
					scope,
					defineAction({
						category: VIEW_CATEGORY,
						execution: EXECUTION,
						// A tree sorts by name, folders first, so the list's sorts wait for list view.
						getAvailability: () =>
							enabledWhen(model?.state.view.mode === 'list', model?.state.view.sortKey === sortKey),
						handler: {
							execute: (context) => {
								if (model === null) return { kind: 'unsupported' };
								context.markCommitted();
								model.setSortKey(sortKey);
								return { kind: 'completed' };
							},
						},
						// VS Code parity: workbench.scm.action.setSortKey.{name,path,status}.
						id,
						input: noActionInput,
						title,
					}),
				),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category: GIT_CATEGORY,
					execution: EXECUTION,
					getAvailability: () => enabledWhen(model?.state.root?.state === 'present'),
					handler: {
						execute: async (context): Promise<ActionResult<void>> => {
							const root = model?.state.root;
							if (session === null || root?.state !== 'present') return { kind: 'unsupported' };
							context.markCommitted();
							// VS Code's `git.init` from the welcome initializes the open folder itself.
							return settle(
								await session.git.run(
									gitRequestId(),
									{ op: 'init', path: root.path },
									context.signal,
								),
							);
						},
					},
					// VS Code parity: git.init.
					id: SOURCE_CONTROL_ACTION_IDS.initializeRepository,
					input: noActionInput,
					title: sourceControlMessages.welcomeInitialize,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category: GIT_CATEGORY,
					execution: EXECUTION,
					getAvailability: () =>
						enabledWhen((model?.state.workspace.summary?.candidates.length ?? 0) > 0),
					handler: {
						execute: async (context): Promise<ActionResult<void>> => {
							const candidates = model?.state.workspace.summary?.candidates ?? [];
							if (session === null || candidates.length === 0) return { kind: 'unsupported' };
							const chosen = await pickRepositories(
								candidates,
								intl.formatMessage(sourceControlMessages.parentRepositoriesTitle),
								intl.formatMessage(sourceControlMessages.parentRepositoriesPlaceholder),
							);
							if (chosen === null) return { kind: 'cancelled' };
							context.markCommitted();
							for (const repository of chosen) {
								const result = settle(
									await session.grants.consentRepository(repository, context.signal),
								);
								if (result.kind !== 'completed') return result;
							}
							return { kind: 'completed' };
						},
					},
					// VS Code parity: git.openRepositoriesInParentFolders.
					id: SOURCE_CONTROL_ACTION_IDS.openParentRepositories,
					input: noActionInput,
					title: sourceControlMessages.parentRepositoriesTitle,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category: GIT_CATEGORY,
					execution: EXECUTION,
					getAvailability: () =>
						enabledWhen((model?.state.workspace.summary?.unsafe.length ?? 0) > 0),
					handler: {
						execute: async (context): Promise<ActionResult<void>> => {
							const unsafe = model?.state.workspace.summary?.unsafe ?? [];
							if (session === null || unsafe.length === 0) return { kind: 'unsupported' };
							const chosen = await pickRepositories(
								unsafe,
								intl.formatMessage(sourceControlMessages.unsafeRepositoriesTitle),
								intl.formatMessage(sourceControlMessages.unsafeRepositoriesPlaceholder),
							);
							if (chosen === null) return { kind: 'cancelled' };
							context.markCommitted();
							// Marked safe in the global Git configuration, discovery opens each one.
							for (const path of chosen) {
								const result = settle(
									await session.git.run(gitRequestId(), { op: 'mark-safe', path }, context.signal),
								);
								if (result.kind !== 'completed') return result;
							}
							return { kind: 'completed' };
						},
					},
					// VS Code parity: git.manageUnsafeRepositories.
					id: SOURCE_CONTROL_ACTION_IDS.manageUnsafeRepositories,
					input: noActionInput,
					title: sourceControlMessages.unsafeRepositoriesTitle,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category: GIT_CATEGORY,
					execution: EXECUTION,
					getAvailability: () => enabledWhen((model?.state.closed ?? 0) > 0),
					handler: {
						execute: async (context): Promise<ActionResult<void>> => {
							const closed = model?.closedRepositories ?? [];
							if (model === null || closed.length === 0) return { kind: 'unsupported' };
							const chosen = await pickRepositories(
								closed,
								intl.formatMessage(sourceControlMessages.closedRepositoriesTitle),
								intl.formatMessage(sourceControlMessages.closedRepositoriesPlaceholder),
							);
							if (chosen === null) return { kind: 'cancelled' };
							context.markCommitted();
							model.reopen(chosen);
							return { kind: 'completed' };
						},
					},
					// VS Code parity: git.reopenClosedRepositories.
					id: SOURCE_CONTROL_ACTION_IDS.reopenClosedRepositories,
					input: noActionInput,
					title: sourceControlMessages.closedRepositoriesTitle,
				}),
			),
		];
		return () => {
			for (const registration of registrations) registration.dispose();
		};
	}, [intl, model, notifications, quickInput, scope, session, typedRuntime]);
	return React.useMemo(() => {
		const run = (actionId: SourceControlActionId, source: 'button' | 'menu'): void => {
			typedRuntime.execute(actionId, undefined, { scope, source });
		};
		const menuCommand = (
			id: SourceControlActionId,
			title: MessageDescriptor,
		): SourceControlMenuCommand => {
			const availability = typedRuntime.resolveAction(id, undefined, { scope, source: 'menu' })
				?.availability ?? { kind: 'hidden' };
			return {
				checked: availability.kind !== 'hidden' && availability.checked === true,
				disabled: availability.kind !== 'enabled',
				id,
				label: intl.formatMessage(title),
				onSelect: () => run(id, 'menu'),
			};
		};
		return {
			initializeRepository: () => run(SOURCE_CONTROL_ACTION_IDS.initializeRepository, 'button'),
			manageUnsafeRepositories: () =>
				run(SOURCE_CONTROL_ACTION_IDS.manageUnsafeRepositories, 'button'),
			openParentRepositories: () => run(SOURCE_CONTROL_ACTION_IDS.openParentRepositories, 'button'),
			reopenClosedRepositories: () =>
				run(SOURCE_CONTROL_ACTION_IDS.reopenClosedRepositories, 'button'),
			viewAndSort: () => [
				VIEW_MODE_ACTIONS.map(({ id, title }) => menuCommand(id, title)),
				SORT_ACTIONS.map(({ id, title }) => menuCommand(id, title)),
			],
		};
	}, [intl, scope, typedRuntime]);
}
