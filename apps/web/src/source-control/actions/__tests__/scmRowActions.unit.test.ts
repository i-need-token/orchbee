import { ActionRuntime, type ActionConfirmation } from '@orchbee/actions';
import { createOrchbeeIntl, sourceControlMessages } from '@orchbee/i18n';
import { describe, expect, it, vi } from 'vitest';

import {
	MemoryFileSessionPort,
	type GitMutation,
	type GitStatusEntry,
	type GitSummary,
} from '../../../file-session';
import { SourceControlModel } from '../../model/SourceControlModel';
import type { ScmRowTarget } from '../../model/scm-row-target';
import type { SourceControlNotifications } from '../../types/source-control-ports';
import { scmRowActions } from '../scmRowActions';

/** How a test answers the questions the Actions ask. */
type Presenter = Parameters<ActionRuntime['registerConfirmationPresenter']>[1];

const WORKSPACE = 'workspace-1';
const REPOSITORY = '/w';

function summary(revision: number): GitSummary {
	return {
		candidates: [],
		git: { state: 'ok', version: '2.47.1' },
		repositories: [
			{
				busy: null,
				counts: { index: 0, merge: 0, untracked: 1, workingTree: 1 },
				head: { branch: 'main', commit: 'a'.repeat(40), detached: false, unborn: false },
				kind: 'root',
				operation: 'none',
				repository: REPOSITORY,
				revision,
				stale: false,
				truncated: false,
				upstream: null,
				warm: true,
			},
		],
		repositoriesTruncated: false,
		unsafe: [],
	};
}

function entry(name: string, x: string, y: string): GitStatusEntry {
	return { original: null, path: `${REPOSITORY}/${name}`, submodule: false, x, y };
}

const MODIFIED = entry('a.ts', '.', 'M');
const UNTRACKED = entry('new.ts', '?', '?');

/** Both files' rows in Changes, as a selection of the two names them. */
const BOTH: ScmRowTarget = {
	changes: [
		{ group: 'workingTree', path: MODIFIED.path, repository: REPOSITORY },
		{ group: 'workingTree', path: UNTRACKED.path, repository: REPOSITORY },
	],
	kind: 'changes',
};

/**
 * Source Control over a repository with a modified and an untracked file, asking `answer`; a
 * `manual` machine holds its answers until the test flushes them.
 */
async function sourceControl(answer: Presenter, answering: 'immediate' | 'manual' = 'immediate') {
	const port = new MemoryFileSessionPort({ answering });
	port.makeDirectory(REPOSITORY);
	port.setRoot(WORKSPACE, REPOSITORY);
	port.git.setStatus(REPOSITORY, 1, [MODIFIED, UNTRACKED]);
	port.git.setSummary(WORKSPACE, summary(1));
	const session = port.open(WORKSPACE);
	const model = new SourceControlModel({
		compareNames: (one, other) => one.localeCompare(other),
		memory: { changes: null, closed: new Set(), inputs: new Map() },
		session,
		settings: undefined,
		statusText: (status) => status,
		storage: undefined,
	});
	await vi.waitFor(() => {
		port.flush();
		expect(model.state.repositories[0]?.groups?.workingTree).toHaveLength(2);
	});
	const runtime = new ActionRuntime();
	const scope = runtime.createScope('source-control');
	runtime.registerConfirmationPresenter(scope, answer);
	const notifications = {
		notify: vi.fn<SourceControlNotifications['notify']>(() =>
			Promise.resolve({ kind: 'dismissed' }),
		),
	};
	const actions = scmRowActions(runtime, scope);
	actions.register({ intl: createOrchbeeIntl('en-US'), model, notifications, session });
	const ran = (): readonly GitMutation[] => port.git.runs().map(({ mutation }) => mutation);
	return { commands: actions.commands, notifications, port, ran };
}

describe('discarding changes from a row', () => {
	it('asks once, moves untracked files to the trash, then restores tracked ones', async () => {
		const asked: ActionConfirmation[] = [];
		const { commands, ran } = await sourceControl((question) => {
			asked.push(question);
			return true;
		});

		const result = await commands.run('clean', BOTH, 'context-menu')?.completion;

		expect(result).toEqual({ kind: 'completed' });
		expect(asked.map(({ title }) => title.id)).toEqual([
			sourceControlMessages.deleteUntrackedTitle.id,
		]);
		expect(ran()).toEqual([
			{ mode: 'trash', op: 'clean', paths: [UNTRACKED.path], repository: REPOSITORY },
			{ op: 'discard', paths: [MODIFIED.path], repository: REPOSITORY },
		]);
	});

	it('offers to delete permanently only the files the trash refused, after its own question', async () => {
		const asked: ActionConfirmation[] = [];
		const { commands, port, ran } = await sourceControl((question) => {
			asked.push(question);
			return true;
		});
		port.git.handle((mutation) => {
			if (mutation.op !== 'clean') return { kind: 'ok', value: { kind: 'done' } };
			const refused = mutation.mode === 'trash';
			return {
				kind: 'ok',
				value: {
					items: mutation.paths.map((path) => ({
						error: refused ? { code: 'trash-unavailable' } : null,
						path,
					})),
					kind: 'cleaned',
				},
			};
		});

		await commands.run('clean', BOTH, 'context-menu')?.completion;
		await vi.waitFor(() => expect(ran()).toHaveLength(3));

		expect(asked.map(({ title }) => title.id)).toEqual([
			sourceControlMessages.deleteUntrackedTitle.id,
			sourceControlMessages.permanentTitle.id,
		]);
		expect(asked[1]?.confirmLabel.values).toEqual({ count: 1 });
		expect(ran()[2]).toEqual({
			mode: 'permanent',
			op: 'clean',
			paths: [UNTRACKED.path],
			repository: REPOSITORY,
		});
	});

	it('withdraws the question when the changes it names move on, and runs nothing', async () => {
		let questions = 0;
		const { commands, port, ran } = await sourceControl(
			(_question, _invocation, signal) =>
				new Promise((resolve) => {
					questions += 1;
					signal.addEventListener('abort', () => resolve(false), { once: true });
				}),
		);

		const execution = commands.run('clean', BOTH, 'context-menu');
		await vi.waitFor(() => expect(questions).toBe(1));
		// An Agent stages the new file while the question shows.
		port.git.setStatus(REPOSITORY, 2, [MODIFIED, entry('new.ts', 'A', '.')]);
		port.git.setSummary(WORKSPACE, summary(2));

		expect(await execution?.completion).toEqual({ kind: 'cancelled' });
		expect(ran()).toEqual([]);
	});

	it('settles a change the person cancels on its way to Git as failed, without an error card', async () => {
		const { commands, notifications, port, ran } = await sourceControl(() => true, 'manual');
		const modified: ScmRowTarget = {
			changes: [{ group: 'workingTree', path: MODIFIED.path, repository: REPOSITORY }],
			kind: 'changes',
		};

		const execution = commands.run('stage', modified, 'button');
		await vi.waitFor(() => expect(port.waiting).toBe(1));
		execution?.cancel();

		expect(await execution?.completion).toEqual({ failure: { code: 'cancelled' }, kind: 'failed' });
		expect(notifications.notify).not.toHaveBeenCalled();
		expect(ran()).toEqual([]);
	});
});
