import type { ActionExecutionContext, ActionResult } from '@orchbee/actions';
import type { TabStripSelectCallback } from '@orchbee/tab-strip';

import type { WorkbenchTerminalCreateInput } from '../actions/workbench-action-ids';

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
	readonly navigate: (
		context: ActionExecutionContext,
		target: WorkbenchNavigationTarget,
	) => WorkbenchHostActionResult;
	readonly selectTab: TabStripSelectCallback;
}

export type WorkbenchHostActionCallback = (
	context: ActionExecutionContext,
) => WorkbenchHostActionResult;

export type WorkbenchTerminalCreateCallback = (
	context: ActionExecutionContext,
	input: WorkbenchTerminalCreateInput,
) => WorkbenchHostActionResult;
