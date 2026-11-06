import type { WorkbenchPanelTerminal } from './workbench-focused-panel';

/** One Git checkout a Workspace can open, other than the one it stands in. */
interface WorkbenchWorktreeCheckout {
	/** The checked-out branch without `refs/heads/`, or null when HEAD is detached. */
	readonly branch: string | null;
	readonly path: string;
}

/** The Workspace's own directory, when it is a linked checkout this Workspace may remove. */
export interface WorkbenchWorktreeRemoval {
	/** The branch Git keeps, without `refs/heads/`, or null when HEAD is detached. */
	readonly branch: string | null;
	readonly path: string;
	/** Terminals that close with the Workspace. */
	readonly terminalCount: number;
}

/**
 * What Git reported for the repository a Workspace stands in. The authority keeps the latest one for
 * each source it read, the Workspace or one of its Panels, and a command acts on it.
 */
export interface WorkbenchWorktreeDiscovery {
	/** Every other non-bare checkout of the repository, in Git's order. */
	readonly checkouts: readonly WorkbenchWorktreeCheckout[];
	readonly removal: WorkbenchWorktreeRemoval | null;
	readonly workspaceId: string;
}

/**
 * `stoppedRemoval` is the latest removal of this Workspace that stopped where it may continue,
 * whatever Git reports now; the menu offers to resume it in place of a new removal, as the TUI
 * reopens a retained removal.
 */
export type WorkbenchWorktreeDiscoveryResult =
	| {
			readonly kind: 'discovered';
			readonly discovery: WorkbenchWorktreeDiscovery;
			readonly stoppedRemoval: WorkbenchWorktreeStoppedRemoval | null;
	  }
	/** `not-repository`: Git found no repository there, or does not trust its owner. */
	| {
			readonly kind: 'failed';
			readonly reason: 'not-repository' | 'unavailable';
			readonly stoppedRemoval: WorkbenchWorktreeStoppedRemoval | null;
	  };

/**
 * Why a Worktree command stopped, in terms a person can act on. `not-repository` and
 * `listing-unavailable` stop a creation from the focused Panel, which reads Git first.
 */
export type WorkbenchWorktreeFailureCause =
	| 'launch-request-too-large'
	| 'checkout-in-use'
	| 'checkout-not-found'
	| 'close-incomplete'
	| 'configuration-unavailable'
	| 'denied'
	| 'directory-in-use'
	| 'dirty'
	| 'git'
	| 'git-failed'
	| 'internal'
	| 'listing-unavailable'
	| 'main-checkout'
	| 'not-repository'
	| 'not-submitted'
	| 'session-stopping'
	| 'source-changed'
	| 'unsupported'
	| 'worker-unavailable';

/** A removal that stopped where it may safely continue. */
export interface WorkbenchWorktreeStoppedRemoval {
	readonly cause: WorkbenchWorktreeFailureCause;
	/** The authority's handle for continuing it; it accepts only removals it reported as stopped. */
	readonly operationId: string;
}

/**
 * A command names the Workspace and, where it acts on one, the checkout the person was shown. The
 * authority refuses it as `source-changed` once its latest discovery for that source no longer
 * offers that checkout. A `panel` makes the source that Panel's terminal rather than the Workspace's
 * first terminal.
 */
export type WorkbenchWorktreeCommand =
	/**
	 * Creates a checkout with Git's and the configuration's defaults and opens it in a Workspace. From
	 * a Panel, which showed the person nothing, the authority reads Git there first.
	 */
	| {
			readonly kind: 'create';
			readonly label: string;
			readonly panel?: WorkbenchPanelTerminal;
			readonly workspaceId: string;
	  }
	| {
			readonly kind: 'open';
			readonly panel?: WorkbenchPanelTerminal;
			readonly path: string;
			readonly workspaceId: string;
	  }
	/** Closes the Workspace, then deletes its checkout. The branch stays. */
	| { readonly kind: 'remove'; readonly path: string; readonly workspaceId: string }
	| { readonly kind: 'resume'; readonly operationId: string };

export type WorkbenchWorktreeOutcome =
	| { readonly kind: 'opened'; readonly workspaceId: string }
	| { readonly kind: 'removed' }
	| {
			readonly kind: 'failed';
			readonly cause: WorkbenchWorktreeFailureCause;
			readonly resumable: WorkbenchWorktreeStoppedRemoval | null;
	  }
	/** The command may have taken effect, so it must not be repeated; the operation can be inspected. */
	| { readonly kind: 'unknown'; readonly operationId: string };

/** Git Worktrees for Workspaces, decided by the authority that owns the Workbench. */
export interface WorkbenchWorktrees {
	/**
	 * Reads Git where the Workspace's first terminal stands, or where the terminal shown in `panel`
	 * stands, as the TUI's `Ctrl+B W` and `Ctrl+B O` read their focused Panel.
	 */
	discover(
		workspaceId: string,
		panel?: WorkbenchPanelTerminal,
	): Promise<WorkbenchWorktreeDiscoveryResult>;
	/** Settles when the operation does, or when it can no longer be observed. */
	run(command: WorkbenchWorktreeCommand): Promise<WorkbenchWorktreeOutcome>;
}
