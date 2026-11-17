import { z } from 'zod';
import {
	SessionIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';

const WorkbenchTerminalRuntimeSchema = z
	.strictObject({
		/** Starts at 1 and advances on each change of the state. */
		revision: WorkbenchRevisionSchema.refine((revision) => revision !== '0'),
		/**
		 * `serving` while a Worker serves the terminals, `reconnecting` while the Server replaces a
		 * lost one, `given-up` once it stopped trying until the Session stops.
		 */
		state: z.enum(['serving', 'reconnecting', 'given-up']),
	})
	.readonly();

const identity = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	sessionId: SessionIdSchema,
};
export const TerminalRuntimeRequestSchema = z
	.strictObject({ ...identity, kind: z.literal('workbench.terminal-runtime') })
	.readonly();
export const TerminalRuntimeResultSchema = z
	.strictObject({
		...identity,
		kind: z.literal('workbench.terminal-runtime.result'),
		outcome: z.discriminatedUnion('kind', [
			z
				.strictObject({ kind: z.literal('current'), runtime: WorkbenchTerminalRuntimeSchema })
				.readonly(),
			z
				.strictObject({ kind: z.literal('failed'), reason: z.enum(['denied', 'unavailable']) })
				.readonly(),
		]),
	})
	.readonly();
