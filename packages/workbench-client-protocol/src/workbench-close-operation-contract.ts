import { z } from 'zod';
import {
	SiteIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchTabIdSchema,
	WorkbenchTerminalContextIdSchema,
	WorkbenchWorkspaceIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';
import {
	WorkbenchTabCloseAttemptGenerationSchema,
	WorkbenchTabCloseStatusSchema,
} from './workbench-tab-close-contract';

const correlation = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	siteId: SiteIdSchema,
	operationId: WorkbenchRequestIdSchema,
};
const target = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('workspace'), workspaceId: WorkbenchWorkspaceIdSchema }),
	z.strictObject({
		kind: z.literal('tab'),
		terminalContextId: WorkbenchTerminalContextIdSchema,
		tabId: WorkbenchTabIdSchema,
	}),
	z.strictObject({
		kind: z.literal('panel'),
		terminalContextId: WorkbenchTerminalContextIdSchema,
		tabId: WorkbenchTabIdSchema,
	}),
]);

export const CloseOperationReadSchema = z
	.strictObject({
		kind: z.literal('workbench.close-operation.read'),
		...correlation,
	})
	.readonly();
export const CloseOperationResultSchema = z
	.strictObject({
		kind: z.literal('workbench.close-operation.result'),
		...correlation,
		outcome: z.discriminatedUnion('kind', [
			z.strictObject({ kind: z.literal('not-found') }),
			z.strictObject({ kind: z.literal('denied') }),
			z.strictObject({ kind: z.literal('unavailable') }),
			z.strictObject({
				kind: z.literal('available'),
				target,
				generation: WorkbenchTabCloseAttemptGenerationSchema,
				status: WorkbenchTabCloseStatusSchema,
			}),
		]),
	})
	.readonly();
