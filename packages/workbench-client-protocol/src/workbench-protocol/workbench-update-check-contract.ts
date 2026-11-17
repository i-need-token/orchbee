import { z } from 'zod';
import {
	SessionIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchRequestIdSchema,
} from './workbench-semantic-contract';

/**
 * An admitted Client asks the Session Server to run its automatic-update job now (`start`), or
 * reads where that job stands (`read`). Both answer at once; a run's end is read, never awaited.
 */
export const WorkbenchUpdateCheckRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.update-check'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		sessionId: SessionIdSchema,
		action: z.enum(['start', 'read']),
	})
	.readonly();

const UpdateCheckOutcomeSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.enum(['checking', 'idle', 'unavailable', 'denied']) }).readonly(),
	z
		.strictObject({
			kind: z.literal('finished'),
			result: z.enum(['up-to-date', 'installed', 'failed']),
		})
		.readonly(),
	z.strictObject({ kind: z.literal('rejected'), reason: z.literal('session-mismatch') }).readonly(),
]);
export const WorkbenchUpdateCheckResultSchema = z
	.strictObject({
		kind: z.literal('workbench.update-check.result'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		outcome: UpdateCheckOutcomeSchema,
	})
	.readonly();
