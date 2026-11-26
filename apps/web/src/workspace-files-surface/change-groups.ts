import type {
	WorkspaceFileSource,
	WorkspaceGitStatusCode,
	WorkspaceGitStatusEntry,
} from '@orchbee/workbench-client-protocol';

export type ChangeGroup = 'conflicts' | 'staged' | 'unstaged';

/** One row of the Changes view and the two sides its diff compares. */
export interface ChangeItem {
	readonly key: string;
	readonly group: ChangeGroup;
	readonly path: string;
	readonly originalPath: string | null;
	/** One letter, as Git's short status shows it; `!` for a conflict. */
	readonly letter: string;
	/** The older side, read at `beforePath`; null where the file is new to this side. */
	readonly before: WorkspaceFileSource | null;
	readonly beforePath: string;
	readonly after: WorkspaceFileSource;
}

export interface ChangeGroups {
	readonly conflicts: readonly ChangeItem[];
	readonly staged: readonly ChangeItem[];
	readonly unstaged: readonly ChangeItem[];
}

const LETTERS: Readonly<Record<WorkspaceGitStatusCode, string>> = {
	added: 'A',
	copied: 'C',
	deleted: 'D',
	modified: 'M',
	renamed: 'R',
	'type-changed': 'T',
	unmerged: 'U',
	unmodified: ' ',
	untracked: 'U',
};

/**
 * Sorts a change list into conflicts, staged changes (`HEAD` against the index) and unstaged ones
 * (the index against the working tree, or nothing against it for an untracked file). A file with
 * both shows in both groups, as Git's own short status does.
 */
export function groupChanges(entries: readonly WorkspaceGitStatusEntry[]): ChangeGroups {
	const conflicts: ChangeItem[] = [];
	const staged: ChangeItem[] = [];
	const unstaged: ChangeItem[] = [];
	for (const entry of entries) {
		if (entry.conflicted) {
			conflicts.push({
				after: 'worktree',
				before: 'head',
				beforePath: entry.path,
				group: 'conflicts',
				key: `conflicts:${entry.path}`,
				letter: '!',
				originalPath: null,
				path: entry.path,
			});
			continue;
		}
		if (entry.index === 'untracked') {
			unstaged.push({
				after: 'worktree',
				before: null,
				beforePath: entry.path,
				group: 'unstaged',
				key: `unstaged:${entry.path}`,
				letter: LETTERS.untracked,
				originalPath: null,
				path: entry.path,
			});
			continue;
		}
		if (entry.index !== 'unmodified') {
			const added = entry.index === 'added' || entry.index === 'copied';
			staged.push({
				after: 'index',
				before: added ? null : 'head',
				beforePath: entry.index === 'renamed' ? (entry.originalPath ?? entry.path) : entry.path,
				group: 'staged',
				key: `staged:${entry.path}`,
				letter: LETTERS[entry.index],
				originalPath: entry.originalPath,
				path: entry.path,
			});
		}
		if (entry.worktree !== 'unmodified') {
			unstaged.push({
				after: 'worktree',
				before: 'index',
				beforePath: entry.path,
				group: 'unstaged',
				key: `unstaged:${entry.path}`,
				letter: LETTERS[entry.worktree],
				originalPath: null,
				path: entry.path,
			});
		}
	}
	const byPath = (left: ChangeItem, right: ChangeItem): number => {
		if (left.path === right.path) return 0;
		return left.path < right.path ? -1 : 1;
	};
	return {
		conflicts: conflicts.sort(byPath),
		staged: staged.sort(byPath),
		unstaged: unstaged.sort(byPath),
	};
}

/** A path's last name and the folder it sits in, as the rows show them. */
export function splitPath(path: string): { readonly name: string; readonly folder: string } {
	const slash = path.lastIndexOf('/');
	return slash < 0
		? { folder: '', name: path }
		: { folder: path.slice(0, slash), name: path.slice(slash + 1) };
}

/**
 * Whether a change can be discarded here: any unstaged change. Git restores a tracked file from the
 * index; an untracked file goes to the system trash.
 */
export function canDiscard(item: ChangeItem): boolean {
	return item.group === 'unstaged';
}

/** Whether a change is a file Git does not track, which discarding moves to the system trash. */
export function isUntracked(item: ChangeItem): boolean {
	return item.group === 'unstaged' && item.before === null;
}

/** Whether the changed file is there to open: a deletion leaves nothing in the working tree. */
export function opensFile(item: ChangeItem): boolean {
	return item.letter !== 'D';
}

/**
 * The paths an unstage names: both sides of a staged rename, so the index loses the new name and
 * gets the old one back.
 */
export function unstagePaths(item: ChangeItem): readonly string[] {
	// A copy leaves its source as it was, and the source's own staged change stays staged.
	return item.letter === 'R' && item.originalPath !== null
		? [item.originalPath, item.path]
		: [item.path];
}
