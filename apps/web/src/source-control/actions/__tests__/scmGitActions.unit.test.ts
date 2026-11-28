import { ActionRuntime } from '@orchbee/actions';
import { createOrchbeeIntl } from '@orchbee/i18n';
import { describe, expect, it, vi } from 'vitest';

import {
	MemoryFileSessionPort,
	type GitMutation,
	type GitRemote,
	type GitRepositorySummary,
	type GitStatusEntry,
	type GitSummary,
} from '../../../file-session';
import type { UserSettingsControl, UserSettingsResult } from '../../../user-settings';
import { SourceControlModel } from '../../model/SourceControlModel';
import type {
	SourceControlNotifications,
	SourceControlPickEntry,
	SourceControlQuickInput,
} from '../../types/source-control-ports';
import { scmGitActions } from '../scmGitActions';

const WORKSPACE = 'workspace-1';
const REPOSITORY = '/w';
const ORIGIN: GitRemote = {
	fetchUrl: 'https://example.invalid/w.git',
	name: 'origin',
	pushUrl: 'https://example.invalid/w.git',
};

const STAGED: GitStatusEntry = {
	original: null,
	path: `${REPOSITORY}/staged.ts`,
	submodule: false,
	x: 'M',
	y: '.',
};

function summary(repository: Partial<GitRepositorySummary>): GitSummary {
	return {
		candidates: [],
		git: { state: 'ok', version: '2.47.1' },
		repositories: [
			{
				busy: null,
				counts: { index: 1, merge: 0, untracked: 0, workingTree: 0 },
				head: { branch: 'main', commit: 'a'.repeat(40), detached: false, unborn: false },
				kind: 'root',
				operation: 'none',
				repository: REPOSITORY,
				revision: 1,
				stale: false,
				truncated: false,
				upstream: { ahead: 1, behind: 1, name: 'origin/main' },
				warm: true,
				...repository,
			},
		],
		repositoriesTruncated: false,
		unsafe: [],
	};
}

/** How a test answers a pick: the id of the row to accept, null to dismiss it, or hold it open. */
type PickAnswer = string | null | 'hold';

interface PickRequest<Value> {
	readonly entries: readonly SourceControlPickEntry<Value>[];
	readonly placeholder?: string | undefined;
	readonly signal?: AbortSignal | undefined;
	readonly title?: string | undefined;
}

/** A quick input that answers each pick as `answer` says, recording what each asked. */
function scriptedQuickInput(answer: (asked: number) => PickAnswer) {
	const asked: string[] = [];
	const quickInput: SourceControlQuickInput = {
		input: () => Promise.resolve({ kind: 'cancelled' }),
		pick<Value>({ entries, placeholder, signal, title }: PickRequest<Value>) {
			asked.push(title ?? placeholder ?? '');
			const chosen = answer(asked.length);
			if (chosen === 'hold') {
				return new Promise((resolve) => {
					signal?.addEventListener('abort', () => resolve({ kind: 'cancelled' }), { once: true });
				});
			}
			const entry = entries.find((each) => each.kind === 'item' && each.id === chosen);
			return Promise.resolve(
				entry?.kind === 'item' ? { kind: 'accepted', value: entry.value } : { kind: 'cancelled' },
			);
		},
	};
	return { asked, quickInput };
}

interface SourceControlOptions {
	readonly answering?: 'immediate' | 'manual';
	readonly pick?: (asked: number) => PickAnswer;
	readonly repository?: Partial<GitRepositorySummary>;
}

/** Source Control over one repository with a staged file and `origin`, its picks scripted. */
async function sourceControl(options: SourceControlOptions = {}) {
	const port = new MemoryFileSessionPort({ answering: options.answering ?? 'immediate' });
	port.makeDirectory(REPOSITORY);
	port.setRoot(WORKSPACE, REPOSITORY);
	port.git.setStatus(REPOSITORY, 1, [STAGED]);
	port.git.setSummary(WORKSPACE, summary(options.repository ?? {}));
	port.git.setRefs(REPOSITORY, [], [ORIGIN]);
	const session = port.open(WORKSPACE);
	// Settings that never arrive keep the Git ones at their defaults; a test reads what was answered.
	const unavailable = (): Promise<UserSettingsResult> => Promise.resolve({ kind: 'unavailable' });
	const settings = {
		edit: unavailable,
		getSnapshot: () => ({ available: false, failure: null, pending: false, snapshot: null }),
		refresh: unavailable,
		restore: unavailable,
		set: vi.fn<UserSettingsControl['set']>(unavailable),
		subscribe: () => () => undefined,
	} satisfies UserSettingsControl;
	const model = new SourceControlModel({
		compareNames: (one, other) => one.localeCompare(other),
		memory: { changes: null, closed: new Set(), inputs: new Map() },
		session,
		settings,
		statusText: (status) => status,
		storage: undefined,
	});
	await vi.waitFor(() => {
		port.flush();
		expect(model.state.remotes.get(REPOSITORY)).toEqual([ORIGIN]);
	});
	const runtime = new ActionRuntime();
	const scope = runtime.createScope('source-control');
	const notifications = {
		notify: vi.fn<SourceControlNotifications['notify']>(() =>
			Promise.resolve({ kind: 'dismissed' }),
		),
	};
	const { asked, quickInput } = scriptedQuickInput(options.pick ?? (() => null));
	const actions = scmGitActions(runtime, scope);
	actions.register({ intl: createOrchbeeIntl('en-US'), model, notifications, quickInput, session });
	const ran = (): readonly GitMutation[] => port.git.runs().map(({ mutation }) => mutation);
	return { asked, commands: actions.commands, model, notifications, port, ran, settings };
}

describe('Git commands that overlap', () => {
	it('turns away a second sync and a pull while a sync runs, which reaches Git once', async () => {
		const { commands, model, port, ran } = await sourceControl({
			answering: 'manual',
			pick: () => 'ok',
		});

		const first = commands.run(REPOSITORY, { kind: 'sync' }, 'button');
		await vi.waitFor(() => expect(port.waiting).toBe(1));
		const second = await commands.run(REPOSITORY, { kind: 'sync' }, 'button').completion;
		const pull = await commands.run(REPOSITORY, { kind: 'pull' }, 'menu').completion;
		port.flush();

		expect(await first.completion).toEqual({ kind: 'completed' });
		expect(second.kind).toBe('blocked');
		expect(pull.kind).toBe('blocked');
		expect(ran()).toEqual([{ op: 'sync', rebase: false, repository: REPOSITORY }]);
		expect(model.state.activity.size).toBe(0);
	});
});

describe('cancelling a Git command', () => {
	it('runs nothing when cancelled while its question is open', async () => {
		const { asked, commands, notifications, ran } = await sourceControl({ pick: () => 'hold' });

		const sync = commands.run(REPOSITORY, { kind: 'sync' }, 'button');
		await vi.waitFor(() => expect(asked).toHaveLength(1));
		sync.cancel();

		expect(await sync.completion).toEqual({ kind: 'cancelled' });
		expect(ran()).toEqual([]);
		expect(notifications.notify).not.toHaveBeenCalled();
	});

	it('fails without a card when cancelled while its change waits, and stops spinning', async () => {
		const { commands, model, notifications, port, ran } = await sourceControl({
			answering: 'manual',
		});

		const pull = commands.run(REPOSITORY, { kind: 'pull' }, 'menu');
		// The remotes are read first, then the pull waits for its answer.
		await vi.waitFor(() => expect(port.waiting).toBe(1));
		port.flush();
		await vi.waitFor(() => expect(model.state.activity.get(REPOSITORY)?.syncing).toBe(true));
		pull.cancel();

		expect(await pull.completion).toEqual({ failure: { code: 'cancelled' }, kind: 'failed' });
		expect(ran()).toEqual([]);
		expect(notifications.notify).not.toHaveBeenCalled();
		expect(model.state.activity.size).toBe(0);
	});
});

describe('committing', () => {
	it('commits the message, then pushes as asked, and forgets the post-commit command', async () => {
		const { commands, model, ran } = await sourceControl();
		const input = model.input(REPOSITORY);
		input.type('feat: x');

		const result = await commands.run(REPOSITORY, { kind: 'commit', post: 'push' }, 'button')
			.completion;

		expect(result).toEqual({ kind: 'completed' });
		expect(ran()).toEqual([
			{
				all: 'none',
				amend: false,
				message: { kind: 'text', text: 'feat: x' },
				noVerify: false,
				op: 'commit',
				repository: REPOSITORY,
				signoff: false,
			},
			{
				branch: null,
				force: 'none',
				op: 'push',
				remote: null,
				repository: REPOSITORY,
				setUpstream: false,
				tags: false,
			},
		]);
		expect(input.value).toBe('');
		expect(input.remembered).toBeUndefined();
	});

	it('keeps the post-commit command and pushes nothing when the commit fails', async () => {
		const { commands, model, notifications, port, ran } = await sourceControl();
		port.git.handle(() => ({
			error: { code: 'git-failed', detail: 'error: boom' },
			kind: 'error',
		}));
		const input = model.input(REPOSITORY);
		input.type('feat: x');

		const result = await commands.run(REPOSITORY, { kind: 'commit', post: 'push' }, 'button')
			.completion;

		expect(result).toEqual({ failure: { code: 'git-failed' }, kind: 'failed' });
		expect(ran().map(({ op }) => op)).toEqual(['commit']);
		expect(input.value).toBe('feat: x');
		expect(input.remembered).toBe('push');
		expect(notifications.notify).toHaveBeenCalledOnce();
	});

	it('asks to stage every change when none is staged, and stops for good on Never', async () => {
		const { asked, commands, model, ran, settings } = await sourceControl({
			pick: (count) => (count === 1 ? 'never' : 'yes'),
			repository: { counts: { index: 0, merge: 0, untracked: 1, workingTree: 1 } },
		});
		model.input(REPOSITORY).type('feat: x');

		const never = await commands.run(REPOSITORY, { kind: 'commit' }, 'keyboard').completion;
		const yes = await commands.run(REPOSITORY, { kind: 'commit' }, 'keyboard').completion;

		expect(never).toEqual({ kind: 'cancelled' });
		expect(settings.set).toHaveBeenCalledWith('git.suggest_smart_commit', false);
		expect(yes).toEqual({ kind: 'completed' });
		expect(asked).toHaveLength(2);
		expect(ran()).toMatchObject([{ all: 'all', op: 'commit' }]);
	});
});

describe('pushing a branch without upstream', () => {
	it('offers to publish it, and publishes without asking again after Don’t Ask Again', async () => {
		const { asked, commands, port, ran } = await sourceControl({
			pick: () => 'again',
			repository: { upstream: null },
		});
		port.git.handle((mutation) =>
			mutation.op === 'push' && mutation.remote === null
				? { error: { code: 'git-failed', gitErrorCode: 'no-upstream-branch' }, kind: 'error' }
				: { kind: 'ok', value: { kind: 'done' } },
		);

		await commands.run(REPOSITORY, { kind: 'push' }, 'menu').completion;
		await commands.run(REPOSITORY, { kind: 'push' }, 'menu').completion;

		const publish = {
			branch: 'main',
			force: 'none',
			op: 'push',
			remote: 'origin',
			repository: REPOSITORY,
			setUpstream: true,
			tags: false,
		};
		expect(ran()).toMatchObject([{ remote: null }, publish, { remote: null }, publish]);
		expect(asked).toHaveLength(1);
	});
});
