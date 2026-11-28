import type { GitStatusEntry } from '../../file-session';

/** A changed path's state in one group, as VS Code's Git extension names it. */
export type ScmResourceStatus =
	| 'added-by-them'
	| 'added-by-us'
	| 'both-added'
	| 'both-deleted'
	| 'both-modified'
	| 'deleted'
	| 'deleted-by-them'
	| 'deleted-by-us'
	| 'ignored'
	| 'index-added'
	| 'index-copied'
	| 'index-deleted'
	| 'index-modified'
	| 'index-renamed'
	| 'intent-to-add'
	| 'intent-to-rename'
	| 'modified'
	| 'type-changed'
	| 'untracked';

/**
 * The resource groups in VS Code's order. Untracked files join Changes, as a fresh VS Code's
 * `git.untrackedChanges` mixes them, so its Untracked Changes group never has content.
 */
export const SCM_GROUP_IDS = ['merge', 'index', 'workingTree'] as const;

export type ScmGroupId = (typeof SCM_GROUP_IDS)[number];

/** One row of a group: a path in one state. A path can be in Staged Changes and Changes at once. */
export interface ScmResource {
	readonly group: ScmGroupId;
	/** The source of a rename or copy, which the left side of its diff reads. */
	readonly original: string | null;
	/** The path the row shows and opens: a rename's new path. */
	readonly path: string;
	readonly status: ScmResourceStatus;
}

export type ScmResourceGroups = Readonly<Record<ScmGroupId, readonly ScmResource[]>>;

/** The theme color of a status: VS Code's `gitDecoration.*ResourceForeground` roles. */
export type ScmStatusColor =
	| 'added'
	| 'conflicting'
	| 'deleted'
	| 'ignored'
	| 'modified'
	| 'renamed'
	| 'stage-deleted'
	| 'stage-modified'
	| 'untracked';

interface ScmStatusFacts {
	readonly color: ScmStatusColor;
	readonly letter: string;
	/** VS Code draws deleted paths struck through. */
	readonly struckThrough: boolean;
}

const fact = (letter: string, color: ScmStatusColor, struckThrough = false): ScmStatusFacts => ({
	color,
	letter,
	struckThrough,
});

/** Each status's letter, color and strike-through, as VS Code's `Resource` gives them. */
export const SCM_STATUS_FACTS: Readonly<Record<ScmResourceStatus, ScmStatusFacts>> = {
	// `!` rather than ⚠, which VS Code avoids because it renders badly on Windows.
	'added-by-them': fact('!', 'conflicting'),
	'added-by-us': fact('!', 'conflicting'),
	'both-added': fact('!', 'conflicting'),
	'both-deleted': fact('!', 'conflicting', true),
	'both-modified': fact('!', 'conflicting'),
	deleted: fact('D', 'deleted', true),
	'deleted-by-them': fact('!', 'conflicting', true),
	'deleted-by-us': fact('!', 'conflicting', true),
	ignored: fact('I', 'ignored'),
	'index-added': fact('A', 'added'),
	'index-copied': fact('C', 'renamed'),
	'index-deleted': fact('D', 'stage-deleted', true),
	'index-modified': fact('M', 'stage-modified'),
	'index-renamed': fact('R', 'renamed'),
	'intent-to-add': fact('A', 'added'),
	'intent-to-rename': fact('R', 'renamed'),
	modified: fact('M', 'modified'),
	'type-changed': fact('T', 'modified'),
	untracked: fact('U', 'untracked'),
};

function mergeStatus(x: string, y: string): ScmResourceStatus | null {
	switch (`${x}${y}`) {
		case 'AA':
			return 'both-added';
		case 'AU':
			return 'added-by-us';
		case 'DD':
			return 'both-deleted';
		case 'DU':
			return 'deleted-by-us';
		case 'UA':
			return 'added-by-them';
		case 'UD':
			return 'deleted-by-them';
		case 'UU':
			return 'both-modified';
		default:
			return null;
	}
}

/** The index's letter; a staged type change, `T`, has no row, as in VS Code. */
function indexStatus(x: string): ScmResourceStatus | null {
	switch (x) {
		case 'A':
			return 'index-added';
		case 'C':
			return 'index-copied';
		case 'D':
			return 'index-deleted';
		case 'M':
			return 'index-modified';
		case 'R':
			return 'index-renamed';
		default:
			return null;
	}
}

function workingTreeStatus(y: string): ScmResourceStatus | null {
	switch (y) {
		case 'A':
			return 'intent-to-add';
		case 'D':
			return 'deleted';
		case 'M':
			return 'modified';
		case 'R':
			return 'intent-to-rename';
		case 'T':
			return 'type-changed';
		default:
			return null;
	}
}

/**
 * Sorts a status into VS Code's groups: an unmerged pair is a merge change and nothing else, an
 * untracked or ignored path a change, and otherwise the index letter and the working tree letter
 * each make a row of their own group. Rows keep the status's porcelain order.
 */
export function scmResourceGroups(entries: readonly GitStatusEntry[]): ScmResourceGroups {
	const merge: ScmResource[] = [];
	const index: ScmResource[] = [];
	const workingTree: ScmResource[] = [];
	for (const { original, path, x, y } of entries) {
		if (x === '?' || x === '!') {
			const status = x === '?' ? 'untracked' : 'ignored';
			workingTree.push({ group: 'workingTree', original: null, path, status });
			continue;
		}
		const conflict = mergeStatus(x, y);
		if (conflict !== null) {
			merge.push({ group: 'merge', original: null, path, status: conflict });
			continue;
		}
		const staged = indexStatus(x);
		if (staged !== null) {
			// Only a rename or copy reads its source on the index side.
			const source = staged === 'index-renamed' || staged === 'index-copied' ? original : null;
			index.push({ group: 'index', original: source, path, status: staged });
		}
		const changed = workingTreeStatus(y);
		if (changed !== null) {
			workingTree.push({ group: 'workingTree', original, path, status: changed });
		}
	}
	return { index, merge, workingTree };
}
