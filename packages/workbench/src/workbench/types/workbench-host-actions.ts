import type { ActionExecutionContext, ActionResult } from '@orchbee/actions';
import type { TabStripSelectCallback } from '@orchbee/tab-strip';

import type { WorkbenchTerminalCreateInput } from '../actions/workbench-action-ids';
import type { CurrentDirectoryTerminalTarget } from '../model/resolve-current-directory-terminal-target';
import type { WorkbenchProjectCreationReceipt } from './workbench-project-folder-acquisition';

export type WorkbenchHostActionResult<Output = void> =
	| ActionResult<Output>
	| Promise<ActionResult<Output>>;

export type WorkbenchNavigationTarget =
	| { readonly kind: 'standalone' }
	| {
			readonly kind: 'workspace';
			readonly projectId: string;
			readonly workspaceId: string;
	  };

export interface WorkbenchNavigationPort {
	/** The currently accepted route, rather than a pending navigation request. */
	readonly isActive?: ((target: WorkbenchNavigationTarget) => boolean) | undefined;
	/** With a creation receipt, completion waits for exact route acceptance. */
	readonly navigate: (
		context: ActionExecutionContext,
		target: WorkbenchNavigationTarget,
		creationReceipt?: WorkbenchProjectCreationReceipt | undefined,
	) => WorkbenchHostActionResult;
	readonly selectTab: TabStripSelectCallback;
}

export type WorkbenchHostActionCallback = (
	context: ActionExecutionContext,
) => WorkbenchHostActionResult;

export type WorkbenchTerminalFontSizeChange = 'decrease' | 'increase' | 'reset';

export type WorkbenchTerminalFontSizeChangeCallback = (
	context: ActionExecutionContext,
	change: WorkbenchTerminalFontSizeChange,
) => WorkbenchHostActionResult;

export type WorkbenchTerminalCreateRequest =
	| (WorkbenchTerminalCreateInput & { readonly currentDirectoryTarget?: never })
	| (Omit<WorkbenchTerminalCreateInput, 'directoryId'> & {
			readonly currentDirectoryTarget: CurrentDirectoryTerminalTarget;
			readonly directoryId?: never;
	  });

export type WorkbenchTerminalCreateCallback = (
	context: ActionExecutionContext,
	input: WorkbenchTerminalCreateRequest,
) => WorkbenchHostActionResult;
