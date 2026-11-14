import { z } from 'zod';
import {
	SessionIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchRequestIdSchema,
	WorkbenchStopIdSchema,
} from './workbench-semantic-contract';

/**
 * An admitted Client asks the Server to stop its Session under an identity it chose. Recording the
 * stop refuses every later record, so `server status` reads its progress and completion.
 */
export const WorkbenchSessionStopRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.session-stop'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		sessionId: SessionIdSchema,
		stopId: WorkbenchStopIdSchema,
	})
	.readonly();

const SessionStopOutcomeSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.enum(['recorded', 'busy', 'unmanaged', 'denied']) }).readonly(),
	z.strictObject({ kind: z.literal('rejected'), reason: z.literal('session-mismatch') }).readonly(),
]);
export const WorkbenchSessionStopResultSchema = z
	.strictObject({
		kind: z.literal('workbench.session-stop.result'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		outcome: SessionStopOutcomeSchema,
	})
	.readonly();
