/** A Workspace row's second line: a branch says Git answers where the Workspace stands. */
interface WorktreeRowLine {
	readonly kind: 'branch' | 'folder';
	readonly text: string;
}

/** What the sidebar knows of Git where a Workspace stands. */
export interface WorktreeRowsEvidence {
	readonly line: WorktreeRowLine | null;
	/**
	 * The last discovery that settled, with the line the row showed when it began, so a move or a
	 * branch switch outdates it.
	 */
	readonly settled: { readonly git: boolean; readonly line: WorktreeRowLine | null } | null;
}

function sameLine(left: WorktreeRowLine | null, right: WorktreeRowLine | null): boolean {
	return left === null || right === null
		? left === right
		: left.kind === right.kind && left.text === right.text;
}

/**
 * Worktree rows show only where Git is known, decided before the menu paints, so a Workspace outside
 * Git never shows them, not even while its first read runs. The last settled discovery decides while
 * the row still shows what it showed then: one that found Git shows the rows, and any failure hides
 * them. Otherwise only a branch on the row shows them; a folder alone says nothing about Git.
 */
export function worktreeRowsShown({ line, settled }: WorktreeRowsEvidence): boolean {
	if (settled !== null && sameLine(settled.line, line)) return settled.git;
	return line?.kind === 'branch';
}
