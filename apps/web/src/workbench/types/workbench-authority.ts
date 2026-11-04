import type { ActionExecutionContext, ActionResult } from '@orchbee/actions';
import type { TabStripArrangementChange } from '@orchbee/tab-strip';
import type {
	UserLaunchProfile,
	UserLaunchProfileCatalog,
} from '@orchbee/workbench-client-protocol';

import type { ProjectWorkspaceEvent } from '../../project-workspace';
import type { WorkbenchAgentBoard } from './workbench-agent-board';
import type { WorkbenchConfiguredCommands } from './workbench-configured-commands';
import type { WorkbenchProjectCreationReceipt } from './workbench-project-folder-acquisition';
import type { WorkbenchWorktrees } from './workbench-worktrees';

/** A change the navigation states without an authority too, as one event of its own. */
export type WorkbenchAuthorityNavigationChange = Extract<
	ProjectWorkspaceEvent,
	{
		readonly type:
			| 'project-dissolved'
			| 'project-renamed'
			| 'project-reordered'
			| 'workspace-project-created'
			| 'workspace-project-set'
			| 'workspace-reordered'
			| 'workspace-updated';
	}
>;

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
	 * location, and the Workspace joins the Project named, or none.
	 */
	| { readonly projectId: string | null; readonly type: 'workspace-created-default' }
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

/** A terminal joining a Tab's Panels, committed as one change. */
export type WorkbenchAuthorityPanelTerminalCreation = WorkbenchAuthorityTabTarget & {
	/** Where the terminal starts. Without one, the authority chooses as it does for any new terminal. */
	readonly cwd?: string | undefined;
	/** The Panel that is split, or that gains the child. */
	readonly panelId: string;
} & (
		| { readonly type: 'panel-split'; readonly direction: WorkbenchAuthorityPanelDirection }
		| { readonly type: 'child-terminal' }
	);

export interface WorkbenchAuthorityPanelTerminalReceipt extends WorkbenchAuthorityTabTarget {
	/** The new Panel of a split, or the Panel that gained the child. */
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
	/** Closes in the given order, each waiting for the one before it, and stops at the first failure. */
	| {
			readonly type: 'tabs-close';
			readonly terminalContextId: string;
			readonly tabIds: readonly string[];
	  }
	/** Settles when the authority accepts it: the Panel leaves with a later publication. */
	| {
			readonly type: 'panel-close';
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
		context: WorkbenchAuthorityContext,
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
	/** Absent where every new Tab starts with the default launch profile. */
	readonly launchProfiles?: WorkbenchLaunchProfiles | undefined;
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
