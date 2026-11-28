import { describe, expect, it, vi } from 'vitest';

import {
	MemoryFileSessionPort,
	type FileWorkspaceSession,
	type GitRepositorySummary,
	type GitStatusEntry,
	type GitSummary,
} from '../../../file-session';
import { SourceControlModel } from '../SourceControlModel';

const WORKSPACE = 'workspace-1';

function repository(path: string, revision: number): GitRepositorySummary {
	return {
		busy: null,
		counts: { index: 0, merge: 0, untracked: 0, workingTree: 0 },
		head: { branch: 'main', commit: 'a'.repeat(40), detached: false, unborn: false },
		kind: 'nested',
		operation: 'none',
		repository: path,
		revision,
		stale: false,
		truncated: false,
		upstream: null,
		warm: true,
	};
}

function summary(...repositories: readonly GitRepositorySummary[]): GitSummary {
	return {
		candidates: [],
		git: { state: 'ok', version: '2.47.1' },
		repositories,
		repositoriesTruncated: false,
		unsafe: [],
	};
}

function entry(path: string, x: string, y: string): GitStatusEntry {
	return { original: null, path, submodule: false, x, y };
}

/** The Changes rows from top to bottom, each as its kind and what it names. */
function shownRows(model: SourceControlModel): readonly string[] {
	const rows: string[] = [];
	for (let index = 0; index < model.changes.rowCount; index += 1) {
		const item = model.changes.rowAt(index).element;
		switch (item.kind) {
			case 'repository':
				rows.push(`repository ${item.repository}`);
				break;
			case 'group':
				rows.push(`group ${item.group}`);
				break;
			case 'resource':
				rows.push(`${item.resource.status} ${item.resource.path}`);
				break;
			default:
				rows.push(item.kind);
		}
	}
	return rows;
}

/**
 * Source Control over a machine that answers when the test flushes it, and the revision of every
 * status read it starts.
 */
function sourceControl(port: MemoryFileSessionPort) {
	const opened = port.open(WORKSPACE);
	const reads: string[] = [];
	const session: FileWorkspaceSession = {
		...opened,
		git: {
			...opened.git,
			statusPage: (request, signal) => {
				if (request.cursor === undefined) reads.push(`${request.repository}@${request.revision}`);
				return opened.git.statusPage(request, signal);
			},
		},
	};
	const model = new SourceControlModel({
		compareNames: (one, other) => one.localeCompare(other),
		memory: { changes: null, closed: new Set(), inputs: new Map() },
		session,
		settings: undefined,
		statusText: (status) => status,
		storage: undefined,
	});
	return { model, reads };
}

/**
 * The backend's outbound queue folds an SCM subscription that would overflow it into its latest
 * summary, so summaries can arrive with any number missing between them. Each states everything.
 */
describe('Source Control when the backend drops the summaries between two it sends', () => {
	it('shows the latest summary whole and reads status only at its revisions', async () => {
		const port = new MemoryFileSessionPort({ answering: 'manual' });
		port.makeDirectory('/w/a');
		port.makeDirectory('/w/b');
		port.makeDirectory('/w/c');
		port.setRoot(WORKSPACE, '/w');
		port.git.setStatus('/w/a', 1, [entry('/w/a/one.ts', '.', 'M')]);
		port.git.setStatus('/w/c', 1, [entry('/w/c/old.ts', '?', '?')]);
		port.git.setSummary(WORKSPACE, summary(repository('/w/a', 1), repository('/w/c', 1)));
		const { model, reads } = sourceControl(port);
		await vi.waitFor(() => {
			port.flush();
			expect(shownRows(model)).toEqual([
				'repository /w/a',
				'input',
				'button',
				'group workingTree',
				'modified /w/a/one.ts',
				'repository /w/c',
				'input',
				'button',
				'group workingTree',
				'untracked /w/c/old.ts',
			]);
		});

		// Revisions 2 to 4 of `/w/a`, `/w/b`'s discovery and `/w/c`'s removal all happened in
		// summaries that never arrived.
		port.git.setStatus('/w/a', 5, [entry('/w/a/two.ts', 'M', '.'), entry('/w/a/one.ts', '.', 'D')]);
		port.git.setStatus('/w/b', 3, [entry('/w/b/new.ts', 'A', '.')]);
		port.git.setSummary(WORKSPACE, summary(repository('/w/a', 5), repository('/w/b', 3)));

		await vi.waitFor(() => {
			port.flush();
			expect(shownRows(model)).toEqual([
				'repository /w/a',
				'input',
				'button',
				'group index',
				'index-modified /w/a/two.ts',
				'group workingTree',
				'deleted /w/a/one.ts',
				'repository /w/b',
				'input',
				'button',
				'group index',
				'index-added /w/b/new.ts',
				'group workingTree',
			]);
		});
		expect(model.state.repositories.map((shown) => shown.summary.repository)).toEqual([
			'/w/a',
			'/w/b',
		]);
		expect(reads).toEqual(['/w/a@1', '/w/c@1', '/w/a@5', '/w/b@3']);
	});

	it('abandons a status read that a later summary overtakes', async () => {
		const port = new MemoryFileSessionPort({ answering: 'manual' });
		port.makeDirectory('/w');
		port.setRoot(WORKSPACE, '/w');
		port.git.setStatus('/w', 2, [entry('/w/first.ts', '.', 'M')]);
		port.git.setSummary(WORKSPACE, summary(repository('/w', 2)));
		const { model, reads } = sourceControl(port);
		await vi.waitFor(() => expect(reads).toEqual(['/w@2']));

		// The read of revision 2 is still waiting when revision 5 arrives without 3 and 4.
		port.git.setStatus('/w', 5, [entry('/w/last.ts', '?', '?')]);
		port.git.setSummary(WORKSPACE, summary(repository('/w', 5)));

		await vi.waitFor(() => {
			port.flush();
			expect(shownRows(model)).toEqual([
				'input',
				'button',
				'group workingTree',
				'untracked /w/last.ts',
			]);
		});
		expect(reads).toEqual(['/w@2', '/w@5']);
	});
});
