import type { FileRoot, GitSummary } from '../../file-session';

/** One of VS Code's Source Control welcome contents, shown while no repository is displayed. */
export type ScmWelcomeItem =
	/** Repositories found that wait for the person: closed here, above the root, or unsafe. */
	| { readonly count: number; readonly kind: 'closed' | 'parent' | 'unsafe' }
	/** The machine has no Git the backend can use: missing, or older than it supports. */
	| { readonly kind: 'missing-git' }
	/** The Workspace has no current directory. */
	| { readonly kind: 'no-directory' }
	/** The folder holds no repository: VS Code's welcome that offers Initialize Repository. */
	| { readonly kind: 'no-repository' }
	/** Discovery has not answered yet. */
	| { readonly kind: 'scanning' };

interface ScmWelcomeInput {
	/** Repositories this window closed, which discovery no longer displays. */
	readonly closed: number;
	/** The repositories the view shows, those discovery found that were not closed here. */
	readonly displayed: number;
	/** The Workspace's root; null before its snapshot. */
	readonly root: FileRoot | null;
	/** The latest summary; null before the first, while discovery has not answered. */
	readonly summary: GitSummary | null;
}

/**
 * What Source Control says instead of rows, in VS Code's order; nothing while a repository shows.
 * The folder's welcome shows only when no repository waits for the person.
 */
export function scmWelcomeItems({
	closed,
	displayed,
	root,
	summary,
}: ScmWelcomeInput): readonly ScmWelcomeItem[] {
	if (root?.state === 'none') return [{ kind: 'no-directory' }];
	if (summary === null) return [{ kind: 'scanning' }];
	if (displayed > 0) return [];
	if (summary.git.state !== 'ok') return [{ kind: 'missing-git' }];
	const items: ScmWelcomeItem[] = [];
	if (summary.candidates.length > 0)
		items.push({ count: summary.candidates.length, kind: 'parent' });
	if (summary.unsafe.length > 0) items.push({ count: summary.unsafe.length, kind: 'unsafe' });
	if (closed > 0) items.push({ count: closed, kind: 'closed' });
	return items.length > 0 ? items : [{ kind: 'no-repository' }];
}
