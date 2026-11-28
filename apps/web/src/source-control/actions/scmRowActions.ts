import {
	createTypedActionRuntime,
	defineAction,
	type ActionAvailability,
	type ActionCatalogId,
	type ActionConfirmation,
	type ActionDisposable,
	type ActionExecution,
	type ActionExecutionContext,
	type ActionInputFor,
	type ActionInvocationSource,
	type ActionResult,
	type ActionRuntime,
	type ActionScope,
	type ActionSignature,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { sourceControlMessages, type IntlShape, type MessageDescriptor } from '@orchbee/i18n';
import { z } from 'zod';

import type { FileError, FileWorkspaceSession, GitMutationOutput } from '../../file-session';
import {
	isScmUntracked,
	scmDiscardQuestion,
	scmPermanentDeleteQuestion,
	type ScmCurrentChange,
} from '../model/scm-discard';
import type { ScmRowCommand } from '../model/scm-menus';
import { SCM_GROUP_IDS, type ScmGroupId, type ScmResource } from '../model/scm-resource';
import type { ScmChangeTarget, ScmRowTarget } from '../model/scm-row-target';
import type { SourceControlModel } from '../model/SourceControlModel';
import type { SourceControlNotifications } from '../types/source-control-ports';
import { gitFailureNotice } from '../utils/gitFailureNotice';
import { gitRequestId } from '../utils/gitRequestId';
import { SCM_GIT_CATEGORY, SCM_VIEW_CATEGORY } from './scm-action-categories';
import { scmActionInput } from './scmActionInput';

const SCM_ROW_ACTION_IDS = {
	closeOtherRepositories: 'source-control.repository.close-others',
	closeRepository: 'source-control.repository.close',
	collapseAll: 'source-control.group.collapse-all',
	deletePermanently: 'source-control.changes.delete-permanently',
	discard: 'source-control.changes.discard',
	discardAll: 'source-control.group.discard-all',
	stage: 'source-control.changes.stage',
	stageAll: 'source-control.group.stage-all',
	unstage: 'source-control.changes.unstage',
	unstageAll: 'source-control.group.unstage-all',
} as const;

interface ScmChangesInput {
	readonly changes: readonly ScmChangeTarget[];
}

interface ScmGroupInput {
	readonly group: ScmGroupId;
	readonly repository: string;
}

interface ScmRepositoriesInput {
	readonly repositories: readonly string[];
}

interface ScmRowActionCatalog {
	readonly [SCM_ROW_ACTION_IDS.closeOtherRepositories]: ActionSignature<ScmRepositoriesInput>;
	readonly [SCM_ROW_ACTION_IDS.closeRepository]: ActionSignature<ScmRepositoriesInput>;
	readonly [SCM_ROW_ACTION_IDS.collapseAll]: ActionSignature<ScmGroupInput>;
	readonly [SCM_ROW_ACTION_IDS.deletePermanently]: ActionSignature<ScmChangesInput>;
	readonly [SCM_ROW_ACTION_IDS.discard]: ActionSignature<ScmChangesInput>;
	readonly [SCM_ROW_ACTION_IDS.discardAll]: ActionSignature<ScmGroupInput>;
	readonly [SCM_ROW_ACTION_IDS.stage]: ActionSignature<ScmChangesInput>;
	readonly [SCM_ROW_ACTION_IDS.stageAll]: ActionSignature<ScmGroupInput>;
	readonly [SCM_ROW_ACTION_IDS.unstage]: ActionSignature<ScmChangesInput>;
	readonly [SCM_ROW_ACTION_IDS.unstageAll]: ActionSignature<ScmGroupInput>;
}

type ScmRowActionId = ActionCatalogId<ScmRowActionCatalog>;

/** Each Action's title, which its menu rows and buttons show. */
const SCM_ROW_ACTION_TITLES = {
	[SCM_ROW_ACTION_IDS.closeOtherRepositories]: sourceControlMessages.closeOtherRepositories,
	[SCM_ROW_ACTION_IDS.closeRepository]: sourceControlMessages.closeRepository,
	[SCM_ROW_ACTION_IDS.collapseAll]: sourceControlMessages.collapseAll,
	[SCM_ROW_ACTION_IDS.deletePermanently]: sourceControlMessages.deletePermanently,
	[SCM_ROW_ACTION_IDS.discard]: sourceControlMessages.discard,
	[SCM_ROW_ACTION_IDS.discardAll]: sourceControlMessages.discardAll,
	[SCM_ROW_ACTION_IDS.stage]: sourceControlMessages.stage,
	[SCM_ROW_ACTION_IDS.stageAll]: sourceControlMessages.stageAll,
	[SCM_ROW_ACTION_IDS.unstage]: sourceControlMessages.unstage,
	[SCM_ROW_ACTION_IDS.unstageAll]: sourceControlMessages.unstageAll,
} as const satisfies Record<ScmRowActionId, MessageDescriptor>;

const PATH = z.string().min(1);
const GROUP = z.enum(SCM_GROUP_IDS);

const CHANGES_INPUT = scmActionInput<ScmChangesInput>(
	'ScmChangesInput',
	'expected-source-control-changes-input',
	z.object({ changes: z.array(z.object({ group: GROUP, path: PATH, repository: PATH })).min(1) }),
);

const GROUP_INPUT = scmActionInput<ScmGroupInput>(
	'ScmGroupInput',
	'expected-source-control-group-input',
	z.object({ group: GROUP, repository: PATH }),
);

const REPOSITORIES_INPUT = scmActionInput<ScmRepositoriesInput>(
	'ScmRepositoriesInput',
	'expected-source-control-repositories-input',
	z.object({ repositories: z.array(PATH).min(1) }),
);

/** A set of repositories as one key, whatever order or repetition names them. */
function repositoriesKey(repositories: readonly string[]): string {
	return [...new Set(repositories)].sort().join('\n');
}

// Changes of one repository queue behind each other, as VS Code queues a repository's operations,
// so a second press waits for the first instead of being turned away.
const CHANGES_EXECUTION = {
	concurrency: 'serialize',
	concurrencyKey: ({ changes }: ScmChangesInput) =>
		repositoriesKey(changes.map(({ repository }) => repository)),
	mode: 'foreground',
} as const;

const GROUP_EXECUTION = {
	concurrency: 'serialize',
	concurrencyKey: ({ repository }: ScmGroupInput) => repository,
	mode: 'foreground',
} as const;

const REPOSITORIES_EXECUTION = {
	concurrency: 'serialize',
	concurrencyKey: ({ repositories }: ScmRepositoriesInput) => repositoriesKey(repositories),
	mode: 'foreground',
} as const;

const ENABLED: ActionAvailability = { kind: 'enabled' };
const HIDDEN: ActionAvailability = { kind: 'hidden' };
const COMPLETED: ActionResult<void> = { kind: 'completed' };
const UNCHANGED: ActionResult<void> = { kind: 'unchanged' };
/** The changes moved on between the question and its answer, so nothing ran. */
const STALE: ActionResult<void> = { code: 'stale-target', kind: 'blocked' };

function shownWhen(condition: boolean): ActionAvailability {
	return condition ? ENABLED : HIDDEN;
}

/** What a question asked about: each change with the status it had. */
function planKey(plan: readonly ScmCurrentChange[]): string {
	return plan
		.map(({ repository, resource }) => `${repository}\0${resource.path}\0${resource.status}`)
		.sort()
		.join('\n');
}

/** Each repository's paths among the changes, in their order. */
function pathsByRepository(changes: readonly ScmCurrentChange[]): Map<string, string[]> {
	const paths = new Map<string, string[]>();
	for (const { repository, resource } of changes) {
		const list = paths.get(repository);
		if (list === undefined) paths.set(repository, [resource.path]);
		else list.push(resource.path);
	}
	return paths;
}

/**
 * Adds the paths of a clean that the trash refused to `refused`, as changes the person can delete
 * permanently instead, and returns the first other failure. A path already gone counts as cleaned.
 */
function cleanFailure(
	output: GitMutationOutput,
	repository: string,
	refused: ScmChangeTarget[],
): FileError | null {
	if (output.kind !== 'cleaned') return null;
	for (const { error, path } of output.items) {
		if (error === null || error.code === 'not-found') continue;
		if (error.code !== 'trash-failed' && error.code !== 'trash-unavailable') return error;
		refused.push({ group: 'workingTree', path, repository });
	}
	return null;
}

/** A row command on its target: the Action it runs with the input it gives it. */
interface ScmBoundCommand {
	readonly execute: (source: ActionInvocationSource) => ActionExecution;
	readonly resolve: (source: ActionInvocationSource) => ActionAvailability;
	readonly title: MessageDescriptor;
}

const CHANGE_ACTIONS = {
	clean: SCM_ROW_ACTION_IDS.discard,
	stage: SCM_ROW_ACTION_IDS.stage,
	unstage: SCM_ROW_ACTION_IDS.unstage,
} as const;

const GROUP_ACTIONS = {
	'clean-all': SCM_ROW_ACTION_IDS.discardAll,
	'collapse-all': SCM_ROW_ACTION_IDS.collapseAll,
	'stage-all': SCM_ROW_ACTION_IDS.stageAll,
	'unstage-all': SCM_ROW_ACTION_IDS.unstageAll,
} as const;

const REPOSITORY_ACTIONS = {
	close: SCM_ROW_ACTION_IDS.closeRepository,
	'close-others': SCM_ROW_ACTION_IDS.closeOtherRepositories,
} as const;

/**
 * The Action a row command runs. Opening files and diffs waits for the file overlay to open them from
 * a view, Reveal in Explorer View for the Explorer to reveal a path, Reveal in File Explorer for a
 * desktop bridge, and Add to .gitignore for a backend write: until then they have none, so they hide.
 */
function commandActionId(command: ScmRowCommand): ScmRowActionId | null {
	switch (command) {
		case 'clean':
		case 'stage':
		case 'unstage':
			return CHANGE_ACTIONS[command];
		case 'clean-all':
		case 'collapse-all':
		case 'stage-all':
		case 'unstage-all':
			return GROUP_ACTIONS[command];
		case 'close':
		case 'close-others':
			return REPOSITORY_ACTIONS[command];
		case 'ignore':
		case 'open-change':
		case 'open-file':
		case 'open-head-file':
		case 'reveal-in-explorer':
		case 'reveal-in-os':
		case 'view-changes':
		case 'view-staged-changes':
			return null;
	}
}

function bindRowCommand(
	typed: TypedActionRuntime<ScmRowActionCatalog>,
	scope: ActionScope,
	command: ScmRowCommand,
	target: ScmRowTarget,
): ScmBoundCommand | null {
	const bound = <Id extends ScmRowActionId>(
		id: Id,
		input: ActionInputFor<ScmRowActionCatalog, Id>,
	): ScmBoundCommand => ({
		execute: (source) => typed.execute(id, input, { scope, source }),
		resolve: (source) => typed.resolveAction(id, input, { scope, source })?.availability ?? HIDDEN,
		title: SCM_ROW_ACTION_TITLES[id],
	});
	switch (command) {
		case 'clean':
		case 'stage':
		case 'unstage':
			return target.kind === 'changes' && target.changes.length > 0
				? bound(CHANGE_ACTIONS[command], { changes: target.changes })
				: null;
		case 'clean-all':
		case 'collapse-all':
		case 'stage-all':
		case 'unstage-all':
			return target.kind === 'group'
				? bound(GROUP_ACTIONS[command], { group: target.group, repository: target.repository })
				: null;
		case 'close':
		case 'close-others':
			return target.kind === 'repositories' && target.repositories.length > 0
				? bound(REPOSITORY_ACTIONS[command], { repositories: target.repositories })
				: null;
		default:
			return null;
	}
}

/** A row command as a button or menu row shows it. */
interface ScmResolvedRowCommand {
	readonly disabled: boolean;
	readonly title: MessageDescriptor;
}

/** What the view's rows run: their inline buttons and context menu rows. */
export interface ScmRowCommands {
	/**
	 * A command's title, the label of its button; null while it has no Action. A row's buttons stand
	 * for VS Code's inline menu, whose rules `scmRowMenus` already applied, so they resolve on a press.
	 */
	title(command: ScmRowCommand): MessageDescriptor | null;
	/** A command on its target as it shows now; null while it hides or has no Action. */
	resolve(
		command: ScmRowCommand,
		target: ScmRowTarget,
		source: ActionInvocationSource,
	): ScmResolvedRowCommand | null;
	/** Runs a command on its target; null when the command has no Action. */
	run(
		command: ScmRowCommand,
		target: ScmRowTarget,
		source: ActionInvocationSource,
	): ActionExecution | null;
}

function rowCommands(
	typed: TypedActionRuntime<ScmRowActionCatalog>,
	scope: ActionScope,
): ScmRowCommands {
	return {
		title: (command) => {
			const id = commandActionId(command);
			return id === null ? null : SCM_ROW_ACTION_TITLES[id];
		},
		resolve: (command, target, source) => {
			const bound = bindRowCommand(typed, scope, command, target);
			const availability = bound?.resolve(source) ?? HIDDEN;
			if (bound === null || availability.kind === 'hidden') return null;
			return { disabled: availability.kind === 'disabled', title: bound.title };
		},
		run: (command, target, source) =>
			bindRowCommand(typed, scope, command, target)?.execute(source) ?? null,
	};
}

/** What the row Actions work with while the view shows a Workspace. */
interface ScmRowActionsOptions {
	readonly intl: IntlShape;
	readonly model: SourceControlModel;
	readonly notifications: SourceControlNotifications;
	readonly session: FileWorkspaceSession;
}

function registerRowActions(
	typed: TypedActionRuntime<ScmRowActionCatalog>,
	scope: ActionScope,
	{ intl, model, notifications, session }: ScmRowActionsOptions,
): ActionDisposable {
	/** What each open or answered question asked about, by its execution's input. */
	const questions = new WeakMap<object, string>();
	const os = () => model.state.rules?.os ?? 'linux';
	const separator = () => model.state.rules?.separator ?? '/';
	const displayed = () => model.state.repositories.map(({ summary }) => summary.repository);
	const groupOf = (repository: string, group: ScmGroupId): readonly ScmResource[] =>
		model.state.repositories.find(({ summary }) => summary.repository === repository)?.groups?.[
			group
		] ?? [];

	/** The targets in `group` that are still changes there, as they stand now. */
	const currentChanges = (
		targets: readonly ScmChangeTarget[],
		group: ScmGroupId,
	): ScmCurrentChange[] => {
		const groups = new Map<string, ReadonlyMap<string, ScmResource>>();
		const changes = new Map<string, ScmCurrentChange>();
		for (const { group: targetGroup, path, repository } of targets) {
			if (targetGroup !== group) continue;
			let resources = groups.get(repository);
			if (resources === undefined) {
				resources = new Map(
					groupOf(repository, group).map((resource) => [resource.path, resource]),
				);
				groups.set(repository, resources);
			}
			const resource = resources.get(path);
			if (resource !== undefined) changes.set(`${repository}\0${path}`, { repository, resource });
		}
		return [...changes.values()];
	};
	const groupChanges = (repository: string, group: ScmGroupId): ScmCurrentChange[] =>
		groupOf(repository, group).map((resource) => ({ repository, resource }));

	// Every Git call follows its Action's commit point, so a cancellation that stops one leaves what
	// ran unknown: the Action fails rather than cancels, and nobody is told.
	const settle = (error: FileError | null): ActionResult<void> => {
		if (error === null) return COMPLETED;
		if (error.code !== 'cancelled') void notifications.notify(gitFailureNotice(intl, error));
		return { failure: { code: error.code }, kind: 'failed' };
	};
	const runPaths = async (
		op: 'discard' | 'stage' | 'unstage',
		changes: readonly ScmCurrentChange[],
		signal: AbortSignal,
	): Promise<FileError | null> => {
		for (const [repository, paths] of pathsByRepository(changes)) {
			const result = await session.git.run(gitRequestId(), { op, paths, repository }, signal);
			if (result.kind === 'error') return result.error;
		}
		return null;
	};
	const clean = async (
		mode: 'permanent' | 'trash',
		changes: readonly ScmCurrentChange[],
		refused: ScmChangeTarget[],
		signal: AbortSignal,
	): Promise<FileError | null> => {
		for (const [repository, paths] of pathsByRepository(changes)) {
			const result = await session.git.run(
				gitRequestId(),
				{ mode, op: 'clean', paths, repository },
				signal,
			);
			if (result.kind === 'error') return result.error;
			const failure = cleanFailure(result.value, repository, refused);
			if (failure !== null) return failure;
		}
		return null;
	};
	const changePaths = async (
		op: 'stage' | 'unstage',
		changes: readonly ScmCurrentChange[],
		context: ActionExecutionContext,
	): Promise<ActionResult<void>> => {
		if (changes.length === 0) return UNCHANGED;
		context.markCommitted();
		return settle(await runPaths(op, changes, context.signal));
	};

	/**
	 * A question about `plan`, remembered for the handler, and withdrawn as soon as the changes it
	 * names move on, as when an Agent edits or stages them while it shows.
	 */
	const ask = (
		input: object,
		plan: readonly ScmCurrentChange[],
		question: ActionConfirmation,
		replan: () => readonly ScmCurrentChange[],
	): ActionConfirmation => {
		const asked = planKey(plan);
		questions.set(input, asked);
		return {
			...question,
			watch: (withdraw) =>
				model.subscribe(() => {
					if (planKey(replan()) !== asked) withdraw();
				}),
		};
	};
	/** Whether `plan` is still what its question asked about. */
	const answered = (input: object, plan: readonly ScmCurrentChange[]): boolean =>
		questions.get(input) === planKey(plan);

	const discard = async (
		context: ActionExecutionContext,
		input: object,
		plan: readonly ScmCurrentChange[],
	): Promise<ActionResult<void>> => {
		if (plan.length === 0) return UNCHANGED;
		if (!answered(input, plan)) return STALE;
		context.markCommitted();
		const refused: ScmChangeTarget[] = [];
		// VS Code removes the untracked files first, then restores the tracked ones from the index.
		const untracked = plan.filter(({ resource }) => isScmUntracked(resource.status));
		const tracked = plan.filter(({ resource }) => !isScmUntracked(resource.status));
		const failure =
			(await clean('trash', untracked, refused, context.signal)) ??
			(await runPaths('discard', tracked, context.signal));
		if (failure === null && refused.length > 0) {
			typed.execute(
				SCM_ROW_ACTION_IDS.deletePermanently,
				{ changes: refused },
				{ scope, source: context.source },
			);
		}
		return settle(failure);
	};

	const discardPlan = ({ changes }: ScmChangesInput) => currentChanges(changes, 'workingTree');
	const discardAllPlan = ({ repository }: ScmGroupInput) => groupChanges(repository, 'workingTree');
	const permanentPlan = ({ changes }: ScmChangesInput) =>
		currentChanges(changes, 'workingTree').filter(({ resource }) =>
			isScmUntracked(resource.status),
		);

	const registrations = [
		typed.register(
			scope,
			defineAction({
				category: SCM_GIT_CATEGORY,
				execution: CHANGES_EXECUTION,
				getAvailability: (_context, { changes }) =>
					shownWhen(changes.some(({ group }) => group === 'workingTree')),
				handler: {
					execute: (context, { changes }) =>
						changePaths('stage', currentChanges(changes, 'workingTree'), context),
				},
				// VS Code parity: git.stage.
				id: SCM_ROW_ACTION_IDS.stage,
				input: CHANGES_INPUT,
				title: SCM_ROW_ACTION_TITLES[SCM_ROW_ACTION_IDS.stage],
			}),
		),
		typed.register(
			scope,
			defineAction({
				category: SCM_GIT_CATEGORY,
				execution: CHANGES_EXECUTION,
				getAvailability: (_context, { changes }) =>
					shownWhen(changes.some(({ group }) => group === 'index')),
				handler: {
					execute: (context, { changes }) =>
						changePaths('unstage', currentChanges(changes, 'index'), context),
				},
				// VS Code parity: git.unstage.
				id: SCM_ROW_ACTION_IDS.unstage,
				input: CHANGES_INPUT,
				title: SCM_ROW_ACTION_TITLES[SCM_ROW_ACTION_IDS.unstage],
			}),
		),
		typed.register(
			scope,
			defineAction({
				category: SCM_GIT_CATEGORY,
				confirmation: (input) => {
					const plan = discardPlan(input);
					if (plan.length === 0) return null;
					return ask(input, plan, scmDiscardQuestion(plan, os(), separator()), () =>
						discardPlan(input),
					);
				},
				execution: CHANGES_EXECUTION,
				getAvailability: (_context, { changes }) =>
					shownWhen(changes.some(({ group }) => group === 'workingTree')),
				handler: { execute: (context, input) => discard(context, input, discardPlan(input)) },
				// VS Code parity: git.clean.
				id: SCM_ROW_ACTION_IDS.discard,
				input: CHANGES_INPUT,
				title: SCM_ROW_ACTION_TITLES[SCM_ROW_ACTION_IDS.discard],
			}),
		),
		typed.register(
			scope,
			defineAction({
				category: SCM_GIT_CATEGORY,
				confirmation: (input) => {
					const plan = permanentPlan(input);
					if (plan.length === 0) return null;
					return ask(input, plan, scmPermanentDeleteQuestion(plan.length, os()), () =>
						permanentPlan(input),
					);
				},
				execution: CHANGES_EXECUTION,
				getAvailability: (_context, { changes }) =>
					shownWhen(changes.some(({ group }) => group === 'workingTree')),
				handler: {
					execute: async (context, input): Promise<ActionResult<void>> => {
						const plan = permanentPlan(input);
						if (plan.length === 0) return UNCHANGED;
						if (!answered(input, plan)) return STALE;
						context.markCommitted();
						return settle(await clean('permanent', plan, [], context.signal));
					},
				},
				// VS Code parity: the permanent deletion VS Code Git's `_clean` offers when the trash fails.
				id: SCM_ROW_ACTION_IDS.deletePermanently,
				input: CHANGES_INPUT,
				title: SCM_ROW_ACTION_TITLES[SCM_ROW_ACTION_IDS.deletePermanently],
			}),
		),
		typed.register(
			scope,
			defineAction({
				category: SCM_GIT_CATEGORY,
				execution: GROUP_EXECUTION,
				getAvailability: (_context, { group }) => shownWhen(group === 'workingTree'),
				handler: {
					execute: (context, { repository }) =>
						changePaths('stage', groupChanges(repository, 'workingTree'), context),
				},
				// VS Code parity: git.stageAll.
				id: SCM_ROW_ACTION_IDS.stageAll,
				input: GROUP_INPUT,
				title: SCM_ROW_ACTION_TITLES[SCM_ROW_ACTION_IDS.stageAll],
			}),
		),
		typed.register(
			scope,
			defineAction({
				category: SCM_GIT_CATEGORY,
				execution: GROUP_EXECUTION,
				getAvailability: (_context, { group }) => shownWhen(group === 'index'),
				handler: {
					execute: (context, { repository }) =>
						changePaths('unstage', groupChanges(repository, 'index'), context),
				},
				// VS Code parity: git.unstageAll.
				id: SCM_ROW_ACTION_IDS.unstageAll,
				input: GROUP_INPUT,
				title: SCM_ROW_ACTION_TITLES[SCM_ROW_ACTION_IDS.unstageAll],
			}),
		),
		typed.register(
			scope,
			defineAction({
				category: SCM_GIT_CATEGORY,
				confirmation: (input) => {
					const plan = discardAllPlan(input);
					if (plan.length === 0) return null;
					return ask(input, plan, scmDiscardQuestion(plan, os(), separator()), () =>
						discardAllPlan(input),
					);
				},
				execution: GROUP_EXECUTION,
				getAvailability: (_context, { group }) => shownWhen(group === 'workingTree'),
				handler: { execute: (context, input) => discard(context, input, discardAllPlan(input)) },
				// VS Code parity: git.cleanAll.
				id: SCM_ROW_ACTION_IDS.discardAll,
				input: GROUP_INPUT,
				title: SCM_ROW_ACTION_TITLES[SCM_ROW_ACTION_IDS.discardAll],
			}),
		),
		typed.register(
			scope,
			defineAction({
				category: SCM_VIEW_CATEGORY,
				execution: {
					concurrency: 'reject-duplicate',
					concurrencyKey: ({ group, repository }: ScmGroupInput) => `${repository}\0${group}`,
					mode: 'foreground',
				},
				getAvailability: () => shownWhen(model.state.view.mode === 'tree'),
				handler: {
					execute: (context, { group, repository }) => {
						context.markCommitted();
						model.collapseGroup(repository, group);
						return COMPLETED;
					},
				},
				// VS Code parity: workbench.scm.action.collapseAll.
				id: SCM_ROW_ACTION_IDS.collapseAll,
				input: GROUP_INPUT,
				title: SCM_ROW_ACTION_TITLES[SCM_ROW_ACTION_IDS.collapseAll],
			}),
		),
		typed.register(
			scope,
			defineAction({
				category: SCM_GIT_CATEGORY,
				execution: REPOSITORIES_EXECUTION,
				getAvailability: (_context, { repositories }) =>
					shownWhen(repositories.some((repository) => displayed().includes(repository))),
				handler: {
					execute: (context, { repositories }) => {
						const shown = displayed();
						const closing = repositories.filter((repository) => shown.includes(repository));
						if (closing.length === 0) return UNCHANGED;
						context.markCommitted();
						model.close(closing);
						return COMPLETED;
					},
				},
				// VS Code parity: git.close.
				id: SCM_ROW_ACTION_IDS.closeRepository,
				input: REPOSITORIES_INPUT,
				title: SCM_ROW_ACTION_TITLES[SCM_ROW_ACTION_IDS.closeRepository],
			}),
		),
		typed.register(
			scope,
			defineAction({
				category: SCM_GIT_CATEGORY,
				execution: REPOSITORIES_EXECUTION,
				getAvailability: (_context, { repositories }) =>
					shownWhen(displayed().some((repository) => !repositories.includes(repository))),
				handler: {
					execute: (context, { repositories }) => {
						const closing = displayed().filter((repository) => !repositories.includes(repository));
						if (closing.length === 0) return UNCHANGED;
						context.markCommitted();
						model.close(closing);
						return COMPLETED;
					},
				},
				// VS Code parity: git.closeOtherRepositories.
				id: SCM_ROW_ACTION_IDS.closeOtherRepositories,
				input: REPOSITORIES_INPUT,
				title: SCM_ROW_ACTION_TITLES[SCM_ROW_ACTION_IDS.closeOtherRepositories],
			}),
		),
	];
	return {
		dispose: () => {
			for (const registration of registrations) registration.dispose();
		},
	};
}

/** The row Actions in one catalog of a runtime: what registers them in a scope and what runs them. */
interface ScmRowActions {
	readonly commands: ScmRowCommands;
	register(options: ScmRowActionsOptions): ActionDisposable;
}

/**
 * The Actions behind the rows' inline buttons and context menus: VS Code's Stage, Unstage and
 * Discard Changes for files, their All variants for a group, Collapse All in tree view, and Close
 * Repository with Close Other Repositories. Discarding asks VS Code's question first; its untracked
 * files go to the trash of the machine holding them, and those the trash refuses are offered for
 * permanent deletion, as VS Code Git offers. Each Action runs on the changes as they stand when it
 * runs, and a question runs only on the changes it asked about.
 */
export function scmRowActions(runtime: ActionRuntime, scope: ActionScope): ScmRowActions {
	const typed = createTypedActionRuntime<ScmRowActionCatalog>(runtime);
	return {
		commands: rowCommands(typed, scope),
		register: (options) => registerRowActions(typed, scope, options),
	};
}
