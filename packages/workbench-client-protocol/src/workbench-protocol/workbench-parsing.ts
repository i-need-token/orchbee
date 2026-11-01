import type { z } from 'zod';

import {
	WorkbenchActionSchema,
	WorkbenchRequestSchema,
	WorkbenchResponseSchema,
	type WorkbenchCommand,
	type WorkbenchRequest,
	type WorkbenchResponse,
} from './workbench-command-contract';

/** A Client names identities as text; parsing admits them and applies the action's limits. */
export function parseWorkbenchAction(
	input: z.input<typeof WorkbenchActionSchema>,
): WorkbenchCommand['action'] {
	return WorkbenchActionSchema.parse(input);
}

export function parseWorkbenchRequest(input: unknown): WorkbenchRequest {
	return WorkbenchRequestSchema.parse(input);
}

export function parseWorkbenchResponse(input: unknown): WorkbenchResponse {
	return WorkbenchResponseSchema.parse(input);
}
