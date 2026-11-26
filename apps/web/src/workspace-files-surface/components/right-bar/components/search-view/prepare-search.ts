import {
	MAXIMUM_WORKSPACE_SEARCH_GLOB_BYTES,
	MAXIMUM_WORKSPACE_SEARCH_GLOBS,
	MAXIMUM_WORKSPACE_SEARCH_PATHS,
	MAXIMUM_WORKSPACE_SEARCH_PATTERN_BYTES,
} from '@orchbee/workbench-client-protocol';

import type { WorkbenchWorkspaceFilesSource } from '../../../../../workbench';
import { readWorkspaceFiles } from '../../../../readWorkspaceFiles';
import { splitGlobs } from '../../../../search-results';
import type { PreparedSearch, SearchDraft } from '../../../../SearchSessions';

/**
 * What the exclude settings leave out while they apply, as an editor's defaults do; the
 * Workspace's own `.git` is never searched.
 */
const DEFAULT_EXCLUDES = ['node_modules/', 'bower_components/'] as const;

const encoder = new TextEncoder();

/** Names a draft by what it searches for, so the same search is not run twice. */
export function searchKey(draft: SearchDraft): string {
	return JSON.stringify([
		draft.pattern,
		draft.regex,
		draft.matchCase,
		draft.wholeWord,
		draft.includes,
		draft.excludes,
		draft.useIgnoreFiles,
		draft.onlyOpen,
		draft.onlyChanged,
	]);
}

function validGlobs(globs: readonly string[]): boolean {
	return (
		globs.length <= MAXIMUM_WORKSPACE_SEARCH_GLOBS &&
		globs.every(
			(glob) =>
				encoder.encode(glob).length <= MAXIMUM_WORKSPACE_SEARCH_GLOB_BYTES &&
				!/[\p{Cc}\uD800-\uDFFF]/u.test(glob),
		)
	);
}

/**
 * The query a draft asks for. Searching only open files names the float window's files; only
 * changed files reads Git's change list first, and both together search the files in both.
 */
export async function prepareSearch(
	source: WorkbenchWorkspaceFilesSource,
	workspaceId: string,
	draft: SearchDraft,
	openPaths: readonly string[],
): Promise<PreparedSearch> {
	if (encoder.encode(draft.pattern).length > MAXIMUM_WORKSPACE_SEARCH_PATTERN_BYTES) {
		return { kind: 'failed', state: 'too-long' };
	}
	const includes = splitGlobs(draft.includes);
	const excludes = [
		...splitGlobs(draft.excludes),
		...(draft.useIgnoreFiles ? DEFAULT_EXCLUDES : []),
	];
	if (!validGlobs(includes) || !validGlobs(excludes)) {
		return { kind: 'failed', state: 'invalid-glob' };
	}
	let paths: readonly string[] | null = null;
	if (draft.onlyOpen) paths = [...new Set(openPaths)];
	if (draft.onlyChanged) {
		const status = await readWorkspaceFiles(source, workspaceId, { kind: 'git-status' });
		if (status.kind !== 'git-status') return { kind: 'failed', state: 'changed-unavailable' };
		// A deleted file has nothing left to search.
		const changed = status.entries
			.filter(
				(entry) =>
					entry.worktree !== 'deleted' &&
					!(entry.index === 'deleted' && entry.worktree === 'unmodified'),
			)
			.map((entry) => entry.path);
		const open = paths === null ? null : new Set(paths);
		paths = open === null ? changed : changed.filter((path) => open.has(path));
	}
	return {
		kind: 'query',
		query: {
			excludes,
			includes,
			matchCase: draft.matchCase,
			paths: paths === null ? null : paths.slice(0, MAXIMUM_WORKSPACE_SEARCH_PATHS),
			pattern: draft.pattern,
			regex: draft.regex,
			useIgnoreFiles: draft.useIgnoreFiles,
			wholeWord: draft.wholeWord,
		},
	};
}
