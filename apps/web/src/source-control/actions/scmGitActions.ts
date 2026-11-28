import {
	createTypedActionRuntime,
	defineAction,
	type ActionAvailability,
	type ActionCatalogId,
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

import type {
	FileError,
	FileResult,
	FileWorkspaceSession,
	GitMutation,
	GitRemote,
	GitRepositorySummary,
} from '../../file-session';
import type { ScmPostCommit } from '../model/scm-action-button';
import { scmActivity } from '../model/scmActivity';
import {
	scmHeadShortName,
	scmRemoteReadOnly,
	scmUpstreamRemote,
} from '../model/scm-repository-row';
import type { ScmLocalActivity, SourceControlModel } from '../model/SourceControlModel';
import type {
	SourceControlNotifications,
	SourceControlPickEntry,
	SourceControlQuickInput,
} from '../types/source-control-ports';
import { gitFailureNotice } from '../utils/gitFailureNotice';
import { gitRequestId } from '../utils/gitRequestId';
import { SCM_GIT_CATEGORY } from './scm-action-categories';
import { scmActionInput } from './scmActionInput';

const SCM_GIT_ACTION_IDS = {
	commit: 'source-control.commit',
	commitAmend: 'source-control.commit-amend',
	fetch: 'source-control.fetch',
	publish: 'source-control.publish',
	pull: 'source-control.pull',
	push: 'source-control.push',
	sync: 'source-control.sync',
} as const;

interface ScmRepositoryInput {
	readonly repository: string;
}

interface ScmCommitActionInput {
	/** What runs after the commit; absent, what the action button remembers, as for Accept Input. */
	readonly post?: ScmPostCommit | undefined;
	readonly repository: string;
}

interface ScmGitActionCatalog {
	readonly [SCM_GIT_ACTION_IDS.commit]: ActionSignature<ScmCommitActionInput>;
	readonly [SCM_GIT_ACTION_IDS.commitAmend]: ActionSignature<ScmRepositoryInput>;
	readonly [SCM_GIT_ACTION_IDS.fetch]: ActionSignature<ScmRepositoryInput>;
	readonly [SCM_GIT_ACTION_IDS.publish]: ActionSignature<ScmRepositoryInput>;
	readonly [SCM_GIT_ACTION_IDS.pull]: ActionSignature<ScmRepositoryInput>;
	readonly [SCM_GIT_ACTION_IDS.push]: ActionSignature<ScmRepositoryInput>;
	readonly [SCM_GIT_ACTION_IDS.sync]: ActionSignature<ScmRepositoryInput>;
}

type ScmGitActionId = ActionCatalogId<ScmGitActionCatalog>;

/** Which remotes a fetch reaches. */
type ScmFetchRemote = Extract<GitMutation, { readonly op: 'fetch' }>['remote'];

/** A Git command of a repository as the view's controls run it. */
export type ScmGitCommand =
	| { readonly kind: 'commit'; readonly post?: ScmPostCommit | undefined }
	| { readonly kind: 'commit-amend' | 'fetch' | 'publish' | 'pull' | 'push' | 'sync' };

const SCM_GIT_ACTION_TITLES = {
	[SCM_GIT_ACTION_IDS.commit]: sourceControlMessages.buttonCommit,
	[SCM_GIT_ACTION_IDS.commitAmend]: sourceControlMessages.buttonCommitAmend,
	[SCM_GIT_ACTION_IDS.fetch]: sourceControlMessages.fetch,
	[SCM_GIT_ACTION_IDS.publish]: sourceControlMessages.publish,
	[SCM_GIT_ACTION_IDS.pull]: sourceControlMessages.pull,
	[SCM_GIT_ACTION_IDS.push]: sourceControlMessages.push,
	[SCM_GIT_ACTION_IDS.sync]: sourceControlMessages.sync,
} as const satisfies Record<ScmGitActionId, MessageDescriptor>;

const PATH = z.string().min(1);

const REPOSITORY_INPUT = scmActionInput<ScmRepositoryInput>(
	'ScmRepositoryInput',
	'expected-source-control-repository-input',
	z.object({ repository: PATH }),
);

const COMMIT_INPUT = scmActionInput<ScmCommitActionInput>(
	'ScmCommitActionInput',
	'expected-source-control-commit-input',
	z.object({ post: z.enum(['push', 'sync']).nullable().optional(), repository: PATH }),
);

// A second press of the same command in a repository is turned away while the first runs, as VS
// Code's button disables meanwhile; different commands queue in the backend, one write at a time.
const REPOSITORY_EXECUTION = {
	concurrency: 'reject-duplicate',
	concurrencyKey: ({ repository }: ScmRepositoryInput) => repository,
	mode: 'foreground',
} as const;

const DISABLED: ActionAvailability = { kind: 'disabled' };
const ENABLED: ActionAvailability = { kind: 'enabled' };
const HIDDEN: ActionAvailability = { kind: 'hidden' };
const CANCELLED: ActionResult<void> = { kind: 'cancelled' };
const COMPLETED: ActionResult<void> = { kind: 'completed' };
const UNCHANGED: ActionResult<void> = { kind: 'unchanged' };

function activityChange(kind: keyof ScmLocalActivity, value: boolean): Partial<ScmLocalActivity> {
	switch (kind) {
		case 'committing':
			return { committing: value };
		case 'postCommit':
			return { postCommit: value };
		case 'syncing':
			return { syncing: value };
	}
}

/** What lasts while the view does, whichever Workspace it shows, as VS Code's global state lasts. */
interface ScmGitMemory {
	/** VS Code's `confirmBranchPublish`: a push without upstream asks before it publishes. */
	confirmBranchPublish: boolean;
}

/** What the Git Actions work with while the view shows a Workspace. */
interface ScmGitActionsOptions {
	readonly intl: IntlShape;
	readonly model: SourceControlModel;
	readonly notifications: SourceControlNotifications;
	readonly quickInput: SourceControlQuickInput;
	readonly session: FileWorkspaceSession;
}

function registerGitActions(
	typed: TypedActionRuntime<ScmGitActionCatalog>,
	scope: ActionScope,
	memory: ScmGitMemory,
	{ intl, model, notifications, quickInput, session }: ScmGitActionsOptions,
): ActionDisposable {
	const summaryOf = (repository: string): GitRepositorySummary | null =>
		model.state.repositories.find(({ summary }) => summary.repository === repository)?.summary ??
		null;
	// VS Code's `operationInProgress` disables its Git commands while a commit, pull, push or sync
	// runs; here that holds per repository, for one from any window and for this window's own.
	const availability = (repository: string): ActionAvailability => {
		const summary = summaryOf(repository);
		if (summary === null) return HIDDEN;
		const running = scmActivity(summary.busy, model.state.activity.get(repository));
		return running.committing || running.postCommit || running.syncing ? DISABLED : ENABLED;
	};
	const tell = (
		message: MessageDescriptor,
		severity: 'info' | 'warning',
		values?: Record<string, string>,
	): ActionResult<void> => {
		void notifications.notify({ message: intl.formatMessage(message, values), severity });
		return UNCHANGED;
	};
	// Every Git change follows its Action's commit point, so a cancellation that stops one leaves what
	// ran unknown: the Action fails rather than cancels, and nobody is told.
	const settle = (error: FileError): ActionResult<void> => {
		if (error.code !== 'cancelled') void notifications.notify(gitFailureNotice(intl, error));
		return { failure: { code: error.code }, kind: 'failed' };
	};

	/** How many of each activity this window runs in each repository, which its controls show. */
	const running = new Map<string, Record<keyof ScmLocalActivity, number>>();
	const during = async <Result>(
		repository: string,
		kind: keyof ScmLocalActivity,
		run: () => Promise<Result>,
	): Promise<Result> => {
		let counts = running.get(repository);
		if (counts === undefined) {
			counts = { committing: 0, postCommit: 0, syncing: 0 };
			running.set(repository, counts);
		}
		counts[kind] += 1;
		if (counts[kind] === 1) model.setActivity(repository, activityChange(kind, true));
		try {
			return await run();
		} finally {
			counts[kind] -= 1;
			if (counts[kind] === 0) model.setActivity(repository, activityChange(kind, false));
		}
	};

	/** The repository's remotes as they are now: one added in a terminal counts at once. */
	const readRemotes = async (
		repository: string,
		signal: AbortSignal,
	): Promise<FileResult<readonly GitRemote[]>> => {
		const result = await session.git.refs({ repository }, signal);
		return result.kind === 'ok' ? { kind: 'ok', value: result.value.remotes } : result;
	};

	/** Runs a change that reaches a remote; a sync, pull or push spins the repository's sync item. */
	const runRemote = async (
		context: ActionExecutionContext,
		mutation: Extract<GitMutation, { readonly repository: string }>,
		activity: 'syncing' | null,
	): Promise<FileResult<unknown>> => {
		context.markCommitted();
		const run = () => session.git.run(gitRequestId(), mutation, context.signal);
		return activity === null ? run() : during(mutation.repository, activity, run);
	};
	const completed = (result: FileResult<unknown>): ActionResult<void> =>
		result.kind === 'ok' ? COMPLETED : settle(result.error);

	/**
	 * VS Code's modal question answered OK or OK, Don’t Show Again, as a pick that shows the question
	 * where its filter would be; null when dismissed.
	 */
	const confirm = async (
		question: string,
		again: MessageDescriptor,
		signal: AbortSignal,
	): Promise<'again' | 'ok' | null> => {
		const answers: SourceControlPickEntry<'again' | 'ok'>[] = [
			{
				id: 'ok',
				kind: 'item',
				label: intl.formatMessage(sourceControlMessages.remoteOk),
				value: 'ok',
			},
			{ id: 'again', kind: 'item', label: intl.formatMessage(again), value: 'again' },
		];
		const outcome = await quickInput.pick({
			entries: answers,
			placeholder: question,
			signal,
			title: question,
		});
		return outcome.kind === 'accepted' ? outcome.value : null;
	};

	/**
	 * `git.publish`: the branch to its only remote, or to the one picked, tracking it from then on.
	 * After a push that found no upstream, dismissing the pick leaves things as the push left them.
	 */
	const publish = async (
		context: ActionExecutionContext,
		repository: string,
		afterPush = false,
	): Promise<ActionResult<void>> => {
		const branch = summaryOf(repository)?.head.branch ?? null;
		if (branch === null) return tell(sourceControlMessages.remotePushDetached, 'warning');
		const remotes = await readRemotes(repository, context.signal);
		if (remotes.kind === 'error') return settle(remotes.error);
		const [only, ...others] = remotes.value;
		if (only === undefined) return tell(sourceControlMessages.remotePublishNoRemotes, 'warning');
		let remote = only.name;
		if (others.length > 0) {
			const outcome = await quickInput.pick({
				entries: remotes.value.map(
					({ name, pushUrl }): SourceControlPickEntry<string> => ({
						description: pushUrl ?? undefined,
						id: name,
						kind: 'item',
						label: name,
						value: name,
					}),
				),
				placeholder: intl.formatMessage(sourceControlMessages.remotePublishPick, { branch }),
				signal: context.signal,
			});
			if (outcome.kind === 'cancelled') return afterPush ? UNCHANGED : CANCELLED;
			remote = outcome.value;
		}
		const mutation = {
			branch,
			force: 'none',
			op: 'push',
			remote,
			repository,
			setUpstream: true,
			tags: false,
		} as const;
		return completed(await runRemote(context, mutation, 'syncing'));
	};

	/**
	 * `git.push`: the branch to its upstream. A branch without one offers to publish instead, after
	 * VS Code's question unless it was answered OK, Don’t Ask Again in this run.
	 */
	const push = async (
		context: ActionExecutionContext,
		repository: string,
	): Promise<ActionResult<void>> => {
		const remotes = await readRemotes(repository, context.signal);
		if (remotes.kind === 'error') return settle(remotes.error);
		if (remotes.value.length === 0) {
			return tell(sourceControlMessages.remotePushNoRemotes, 'warning');
		}
		const branch = summaryOf(repository)?.head.branch ?? null;
		if (branch === null) return tell(sourceControlMessages.remotePushDetached, 'warning');
		const mutation = {
			branch: null,
			force: 'none',
			op: 'push',
			remote: null,
			repository,
			setUpstream: false,
			tags: false,
		} as const;
		const result = await runRemote(context, mutation, 'syncing');
		if (result.kind === 'ok') return COMPLETED;
		if (result.error.gitErrorCode !== 'no-upstream-branch') return settle(result.error);
		if (memory.confirmBranchPublish) {
			const answer = await confirm(
				intl.formatMessage(sourceControlMessages.remotePublishQuestion, { branch }),
				sourceControlMessages.remoteOkDontAskAgain,
				context.signal,
			);
			// The push already ran and changed nothing, so declining leaves the repository as it was.
			if (answer === null) return UNCHANGED;
			if (answer === 'again') memory.confirmBranchPublish = false;
		}
		return publish(context, repository, true);
	};

	/**
	 * `git.sync`: pulls from the upstream and pushes what the pull leaves ahead, after VS Code's
	 * question unless the remote takes no pushes or `git.confirm_sync` is off. A branch without
	 * upstream pushes instead, which offers to publish it.
	 */
	const sync = async (
		context: ActionExecutionContext,
		repository: string,
	): Promise<ActionResult<void>> => {
		const summary = summaryOf(repository);
		if (summary === null || scmHeadShortName(summary) === null) return UNCHANGED;
		const { upstream } = summary;
		if (upstream === null) return push(context, repository);
		const remote = scmUpstreamRemote(upstream.name, model.state.remotes.get(repository) ?? []);
		const readOnly = remote !== null && scmRemoteReadOnly(remote);
		if (!readOnly && model.state.settings.confirmSync) {
			const separator = remote === null ? upstream.name.indexOf('/') : remote.name.length;
			const answer = await confirm(
				intl.formatMessage(sourceControlMessages.remoteConfirmSync, {
					branch: separator < 0 ? upstream.name : upstream.name.slice(separator + 1),
					remote: separator < 0 ? '' : upstream.name.slice(0, separator),
				}),
				sourceControlMessages.remoteOkDontShowAgain,
				context.signal,
			);
			if (answer === null) return CANCELLED;
			if (answer === 'again') model.writeSetting('git.confirm_sync', false);
		}
		const mutation = { op: 'sync', rebase: false, repository } as const;
		return completed(await runRemote(context, mutation, 'syncing'));
	};

	/** `git.pull`: from the branch's upstream, or as Git decides without one. */
	const pull = async (
		context: ActionExecutionContext,
		repository: string,
	): Promise<ActionResult<void>> => {
		const remotes = await readRemotes(repository, context.signal);
		if (remotes.kind === 'error') return settle(remotes.error);
		if (remotes.value.length === 0) {
			return tell(sourceControlMessages.remotePullNoRemotes, 'warning');
		}
		const mutation = { op: 'pull', rebase: false, repository } as const;
		return completed(await runRemote(context, mutation, 'syncing'));
	};

	/** `git.fetch`: the only remote, else the one picked, the upstream's first, or all of them. */
	const fetch = async (
		context: ActionExecutionContext,
		repository: string,
	): Promise<ActionResult<void>> => {
		const remotes = await readRemotes(repository, context.signal);
		if (remotes.kind === 'error') return settle(remotes.error);
		if (remotes.value.length === 0) {
			return tell(sourceControlMessages.remoteFetchNoRemotes, 'warning');
		}
		let remote: ScmFetchRemote = { kind: 'default' };
		if (remotes.value.length > 1) {
			const fetchable = remotes.value.filter(({ fetchUrl }) => fetchUrl !== null);
			const upstream = summaryOf(repository)?.upstream?.name ?? null;
			const first = upstream === null ? null : scmUpstreamRemote(upstream, fetchable);
			const ordered =
				first === null ? fetchable : [first, ...fetchable.filter((each) => each !== first)];
			const entries: SourceControlPickEntry<ScmFetchRemote>[] = ordered.map(
				({ fetchUrl, name }) => ({
					description: fetchUrl ?? undefined,
					id: name,
					kind: 'item',
					label: name,
					value: { kind: 'named', name },
				}),
			);
			entries.push(
				{ kind: 'separator' },
				{
					id: '',
					kind: 'item',
					label: intl.formatMessage(sourceControlMessages.remoteFetchAll),
					value: { kind: 'all' },
				},
			);
			const outcome = await quickInput.pick({
				entries,
				placeholder: intl.formatMessage(sourceControlMessages.remoteFetchPick),
				signal: context.signal,
			});
			if (outcome.kind === 'cancelled') return CANCELLED;
			remote = outcome.value;
		}
		const mutation = { background: false, op: 'fetch', prune: false, remote, repository } as const;
		return completed(await runRemote(context, mutation, null));
	};

	/** The smart commit question; true when the commit stages every change, null when it stops. */
	const askSmartCommit = async (signal: AbortSignal): Promise<boolean | null> => {
		const answer = <Value extends string>(
			value: Value,
			label: MessageDescriptor,
		): SourceControlPickEntry<Value> => ({
			id: value,
			kind: 'item',
			label: intl.formatMessage(label),
			value,
		});
		const answers: SourceControlPickEntry<'always' | 'never' | 'yes'>[] = [
			answer('yes', sourceControlMessages.smartCommitYes),
			answer('always', sourceControlMessages.smartCommitAlways),
			answer('never', sourceControlMessages.smartCommitNever),
		];
		const outcome = await quickInput.pick({
			entries: answers,
			placeholder: intl.formatMessage(sourceControlMessages.smartCommitPlaceholder),
			signal,
			title: intl.formatMessage(sourceControlMessages.smartCommitTitle),
		});
		if (outcome.kind === 'cancelled') return null;
		if (outcome.value === 'never') {
			model.writeSetting('git.suggest_smart_commit', false);
			return null;
		}
		if (outcome.value === 'always') model.writeSetting('git.enable_smart_commit', true);
		return true;
	};

	/**
	 * VS Code's `commitWithAnyInput` with its default settings: with nothing staged, smart commit
	 * stages every change, after its question unless `git.enable_smart_commit` is on; with nothing to
	 * commit it says so; an empty input asks for the message, apart from an amend, which keeps the
	 * last one, and a rebase, which keeps the commit's own. After the commit the input empties, and
	 * the post-commit command runs: the one asked for, else the one the button remembers. A rebase
	 * continues without one, as VS Code's `rebaseCommit` does.
	 */
	const commit = async (
		context: ActionExecutionContext,
		repository: string,
		amend: boolean,
		post: ScmPostCommit | undefined,
	): Promise<ActionResult<void>> => {
		const summary = summaryOf(repository);
		if (summary === null) return UNCHANGED;
		const { counts, operation } = summary;
		const settings = model.state.settings;
		const merging = operation === 'merge';
		const rebasing = operation === 'rebase';
		const noStaged = counts.index === 0;
		const noUnstaged = counts.workingTree + counts.untracked === 0;
		// A merge or rebase continues as it is when its only unstaged changes are untracked files.
		const continuesUnstaged = (merging || rebasing) && counts.workingTree === 0;
		let smartCommit = settings.enableSmartCommit;
		if (!noUnstaged && noStaged && !smartCommit && !amend && !continuesUnstaged) {
			if (!settings.suggestSmartCommit) return UNCHANGED;
			const staged = await askSmartCommit(context.signal);
			if (staged === null) return CANCELLED;
			smartCommit = staged;
		}
		const all = smartCommit && noStaged ? 'all' : 'none';
		const nothing = (noStaged && noUnstaged) || (all === 'none' && noStaged);
		if (nothing && !amend && !merging && !rebasing) {
			return tell(sourceControlMessages.commitNoChanges, 'info');
		}
		const input = model.input(repository);
		let text = input.value;
		if (text === '' && !(amend && summary.head.commit !== null) && !rebasing) {
			const branch = scmHeadShortName(summary);
			const answer = await quickInput.input({
				ignoreFocusOut: true,
				placeholder:
					branch === null
						? intl.formatMessage(sourceControlMessages.commitMessagePlaceholderNoBranch)
						: intl.formatMessage(sourceControlMessages.commitMessagePlaceholder, { branch }),
				prompt: intl.formatMessage(sourceControlMessages.commitMessagePrompt),
			});
			if (answer.kind === 'cancelled' || answer.value === '' || context.signal.aborted) {
				return CANCELLED;
			}
			text = answer.value;
		}
		// A press of the button or its menu chose the post-commit command, which it now remembers.
		if (post !== undefined && !rebasing) model.remember(repository, post);
		context.markCommitted();
		const mutation = {
			all,
			amend,
			message: { kind: 'text', text },
			noVerify: false,
			op: 'commit',
			repository,
			signoff: false,
		} as const;
		const result = await during(repository, 'committing', () =>
			session.git.run(gitRequestId(), mutation, context.signal),
		);
		// A failed commit keeps the remembered command for the next press, as VS Code's does.
		if (result.kind === 'error') return settle(result.error);
		if (text !== '') input.set('');
		if (rebasing) return COMPLETED;
		const after = input.remembered ?? null;
		try {
			if (after === null) return COMPLETED;
			const outcome = await during(repository, 'postCommit', () =>
				after === 'push' ? push(context, repository) : sync(context, repository),
			);
			// The commit stands whatever its push or sync answers, unless that fails.
			return outcome.kind === 'failed' ? outcome : COMPLETED;
		} finally {
			model.remember(repository, undefined);
		}
	};

	const registrations = [
		typed.register(
			scope,
			defineAction({
				category: SCM_GIT_CATEGORY,
				execution: REPOSITORY_EXECUTION,
				getAvailability: (_context, { repository }) => availability(repository),
				handler: {
					execute: (context, { post, repository }) => commit(context, repository, false, post),
				},
				// VS Code parity: git.commit.
				id: SCM_GIT_ACTION_IDS.commit,
				input: COMMIT_INPUT,
				title: SCM_GIT_ACTION_TITLES[SCM_GIT_ACTION_IDS.commit],
			}),
		),
		typed.register(
			scope,
			defineAction({
				category: SCM_GIT_CATEGORY,
				execution: REPOSITORY_EXECUTION,
				getAvailability: (_context, { repository }) => availability(repository),
				handler: {
					execute: (context, { repository }) => commit(context, repository, true, undefined),
				},
				// VS Code parity: git.commitAmend.
				id: SCM_GIT_ACTION_IDS.commitAmend,
				input: REPOSITORY_INPUT,
				title: SCM_GIT_ACTION_TITLES[SCM_GIT_ACTION_IDS.commitAmend],
			}),
		),
		...(
			[
				// VS Code parity: git.fetch, git.publish, git.pull, git.push, git.sync.
				[SCM_GIT_ACTION_IDS.fetch, fetch],
				[SCM_GIT_ACTION_IDS.publish, publish],
				[SCM_GIT_ACTION_IDS.pull, pull],
				[SCM_GIT_ACTION_IDS.push, push],
				[SCM_GIT_ACTION_IDS.sync, sync],
			] as const
		).map(([id, run]) =>
			typed.register(
				scope,
				defineAction({
					category: SCM_GIT_CATEGORY,
					execution: REPOSITORY_EXECUTION,
					getAvailability: (_context, { repository }) => availability(repository),
					handler: { execute: (context, { repository }) => run(context, repository) },
					id,
					input: REPOSITORY_INPUT,
					title: SCM_GIT_ACTION_TITLES[id],
				}),
			),
		),
	];
	return {
		dispose: () => {
			for (const registration of registrations) registration.dispose();
		},
	};
}

/** What the view's controls run in a repository: the commit input, its button and the menus. */
export interface ScmGitCommands {
	/** Whether the command can run in the repository now, which its menu row shows. */
	enabled(repository: string, command: ScmGitCommand): boolean;
	/** A command's title, the label of its menu row. */
	title(command: ScmGitCommand): MessageDescriptor;
	run(repository: string, command: ScmGitCommand, source: ActionInvocationSource): ActionExecution;
}

/** The Actions that commit and reach remotes in one catalog of a runtime. */
interface ScmGitActions {
	readonly commands: ScmGitCommands;
	register(options: ScmGitActionsOptions): ActionDisposable;
}

function commandActionId(command: ScmGitCommand): ScmGitActionId {
	switch (command.kind) {
		case 'commit':
			return SCM_GIT_ACTION_IDS.commit;
		case 'commit-amend':
			return SCM_GIT_ACTION_IDS.commitAmend;
		case 'fetch':
			return SCM_GIT_ACTION_IDS.fetch;
		case 'publish':
			return SCM_GIT_ACTION_IDS.publish;
		case 'pull':
			return SCM_GIT_ACTION_IDS.pull;
		case 'push':
			return SCM_GIT_ACTION_IDS.push;
		case 'sync':
			return SCM_GIT_ACTION_IDS.sync;
	}
}

/**
 * VS Code's Git commands that commit and reach remotes: Commit and Commit (Amend) with smart
 * commit and the post-commit command, Push, which offers to publish a branch without upstream,
 * Publish Branch, Sync with its question, Pull and Fetch. A second press of a command in a
 * repository is turned away while the first runs.
 */
export function scmGitActions(runtime: ActionRuntime, scope: ActionScope): ScmGitActions {
	const typed = createTypedActionRuntime<ScmGitActionCatalog>(runtime);
	const memory: ScmGitMemory = { confirmBranchPublish: true };
	const execute = <Id extends ScmGitActionId>(
		id: Id,
		input: ActionInputFor<ScmGitActionCatalog, Id>,
		source: ActionInvocationSource,
	): ActionExecution => typed.execute(id, input, { scope, source });
	const input = (repository: string, command: ScmGitCommand): ScmCommitActionInput =>
		command.kind === 'commit' ? { post: command.post, repository } : { repository };
	return {
		commands: {
			enabled: (repository, command) =>
				typed.resolveAction(commandActionId(command), input(repository, command), {
					scope,
					source: 'menu',
				})?.availability.kind === 'enabled',
			run: (repository, command, source) =>
				command.kind === 'commit'
					? execute(SCM_GIT_ACTION_IDS.commit, { post: command.post, repository }, source)
					: execute(commandActionId(command), { repository }, source),
			title: (command) => SCM_GIT_ACTION_TITLES[commandActionId(command)],
		},
		register: (options) => registerGitActions(typed, scope, memory, options),
	};
}
