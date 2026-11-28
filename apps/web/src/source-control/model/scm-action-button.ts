import type { GitRemote, GitRepositorySummary } from '../../file-session';
import { scmUpstreamItem, type ScmSyncTooltip } from './scm-repository-row';

/** What runs after a commit, as VS Code's post-commit commands: nothing, Push, or Sync. */
export type ScmPostCommit = 'push' | 'sync' | null;

/** What a press of the button, or of an item of its menu, runs. */
export type ScmActionButtonCommand =
	| { readonly kind: 'commit'; readonly post: ScmPostCommit }
	| { readonly kind: 'commit-amend' | 'publish' | 'sync' };

/** The codicon a title starts with; `syncing` is VS Code's `sync~spin`. */
export type ScmActionButtonIcon = 'arrow-up' | 'check' | 'cloud-upload' | 'sync' | 'syncing';

/** A title's words after its icon. */
export type ScmActionButtonTitle =
	| {
			readonly kind:
				| 'commit'
				| 'commit-amend'
				| 'commit-and-push'
				| 'commit-and-sync'
				| 'continue'
				| 'publish';
	  }
	| { readonly ahead: number; readonly behind: number; readonly kind: 'sync' };

export type ScmActionButtonTooltip =
	| { readonly branch: string | null; readonly kind: 'commit' }
	| { readonly branch: string; readonly kind: 'publish' | 'publishing' }
	| { readonly kind: 'sync'; readonly tooltip: ScmSyncTooltip }
	| {
			readonly kind:
				| 'commit-and-push'
				| 'commit-and-sync'
				| 'committing'
				| 'committing-and-pushing'
				| 'committing-and-syncing'
				| 'continue-merge'
				| 'continue-rebase'
				| 'continuing-merge'
				| 'continuing-rebase'
				| 'syncing';
	  };

interface ScmActionButtonItem {
	readonly command: ScmActionButtonCommand;
	readonly icon: ScmActionButtonIcon;
	readonly title: ScmActionButtonTitle;
	readonly tooltip: ScmActionButtonTooltip;
}

/** VS Code's SCM action button: what a press runs, and its dropdown's groups when it has one. */
export interface ScmActionButton {
	readonly enabled: boolean;
	readonly primary: ScmActionButtonItem;
	/** The dropdown's groups, separated by lines; empty for a button without a dropdown. */
	readonly secondary: readonly (readonly ScmActionButtonItem[])[];
}

export interface ScmActionButtonInput {
	/** Whether a commit runs in the repository: from any window, or this window's commit step. */
	readonly committing: boolean;
	/** Whether this window runs the push or sync a commit asked for after it. */
	readonly postCommit: boolean;
	/**
	 * The post-commit command a press of the button or of its menu chose, which the primary press
	 * offers until that commit's post-commit command ends, as VS Code's global state does.
	 */
	readonly remembered: ScmPostCommit;
	readonly remotes: readonly GitRemote[];
	/** `git.enable_smart_commit` and `git.suggest_smart_commit`. */
	readonly smartCommit: { readonly enabled: boolean; readonly suggest: boolean };
	readonly summary: GitRepositorySummary;
	/** Whether a sync, push or pull runs in the repository: from any window, or this window's. */
	readonly syncing: boolean;
}

const COMMIT = { kind: 'commit', post: null } as const;

/** Whether the repository has VS Code's action button at all: once its status is read, with a HEAD. */
export function scmActionButtonShown({ head, warm }: GitRepositorySummary): boolean {
	return warm && (head.branch !== null || head.commit !== null);
}

/**
 * VS Code's Git action button (`actionButton.ts` and its post-commit command center): none
 * without a HEAD; Commit while there are changes to commit; else Publish Branch, Sync Changes or a
 * disabled Commit, in that order. A rebase or merge continues instead of committing, and a
 * detached HEAD commits without the dropdown.
 */
export function scmActionButton(input: ScmActionButtonInput): ScmActionButton | null {
	const { committing, postCommit, remembered, smartCommit, summary, syncing } = input;
	const { counts, head, operation, upstream } = summary;
	if (!scmActionButtonShown(summary)) return null;
	const rebasing = operation === 'rebase';
	const merging = operation === 'merge';
	const commitRunning = committing || postCommit;
	// Untracked files count among working tree changes, as VS Code's `git.untrackedChanges: mixed`.
	const changesToCommit =
		counts.index > 0 ||
		((smartCommit.enabled || smartCommit.suggest) && counts.workingTree + counts.untracked > 0);

	const commitButton = (): ScmActionButton => {
		const enabled =
			(changesToCommit || ((rebasing || merging) && counts.merge === 0)) && !commitRunning;
		if (rebasing || merging) {
			let tooltip: 'continue-merge' | 'continue-rebase' | 'continuing-merge' | 'continuing-rebase';
			if (rebasing) tooltip = commitRunning ? 'continuing-rebase' : 'continue-rebase';
			else tooltip = commitRunning ? 'continuing-merge' : 'continue-merge';
			return {
				enabled,
				primary: {
					command: COMMIT,
					icon: 'check',
					title: { kind: 'continue' },
					tooltip: { kind: tooltip },
				},
				secondary: [],
			};
		}
		if (head.branch === null) {
			const tooltip: ScmActionButtonTooltip = commitRunning
				? { kind: 'committing' }
				: { branch: null, kind: 'commit' };
			return {
				enabled,
				primary: { command: COMMIT, icon: 'check', title: { kind: 'commit' }, tooltip },
				secondary: [],
			};
		}
		const commitTooltip: ScmActionButtonTooltip = committing
			? { kind: 'committing' }
			: { branch: head.branch, kind: 'commit' };
		const commit: ScmActionButtonItem = {
			command: COMMIT,
			icon: 'check',
			title: { kind: 'commit' },
			tooltip: commitTooltip,
		};
		const amend: ScmActionButtonItem = {
			command: { kind: 'commit-amend' },
			icon: 'check',
			title: { kind: 'commit-amend' },
			tooltip: commitTooltip,
		};
		const push: ScmActionButtonItem = {
			command: { kind: 'commit', post: 'push' },
			icon: commitRunning ? 'syncing' : 'arrow-up',
			title: { kind: 'commit-and-push' },
			tooltip: { kind: commitRunning ? 'committing-and-pushing' : 'commit-and-push' },
		};
		const sync: ScmActionButtonItem = {
			command: { kind: 'commit', post: 'sync' },
			icon: commitRunning ? 'syncing' : 'sync',
			title: { kind: 'commit-and-sync' },
			tooltip: { kind: commitRunning ? 'committing-and-syncing' : 'commit-and-sync' },
		};
		let primary = commit;
		if (remembered === 'push') primary = push;
		else if (remembered === 'sync') primary = sync;
		return {
			enabled,
			primary,
			secondary: [
				[commit, amend],
				[push, sync],
			],
		};
	};

	const publishButton = (): ScmActionButton | null => {
		if (upstream !== null || head.branch === null || commitRunning || rebasing || merging) {
			return null;
		}
		const branch = head.branch;
		return {
			enabled: !syncing,
			primary: {
				command: { kind: 'publish' },
				icon: syncing ? 'syncing' : 'cloud-upload',
				title: { kind: 'publish' },
				tooltip: { branch, kind: syncing ? 'publishing' : 'publish' },
			},
			secondary: [],
		};
	};

	const syncButton = (): ScmActionButton | null => {
		if (upstream === null || (upstream.ahead === 0 && upstream.behind === 0)) return null;
		if (commitRunning || rebasing || merging) return null;
		return {
			enabled: !syncing,
			primary: {
				command: { kind: 'sync' },
				icon: syncing ? 'syncing' : 'sync',
				title: { ahead: upstream.ahead, behind: upstream.behind, kind: 'sync' },
				tooltip: syncing
					? { kind: 'syncing' }
					: { kind: 'sync', tooltip: scmUpstreamItem(upstream, input.remotes).tooltip },
			},
			secondary: [],
		};
	};

	if (changesToCommit) return commitButton();
	return publishButton() ?? syncButton() ?? commitButton();
}
