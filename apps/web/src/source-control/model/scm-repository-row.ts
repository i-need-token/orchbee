import type { GitRemote, GitRepositorySummary } from '../../file-session';

/** The codicon VS Code's checkout item starts with. */
type ScmCheckoutIcon =
	| 'git-branch'
	| 'git-branch-changes'
	| 'git-branch-conflicts'
	| 'git-branch-staged-changes'
	| 'git-commit';

/** A change this window runs in the repository that VS Code's checkout and sync items name. */
export type ScmRepositoryActivity = 'commit' | 'none' | 'sync';

/** VS Code's checkout item: the branch or commit, with its marks and icon. */
export interface ScmCheckoutItem {
	readonly activity: ScmRepositoryActivity;
	/** Null for an unborn branch without a name, which VS Code draws without an icon. */
	readonly icon: ScmCheckoutIcon | null;
	/** VS Code's `headLabel`: the branch, else the commit's first 8 characters, with `*+!` marks. */
	readonly label: string;
	readonly rebasing: boolean;
}

/** What the sync item's tooltip says. */
export type ScmSyncTooltip =
	| { readonly kind: 'committing' | 'none' | 'publish' | 'synchronize' | 'syncing' }
	| { readonly behind: number; readonly kind: 'pull'; readonly upstream: string }
	| { readonly ahead: number; readonly kind: 'push'; readonly upstream: string }
	| {
			readonly ahead: number;
			readonly behind: number;
			readonly kind: 'pull-and-push';
			readonly upstream: string;
	  };

/** VS Code's sync item, which a repository without remotes does not have. */
export interface ScmSyncItem {
	/** What a click runs; null while it is disabled. */
	readonly action: 'publish' | 'sync' | null;
	readonly icon: 'cloud-upload' | 'sync' | 'syncing';
	/** `1↓ 2↑`, or only the incoming commits when the remote cannot be pushed to; null when even. */
	readonly label: { readonly ahead: number | null; readonly behind: number } | null;
	readonly tooltip: ScmSyncTooltip;
}

export function scmCheckoutItem(
	summary: GitRepositorySummary,
	activity: ScmRepositoryActivity,
): ScmCheckoutItem {
	const { counts, head, operation } = summary;
	const conflicted = operation === 'merge' || operation === 'rebase';
	const changed = counts.workingTree + counts.untracked > 0;
	const staged = counts.index > 0;
	const name = head.branch ?? (head.commit ?? '').slice(0, 8);
	const label = `${name}${changed ? '*' : ''}${staged ? '+' : ''}${conflicted ? '!' : ''}`;
	let icon: ScmCheckoutIcon | null = null;
	if (head.branch !== null) {
		if (conflicted) icon = 'git-branch-conflicts';
		else if (staged) icon = 'git-branch-staged-changes';
		else if (changed) icon = 'git-branch-changes';
		else icon = 'git-branch';
	} else if (head.commit !== null) icon = 'git-commit';
	return { activity, icon, label, rebasing: operation === 'rebase' };
}

/** The remote an upstream such as `origin/main` belongs to; remote names may hold slashes. */
function upstreamRemote(upstream: string, remotes: readonly GitRemote[]): GitRemote | null {
	let found: GitRemote | null = null;
	for (const remote of remotes) {
		if (!upstream.startsWith(`${remote.name}/`)) continue;
		if (found === null || remote.name.length > found.name.length) found = remote;
	}
	return found;
}

/** VS Code's `syncLabel` and `syncTooltip` for a branch that has an upstream. */
function upstreamItem(
	upstream: NonNullable<GitRepositorySummary['upstream']>,
	remotes: readonly GitRemote[],
): Pick<ScmSyncItem, 'label' | 'tooltip'> {
	const { ahead, behind, name } = upstream;
	if (ahead === 0 && behind === 0) return { label: null, tooltip: { kind: 'synchronize' } };
	const remote = upstreamRemote(name, remotes);
	// A remote whose push URL Git was told never to use, as `no_push`, takes no outgoing commits.
	const readOnly = remote !== null && (remote.pushUrl === null || remote.pushUrl === 'no_push');
	const label = { ahead: readOnly ? null : ahead, behind };
	if (readOnly || ahead === 0) return { label, tooltip: { behind, kind: 'pull', upstream: name } };
	if (behind === 0) return { label, tooltip: { ahead, kind: 'push', upstream: name } };
	return { label, tooltip: { ahead, behind, kind: 'pull-and-push', upstream: name } };
}

/** VS Code's sync item; null while the repository has no remotes, as nothing could publish it. */
export function scmSyncItem(
	summary: GitRepositorySummary,
	remotes: readonly GitRemote[],
	activity: ScmRepositoryActivity,
): ScmSyncItem | null {
	if (remotes.length === 0) return null;
	const { head, upstream } = summary;
	let item: ScmSyncItem;
	if (head.branch === null || head.commit === null) {
		item = { action: null, icon: 'sync', label: null, tooltip: { kind: 'none' } };
	} else if (upstream === null) {
		item = { action: 'publish', icon: 'cloud-upload', label: null, tooltip: { kind: 'publish' } };
	} else {
		item = { action: 'sync', icon: 'sync', ...upstreamItem(upstream, remotes) };
	}
	if (activity === 'commit') return { ...item, action: null, tooltip: { kind: 'committing' } };
	if (activity === 'sync') {
		return { ...item, action: null, icon: 'syncing', tooltip: { kind: 'syncing' } };
	}
	return item;
}

interface ScmRepositoryName {
	/** Disambiguates repositories that share a name: the folder holding each. */
	readonly description: string | null;
	readonly name: string;
}

function lastSegment(path: string, separator: string): string {
	const trimmed = path.endsWith(separator) ? path.slice(0, -1) : path;
	return trimmed.slice(trimmed.lastIndexOf(separator) + 1);
}

/**
 * Each repository's row label, VS Code's provider name: the folder's name, described by the
 * folder holding it, relative to the root when inside it, when another repository has that name.
 */
export function scmRepositoryNames(
	repositories: readonly string[],
	separator: '/' | '\\',
	root: string | null,
): ReadonlyMap<string, ScmRepositoryName> {
	const names = new Map<string, number>();
	for (const repository of repositories) {
		const name = lastSegment(repository, separator);
		names.set(name, (names.get(name) ?? 0) + 1);
	}
	const rootPrefix = root === null || root.endsWith(separator) ? root : `${root}${separator}`;
	const labels = new Map<string, ScmRepositoryName>();
	for (const repository of repositories) {
		const name = lastSegment(repository, separator);
		let description: string | null = null;
		if ((names.get(name) ?? 0) > 1) {
			const parent = repository.slice(0, repository.length - name.length).replace(/[\\/]$/u, '');
			const relative =
				rootPrefix !== null && `${parent}${separator}`.startsWith(rootPrefix)
					? parent.slice(rootPrefix.length)
					: parent;
			description = relative === '' ? null : relative;
		}
		labels.set(repository, { description, name });
	}
	return labels;
}
