import type { GitSummary } from '../../file-session';

/**
 * The count badge: every pending change of every displayed repository, as VS Code's
 * `scm.countBadge` and `git.countBadge` add them by default. Untracked files count with Changes.
 */
export function sourceControlCount(summary: GitSummary): number {
	let count = 0;
	for (const { counts } of summary.repositories) {
		count += counts.merge + counts.index + counts.workingTree + counts.untracked;
	}
	return count;
}
