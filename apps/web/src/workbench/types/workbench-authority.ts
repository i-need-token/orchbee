import type { ActionExecutionContext, ActionResult } from '@orchbee/actions';
import type { TabStripArrangementChange } from '@orchbee/tab-strip';
import type {
	UserLaunchProfile,
	UserLaunchProfileCatalog,
} from '@orchbee/workbench-client-protocol';

import type { WorkbenchAgentBoard } from './workbench-agent-board';
import type { WorkbenchConfiguredCommands } from './workbench-configured-commands';
import type { WorkbenchFocusedPanel } from './workbench-focused-panel';
import type { WorkbenchProjectCreationReceipt } from './workbench-project-folder-acquisition';
import type { WorkbenchWorktrees } from './workbench-worktrees';

/** A change to how Projects name, group, and order Workspaces. */
export type WorkbenchAuthorityNavigationChange =
	| { readonly type: 'project-dissolved'; readonly projectId: string }
	| { readonly type: 'project-renamed'; readonly projectId: string; readonly name: string }
	| {
			readonly type: 'project-reordered';
			readonly projectId: string;
			readonly beforeProjectId: string | null;
	  }
	| {
			readonly type: 'workspace-project-created';
			readonly workspaceId: string;
			readonly name: string;
	  }
	| {
			readonly type: 'workspace-project-set';
			readonly workspaceId: string;
			readonly projectId: string | null;
	  }
	/** Moves a Workspace to its place in a Project's list, or among those outside any Project. */
	| {
			readonly type: 'workspace-moved';
			readonly workspaceId: string;
			readonly projectId: string | null;
			readonly beforeWorkspaceId: string | null;
	  }
	| {
			readonly type: 'workspace-updated';
			readonly workspaceId: string;
			readonly name: string;
	  };

type WorkbenchAuthorityPanelDirection = 'above' | 'below' | 'left' | 'right';

/** Names a Tab, because a command can settle after this Client has moved to another. */
interface WorkbenchAuthorityTabTarget {
	readonly tabId: string;
	readonly terminalContextId: string;
}

/** A change to how the Panels of one Tab share its space, in every Client that shows it. */
export type WorkbenchAuthoritySplitRatioChange = WorkbenchAuthorityTabTarget &
	(
		| {
				readonly type: 'split-ratio-set';
				/** The first side's share, out of 100. */
				readonly leadingWeight: number;
				readonly splitId: string;
		  }
		/** Grows or shrinks the Panel's side of its nearest split on that axis. `reset` evens all of them. */
		| {
				readonly type: 'panel-size-adjusted';
				readonly adjustment: 'narrower' | 'reset' | 'shorter' | 'taller' | 'wider';
				readonly multiplier: 1 | 2;
				readonly panelId: string;
		  }
		/** Gives every Panel the same share along each axis, as far as the authority's limit allows. */
		| { readonly type: 'panels-equalized' }
	);

/** A change to the Workbench structure every Client shares, in this Workbench's own vocabulary. */
export type WorkbenchAuthorityChange =
	| WorkbenchAuthorityNavigationChange
	| WorkbenchAuthoritySplitRatioChange
	| (WorkbenchAuthorityTabTarget & {
			readonly type: 'panel-renamed';
			readonly panelId: string;
			readonly label: string | null;
	  })
	| {
			readonly type: 'tab-renamed';
			readonly label: string;
			readonly tabId: string;
			readonly terminalContextId: string;
	  }
	| {
			readonly type: 'tabs-changed';
			readonly change: TabStripArrangementChange;
			readonly terminalContextId: string;
	  }
	| (WorkbenchAuthorityTabTarget & {
			readonly type: 'panel-moved';
			readonly direction: WorkbenchAuthorityPanelDirection;
			readonly panelId: string;
			readonly targetPanelId: string;
	  })
	| (WorkbenchAuthorityTabTarget & {
			readonly type: 'panels-swapped';
			readonly panelId: string;
			readonly targetPanelId: string;
	  });

/** A Workspace with its first terminal. The authority names it and commits it as one change. */
export type WorkbenchAuthorityCreation =
	/**
	 * What New Workspace asks for: the authority chooses the directory from this Client's current
	 * location, and the Workspace joins the Project named, or none. A command that names no row
	 * carries the focused Panel, whose terminal says where the person works; a Project row carries
	 * none, so the focused Workspace's first terminal answers.
	 */
	| {
			readonly focusedPanel: WorkbenchFocusedPanel | null;
			readonly projectId: string | null;
			readonly type: 'workspace-created-default';
	  }
	| {
			readonly type: 'workspace-created-at-path';
			readonly path: string;
			readonly projectId: string;
	  }
	| {
			readonly type: 'workspace-created';
			readonly directoryId: string;
			readonly projectId: string | null;
	  }
	| {
			readonly type: 'project-created';
			readonly path: string;
			readonly projectName: string;
	  };

/** A Tab with its first terminal, committed as one change. */
export interface WorkbenchAuthorityTabCreation {
	/** Where the terminal starts. Without one, the authority chooses as it does for any new Tab. */
	readonly cwd?: string | undefined;
	/** Null goes last, in no group. */
	readonly placement:
		| { readonly kind: 'after-tab'; readonly tabId: string }
		/** Follows its source with the same name, pin, and group, and none of its terminals. */
		| { readonly kind: 'duplicate'; readonly tabId: string }
		| { readonly groupId: string; readonly kind: 'in-group' }
		| { readonly kind: 'new-group' }
		| null;
	/**
	 * Starts the terminal with this saved launch profile instead of the default, once, without
	 * changing the default. One no longer saved as shown, or that this Client cannot start, settles
	 * `blocked` with `launch-profile-unavailable` and creates nothing.
	 */
	readonly profile?: UserLaunchProfile | undefined;
	readonly terminalContextId: string;
}

/** Whether a Panel this Client creates takes the focus, as the Session's configuration sets it now. */
export interface WorkbenchFocusNewPanelSource {
	current(): boolean;
}

/** The saved launch profiles a new Tab can start with, as every Client of the Session shares them. */
export interface WorkbenchLaunchProfiles {
	/** Holds its identity until the catalog changes. */
	getSnapshot(): Pick<UserLaunchProfileCatalog, 'defaultProfileId' | 'profiles'>;
	subscribe(listener: () => void): () => void;
}

export interface WorkbenchAuthorityTabReceipt {
	readonly tabId: string;
	readonly terminalContextId: string;
	/** False when the Tab exists and its terminal is still starting, failed, or may not have started. */
	readonly terminalStarted: boolean;
}

/** Several Tabs of one terminal context, each copied right after itself, in the order shown. */
export interface WorkbenchAuthorityTabDuplication {
	readonly tabIds: readonly string[];
	readonly terminalContextId: string;
}

/** Why a Duplicate left some of its Tabs uncopied. */
export type WorkbenchTabDuplicationStop =
	| 'changed'
	| 'deadline'
	| 'not-created'
	| 'terminal-not-started';

/** How a duplication of several Tabs ended: every copy started, or where it stopped. */
export type WorkbenchAuthorityTabDuplicationReport =
	| { readonly kind: 'duplicated' }
	| {
			/** Copies that exist, whether or not their terminals started. */
			readonly created: number;
			readonly kind: 'stopped';
			readonly stop: WorkbenchTabDuplicationStop;
			readonly total: number;
	  };

/** A terminal joining a Tab's Panels by a split, committed as one change. */
export interface WorkbenchAuthorityPanelTerminalCreation extends WorkbenchAuthorityTabTarget {
	/** Where the terminal starts. Without one, the authority chooses as it does for any new terminal. */
	readonly cwd?: string | undefined;
	readonly direction: WorkbenchAuthorityPanelDirection;
	/** The Panel that is split. */
	readonly panelId: string;
}

export interface WorkbenchAuthorityPanelTerminalReceipt extends WorkbenchAuthorityTabTarget {
	/** The new Panel of the split. */
	readonly panelId: string;
	readonly presentationId: string;
	/** False when the terminal is still starting, failed, or may not have started. */
	readonly terminalStarted: boolean;
}

/** Something whose terminals end for every Client. */
export type WorkbenchAuthorityClosure =
	/** Closes only what runs nothing; otherwise settles `blocked` with `confirmation-required`. */
	| { readonly type: 'workspace-close-if-idle'; readonly workspaceId: string }
	| { readonly type: 'workspace-close'; readonly workspaceId: string }
	| { readonly type: 'project-workspaces-close'; readonly projectId: string }
	/**
	 * Closes in the given order, each waiting for the one before it, and stops at the first failure.
	 * Several Tabs close as one operation of the authority, which this Client only follows.
	 */
	| {
			readonly type: 'tabs-close';
			readonly terminalContextId: string;
			readonly tabIds: readonly string[];
	  }
	/**
	 * As `tabs-close`, only if no Tab runs anything; otherwise settles `blocked` with
	 * `confirmation-required`.
	 */
	| {
			readonly type: 'tabs-close-if-idle';
			readonly terminalContextId: string;
			readonly tabIds: readonly string[];
	  }
	/**
	 * Settles when the authority accepts it, and the Panel leaves with a later publication. A Tab's
	 * last Panel takes the Tab with it, and that close settles as a Tab's close does.
	 */
	| {
			readonly type: 'panel-close';
			readonly terminalContextId: string;
			readonly tabId: string;
			readonly panelId: string;
	  }
	/** As `panel-close`, only if the Panel runs nothing; otherwise as `tabs-close-if-idle` refuses. */
	| {
			readonly type: 'panel-close-if-idle';
			readonly terminalContextId: string;
			readonly tabId: string;
			readonly panelId: string;
	  }
	/**
	 * Continues the Tab's close that {@link WorkbenchAuthority.stoppedTabClose} reports, and never
	 * starts another one: `blocked` with `scope-changed` when there is none to continue.
	 */
	| {
			readonly type: 'tab-close-resume';
			readonly terminalContextId: string;
			readonly tabId: string;
	  };

/** Where a Tab's close stopped when the authority may continue it, as its status said. */
export type WorkbenchTabCloseStop = 'awaiting-removal' | 'cleanup-blocked';

/** What an authority takes from the Action asking it: cancellation, and the commit point to mark. */
export type WorkbenchAuthorityContext = Pick<ActionExecutionContext, 'markCommitted' | 'signal'>;

/**
 * What a close takes besides cancellation and the commit point. A Workspace or Project close calls
 * `markLeft` once, when the Server accepts it and its Workspaces leave this Client's view, before
 * the close settles; a later failure or lost outcome shows them again with its result. The close of
 * one Tab, its last Panel's, or its Resume calls it as the Tab leaves the strip: at the acceptance,
 * or, for a close that joins one that stopped, once a read finds it on its way again. A set of Tabs
 * and a Panel of several never call it.
 */
export interface WorkbenchAuthorityCloseContext extends WorkbenchAuthorityContext {
	readonly markLeft?: (() => void) | undefined;
}

/** How starting the terminal of a Presentation a restore held for an unavailable directory ended. */
export type WorkbenchHeldRestoreOutcome =
	/** The terminal was created; a failed launch shows its own guidance in the Panel. */
	| { readonly agentNotResumed: boolean; readonly kind: 'created' }
	/** Retry found the saved directory still unavailable, so the Panel stays held. */
	| { readonly kind: 'directory-unavailable' }
	/** Nothing started: the hold changed, the Server refused, or the outcome is unknown. */
	| { readonly kind: 'not-started' };

/** Starts the terminal of a held restore in its saved directory or the home folder, once. */
export interface WorkbenchHeldRestores {
	restore(
		presentationId: string,
		directory: 'home' | 'saved',
	): Promise<WorkbenchHeldRestoreOutcome>;
}

/**
 * Whoever decides the shared Workbench when this Client does not. A result settles only once this
 * Client's read model holds what it reports, so the caller can act on what it reads next. A caller
 * never repeats a change whose outcome is unknown.
 */
export interface WorkbenchAuthority {
	/** The smallest share either side of a split can hold, out of 100. */
	readonly minimumSplitPercent: number;
	/** Absent where this authority cannot read the Server's Agent sessions. */
	readonly agentBoard?: WorkbenchAgentBoard | undefined;
	/** Absent where this authority runs no configured commands. */
	readonly configuredCommands?: WorkbenchConfiguredCommands | undefined;
	change(
		context: WorkbenchAuthorityContext,
		change: WorkbenchAuthorityChange,
	): Promise<ActionResult<void>>;
	/**
	 * `blocked` means nothing closed, with `scope-changed` when the target is no longer what the person
	 * was shown. `failed` keeps what is known: `outcome-unknown`, or how a close that was accepted
	 * stopped. Its details hold the operation identity when the target alone cannot name it.
	 */
	close(
		context: WorkbenchAuthorityCloseContext,
		closure: WorkbenchAuthorityClosure,
	): Promise<ActionResult<void>>;
	/** A completed creation without a receipt exists, but this Client can no longer read it. */
	create(
		context: WorkbenchAuthorityContext,
		creation: WorkbenchAuthorityCreation,
	): Promise<ActionResult<WorkbenchProjectCreationReceipt>>;
	/** A completed creation without a receipt exists, but this Client can no longer read it. */
	createPanelTerminal(
		context: WorkbenchAuthorityContext,
		creation: WorkbenchAuthorityPanelTerminalCreation,
	): Promise<ActionResult<WorkbenchAuthorityPanelTerminalReceipt>>;
	/** A completed creation without a receipt exists, but this Client can no longer read it. */
	createTab(
		context: WorkbenchAuthorityContext,
		creation: WorkbenchAuthorityTabCreation,
	): Promise<ActionResult<WorkbenchAuthorityTabReceipt>>;
	/**
	 * Copies several Tabs as one operation that copies them one at a time and stops at the first it
	 * cannot copy with a started terminal. Nothing is repeated, and no Client can cancel it once
	 * started; a completed report says how it ended.
	 */
	duplicateTabs(
		context: WorkbenchAuthorityContext,
		duplication: WorkbenchAuthorityTabDuplication,
	): Promise<ActionResult<WorkbenchAuthorityTabDuplicationReport>>;
	/** Absent where no restore holds a Presentation for an unavailable directory. */
	readonly heldRestores?: WorkbenchHeldRestores | undefined;
	/** Absent where every new Tab starts with the default launch profile. */
	readonly launchProfiles?: WorkbenchLaunchProfiles | undefined;
	/** Absent where every new Panel takes the focus, as the default configuration sets it. */
	readonly focusNewPanel?: WorkbenchFocusNewPanelSource | undefined;
	/**
	 * Where the last close of this Tab that this Client followed stopped, while a `tab-close-resume`
	 * may continue it; null otherwise.
	 */
	stoppedTabClose(terminalContextId: string, tabId: string): WorkbenchTabCloseStop | null;
	/** Reports when what a confirmation of `closure` describes has changed. Returns how to stop. */
	watchClose(closure: WorkbenchAuthorityClosure, onChanged: () => void): () => void;
	/** Absent where this authority cannot reach Git for its Workspaces. */
	readonly worktrees?: WorkbenchWorktrees | undefined;
}
