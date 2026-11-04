import { z } from 'zod';
import {
	SessionIdSchema,
	TerminalPresentationIdSchema,
	WorkbenchDirectoryPathSchema,
	WorkbenchPanelIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchWorkspaceIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';
/** A longest directory with every byte escaped to six JSON bytes, plus room for its identity. */
export const WORKBENCH_MAXIMUM_CURRENT_DIRECTORY_BYTES = 26 * 1024;
/**
 * A Panel of the Workspace and the Presentation the asking Client shows in it, which is a child
 * terminal while one is displayed. Absent, the read is about the Workspace's first terminal.
 */
const CurrentDirectoryPanelSchema = z
	.strictObject({ panelId: WorkbenchPanelIdSchema, presentationId: TerminalPresentationIdSchema })
	.readonly();
const identity = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	sessionId: SessionIdSchema,
	workspaceId: WorkbenchWorkspaceIdSchema,
	panel: CurrentDirectoryPanelSchema.optional(),
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
