import { z } from 'zod';
import {
	SessionIdSchema,
	WorkbenchDirectoryPathSchema,
	WorkbenchRequestIdSchema,
	WorkbenchWorkspaceIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';
/** A longest directory with every byte escaped to six JSON bytes, plus room for its identity. */
export const WORKBENCH_MAXIMUM_CURRENT_DIRECTORY_BYTES = 26 * 1024;
const identity = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	sessionId: SessionIdSchema,
	workspaceId: WorkbenchWorkspaceIdSchema,
};
export const CurrentDirectoryRequestSchema = z
	.strictObject({ ...identity, kind: z.literal('workbench.current-directory') })
	.readonly();
const CurrentDirectoryOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({ kind: z.literal('observed'), directory: WorkbenchDirectoryPathSchema })
		.readonly(),
	z
		.strictObject({
			kind: z.literal('failed'),
			reason: z.enum(['denied', 'not-found', 'unavailable']),
		})
		.readonly(),
]);
export const CurrentDirectoryResultSchema = z
	.strictObject({
		...identity,
		kind: z.literal('workbench.current-directory.result'),
		outcome: CurrentDirectoryOutcomeSchema,
	})
	.readonly();
