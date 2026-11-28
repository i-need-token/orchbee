import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	type FilePathRules,
	type GitRepositorySummary,
	type GitStatusEntry,
	type GitSummary,
	MemoryFileSessionPort,
} from '../../../file-session';
import { ExplorerGitDecorations } from '../ExplorerGitDecorations';

const WORKSPACE = 'workspace';

function repository(path: string): GitRepositorySummary {
	return {
		busy: null,
		counts: { index: 0, merge: 0, untracked: 0, workingTree: 0 },
		head: { branch: 'main', commit: 'a'.repeat(40), detached: false, unborn: false },
		kind: 'root',
		operation: 'none',
		repository: path,
		revision: 1,
		stale: false,
		truncated: false,
		upstream: null,
		warm: true,
	};
}

function entry(path: string, x: string, y: string, original: string | null = null): GitStatusEntry {
	return { original, path, submodule: false, x, y };
}

function summary(...repositories: readonly string[]): GitSummary {
	return {
		candidates: [],
		git: { state: 'ok', version: '2.47.1' },
		repositories: repositories.map(repository),
		repositoriesTruncated: false,
		unsafe: [],
	};
}

async function settle(): Promise<void> {
	for (let turn = 0; turn < 20; turn += 1) await Promise.resolve();
}

/** Decorations over a machine whose one repository, at the root, has these changes. */
async function decorated(
	root: string,
	entries: readonly GitStatusEntry[],
	rules?: FilePathRules,
): Promise<ExplorerGitDecorations> {
	const port = new MemoryFileSessionPort(rules === undefined ? {} : { rules });
	port.writeFile(`${root}${rules?.separator ?? '/'}a.txt`, 'a');
	port.setRoot(WORKSPACE, root);
	port.git.setStatus(root, 1, entries);
	port.git.setSummary(WORKSPACE, summary(root));
	const decorations = new ExplorerGitDecorations(port.open(WORKSPACE));
	await settle();
	return decorations;
}

/** The statuses a row shows: its own and, for a folder, the one it takes from below. */
function shown(
	decorations: ExplorerGitDecorations,
	path: string,
	directory: boolean,
): { readonly descendant: string | null; readonly own: string | null } {
	const { descendant, own } = decorations.decorationOf(path, directory);
	return { descendant: descendant?.status ?? null, own: own?.status ?? null };
}

afterEach(() => {
	vi.useRealTimers();
});

describe('ExplorerGitDecorations', () => {
	it('colors a folder by the first change below it in VS Code’s path order, never by a deletion', async () => {
		const decorations = await decorated('/w', [
			entry('/w/src-x/m.ts', '.', 'M'),
			entry('/w/src/b.ts', '.', 'M'),
			entry('/w/src/a/new.ts', '?', '?'),
			entry('/w/docs/old.md', 'D', '.'),
		]);

		expect(shown(decorations, '/w/src/b.ts', false)).toEqual({ descendant: null, own: 'modified' });
		expect(shown(decorations, '/w/src', true)).toEqual({ descendant: 'untracked', own: null });
		// Path order compares whole names, so `src` and everything below it come before `src-x`.
		expect(shown(decorations, '/w', true)).toEqual({ descendant: 'untracked', own: null });
		expect(shown(decorations, '/w/docs', true)).toEqual({ descendant: null, own: null });
		expect(shown(decorations, '/w/docs/old.md', false)).toEqual({
			descendant: null,
			own: 'index-deleted',
		});
	});

	it('takes a change over a staged one, and marks a rename on both of its paths', async () => {
		const decorations = await decorated('/w', [
			entry('/w/both.ts', 'M', 'M'),
			entry('/w/lib/new.ts', 'R', '.', '/w/old.ts'),
			entry('/w/conflict.ts', 'U', 'U'),
		]);

		expect(shown(decorations, '/w/both.ts', false).own).toBe('modified');
		expect(shown(decorations, '/w/lib/new.ts', false).own).toBe('index-renamed');
		expect(shown(decorations, '/w/old.ts', false).own).toBe('index-renamed');
		expect(shown(decorations, '/w/lib', true).descendant).toBe('index-renamed');
		expect(shown(decorations, '/w/conflict.ts', false).own).toBe('both-modified');
	});

	it('matches paths in any case and orders them by segment on a machine that ignores case', async () => {
		const decorations = await decorated(
			'C:\\Work',
			[entry('C:\\Work\\Src-x\\M.ts', '.', 'M'), entry('C:\\Work\\Src\\A\\New.ts', '?', '?')],
			{ caseSensitive: false, os: 'windows', separator: '\\' },
		);

		expect(shown(decorations, 'c:\\work\\src-x\\m.TS', false).own).toBe('modified');
		expect(shown(decorations, 'C:\\WORK', true).descendant).toBe('untracked');
	});

	it('asks each repository which shown paths it ignores once none has come for 500 ms', async () => {
		vi.useFakeTimers();
		const port = new MemoryFileSessionPort();
		port.writeFile('/w/a.txt', 'a');
		port.setRoot(WORKSPACE, '/w');
		port.git.setStatus('/w', 1, []);
		port.git.setStatus('/w/vendor/lib', 1, []);
		port.git.setIgnored('/w', ['/w/build', '/w/build/out.js']);
		port.git.setIgnored('/w/vendor/lib', ['/w/vendor/lib/dist']);
		port.git.setSummary(WORKSPACE, summary('/w', '/w/vendor/lib'));
		const session = port.open(WORKSPACE);
		const asked: { readonly paths: readonly string[]; readonly repository: string }[] = [];
		const decorations = new ExplorerGitDecorations({
			...session,
			git: {
				...session.git,
				checkIgnore: (request, signal) => {
					asked.push(request);
					return session.git.checkIgnore(request, signal);
				},
			},
		});
		await settle();

		decorations.show('/w/build');
		await vi.advanceTimersByTimeAsync(300);
		for (const path of ['/w/build/out.js', '/w/vendor/lib/dist', '/w/vendor/lib', '/w/src']) {
			decorations.show(path);
		}
		await vi.advanceTimersByTimeAsync(499);
		expect(asked).toEqual([]);
		await vi.advanceTimersByTimeAsync(1);
		await settle();

		// A repository's own folder is never asked about, and a path goes to its innermost repository.
		expect(asked).toEqual([
			{ paths: ['/w/build', '/w/build/out.js', '/w/src'], repository: '/w' },
			{ paths: ['/w/vendor/lib/dist'], repository: '/w/vendor/lib' },
		]);
		const ignored = [
			'/w/build',
			'/w/build/out.js',
			'/w/vendor/lib/dist',
			'/w/vendor/lib',
			'/w/src',
		];
		expect(ignored.map((path) => decorations.decorationOf(path, false).ignored)).toEqual([
			true,
			true,
			true,
			false,
			false,
		]);
		decorations.dispose();
	});
});
