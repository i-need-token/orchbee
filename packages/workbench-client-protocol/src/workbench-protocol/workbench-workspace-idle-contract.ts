import { z } from 'zod';
import {
	SessionIdSchema,
	TerminalPresentationIdSchema,
	WORKBENCH_MAXIMUM_PANEL_NODES_PER_CONTEXT,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchPanelIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchTabIdSchema,
	WorkbenchWorkspaceIdSchema,
} from './workbench-semantic-contract';

/**
 * Exact encoded bound of a `workbench.workspace-idle.result` document with the protocol's maximum
 * of busy members at their longest reason. The Rust protocol asserts equality against the
 * constructed worst document.
 */
export const WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_IDLE_RESULT_BYTES = 659_699;

const WorkspaceIdleMemberSchema = z
	.strictObject({
		tabId: WorkbenchTabIdSchema,
		panelId: WorkbenchPanelIdSchema,
		presentationId: TerminalPresentationIdSchema,
		/**
		 * `running` when the shell reported a running command or another program holds the console,
		 * `unknown` when the evidence could not be read in time or did not tell.
		 */
		reason: z.enum(['running', 'unknown']),
	})
	.readonly();

const identity = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	sessionId: SessionIdSchema,
	workspaceId: WorkbenchWorkspaceIdSchema,
};
export const WorkspaceIdleRequestSchema = z
	.strictObject({ ...identity, kind: z.literal('workbench.workspace-idle') })
	.readonly();
export const WorkspaceIdleResultSchema = z
	.strictObject({
		...identity,
		kind: z.literal('workbench.workspace-idle.result'),
		outcome: z.discriminatedUnion('kind', [
			z.strictObject({ kind: z.literal('idle') }).readonly(),
			z
				.strictObject({
					kind: z.literal('busy'),
					/** Distinct Presentations, in the order a Close of the Workspace captures them. */
					members: z
						.array(WorkspaceIdleMemberSchema)
						.min(1)
						.max(WORKBENCH_MAXIMUM_PANEL_NODES_PER_CONTEXT)
						.readonly()
						.refine(
							(members) =>
								new Set(members.map((member) => member.presentationId)).size === members.length,
						),
				})
				.readonly(),
			z
				.strictObject({
					kind: z.literal('failed'),
					reason: z.enum(['denied', 'not-found', 'unavailable']),
				})
				.readonly(),
		]),
	})
	.readonly();
