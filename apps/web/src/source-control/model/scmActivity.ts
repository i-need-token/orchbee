import type { GitRepositorySummary } from '../../file-session';
import type { ScmLocalActivity } from './SourceControlModel';

/** What runs in a repository as VS Code's status bar and action button tell it. */
export interface ScmActivity {
	/** A commit runs, from any window or this window's commit step: VS Code's `isCommitRunning`. */
	readonly committing: boolean;
	/** This window runs the push or sync a commit asked for after it. */
	readonly postCommit: boolean;
	/** A sync, push or pull runs, from any window or this window's: VS Code's `isSyncRunning`. */
	readonly syncing: boolean;
}

/** What runs in a repository: the change the backend reports from any window, and this window's own. */
export function scmActivity(
	busy: GitRepositorySummary['busy'],
	local: ScmLocalActivity | undefined,
): ScmActivity {
	return {
		committing: busy === 'commit' || local?.committing === true,
		postCommit: local?.postCommit === true,
		syncing: busy === 'pull' || busy === 'push' || busy === 'sync' || local?.syncing === true,
	};
}
