import type { ActionExecutionContext, ActionResult } from '@orchbee/actions';

import type { ProjectWorkspaceEvent } from '../../project-workspace';
import type { WorkbenchProjectCreationReceipt } from './workbench-project-folder-acquisition';

/** A change to the Workbench structure every Client shares, in this Workbench's own vocabulary. */
export type WorkbenchAuthorityChange = Extract<
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

/** A Workspace with its first terminal. The authority names it and commits it as one change. */
export type WorkbenchAuthorityCreation =
	/** What New Workspace asks for: the authority chooses from this Client's current location. */
	| { readonly type: 'workspace-created-default' }
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
	  };

/** What an authority takes from the Action asking it: cancellation, and the commit point to mark. */
export type WorkbenchAuthorityContext = Pick<ActionExecutionContext, 'markCommitted' | 'signal'>;

/**
 * Whoever decides the shared Workbench when this Client does not. A result settles only once this
 * Client's read model holds what it reports, so the caller can act on what it reads next. A caller
 * never repeats a change whose outcome is unknown.
 */
export interface WorkbenchAuthority {
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
	/** Reports when what a confirmation of `closure` describes has changed. Returns how to stop. */
	watchClose(closure: WorkbenchAuthorityClosure, onChanged: () => void): () => void;
}
