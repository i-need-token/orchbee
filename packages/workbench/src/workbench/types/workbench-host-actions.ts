import type { ActionExecutionContext, ActionResult } from '@orchbee/actions';

import type { WorkbenchTerminalCreateInput } from '../actions/workbench-action-ids';

export type WorkbenchHostActionResult<Output = void> =
	| ActionResult<Output>
	| Promise<ActionResult<Output>>;

export type WorkbenchHostActionCallback = (
	context: ActionExecutionContext,
) => WorkbenchHostActionResult;

export type WorkbenchTerminalCreateCallback = (
	context: ActionExecutionContext,
	input: WorkbenchTerminalCreateInput,
) => WorkbenchHostActionResult;
