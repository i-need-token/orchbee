import { z } from 'zod';
import {
	SessionIdSchema,
	WORKBENCH_MAXIMUM_TABS_PER_CONTEXT,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkbenchTabIdSchema,
	WorkbenchTerminalContextIdSchema,
} from './workbench-semantic-contract';
import {
	WorkbenchTerminalGeometrySchema,
	WorkbenchTerminalOutcomeSchema,
} from './workbench-terminal-launch-contract';

export const WORKBENCH_MAXIMUM_ENCODED_TAB_DUPLICATE_RESULT_BYTES = 256 * 1024;
const TabDuplicateActionSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('start'),
			operationId: WorkbenchRequestIdSchema,
			terminalContextId: WorkbenchTerminalContextIdSchema,
			tabIds: z
				.array(WorkbenchTabIdSchema)
				.min(1)
				.max(WORKBENCH_MAXIMUM_TABS_PER_CONTEXT)
				.refine((tabIds) => new Set(tabIds).size === tabIds.length, {
					message: 'A Tab appears twice in the operation',
				})
				.readonly(),
			expectedRevision: WorkbenchRevisionSchema,
			geometry: WorkbenchTerminalGeometrySchema,
		})
		.readonly(),
	z.strictObject({ kind: z.literal('read'), operationId: WorkbenchRequestIdSchema }).readonly(),
]);
export const WorkbenchTabDuplicateRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.tab-duplicate'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		sessionId: SessionIdSchema,
		action: TabDuplicateActionSchema,
	})
	.readonly();
const TabDuplicateCopySchema = z
	.strictObject({
		tabId: WorkbenchTabIdSchema,
		revision: WorkbenchRevisionSchema,
		terminal: WorkbenchTerminalOutcomeSchema.nullable(),
	})
	.readonly();
const TabDuplicateMemberSchema = z
	.strictObject({ tabId: WorkbenchTabIdSchema, copy: TabDuplicateCopySchema.nullable() })
	.readonly();
const TabDuplicateStatusSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.enum(['pending', 'duplicated']) }).readonly(),
	z
		.strictObject({
			kind: z.literal('failed'),
			reason: z.enum([
				'source-unavailable',
				'copy-refused',
				'terminal-not-started',
				'outcome-unknown',
				'deadline',
				'session-stopping',
				'internal',
			]),
		})
		.readonly(),
]);
const TabDuplicateOutcomeSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.enum(['not-found', 'conflict', 'denied']) }).readonly(),
	z
		.strictObject({
			kind: z.literal('rejected'),
			revision: WorkbenchRevisionSchema,
			reason: z.enum([
				'revision-mismatch',
				'terminal-context-unavailable',
				'tab-unavailable',
				'tab-order-mismatch',
				'resource-limit',
			]),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('unavailable'),
			reason: z.enum(['service-unavailable', 'busy', 'capacity']),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('available'),
			terminalContextId: WorkbenchTerminalContextIdSchema,
			status: TabDuplicateStatusSchema,
			members: z
				.array(TabDuplicateMemberSchema)
				.min(1)
				.max(WORKBENCH_MAXIMUM_TABS_PER_CONTEXT)
				.readonly(),
		})
		.readonly(),
]);
export const WorkbenchTabDuplicateResultSchema = z
	.strictObject({
		kind: z.literal('workbench.tab-duplicate.result'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		operationId: WorkbenchRequestIdSchema,
		outcome: TabDuplicateOutcomeSchema,
	})
	.superRefine((value, context) => {
		const { outcome } = value;
		if (outcome.kind !== 'available') return;
		const sources = new Set(outcome.members.map((member) => member.tabId));
		let valid = sources.size === outcome.members.length;
		let unfinished = false;
		const copies = new Set<string>();
		for (const member of outcome.members) {
			const { copy } = member;
			if (copy !== null) {
				if (unfinished || sources.has(copy.tabId) || copies.has(copy.tabId)) valid = false;
				copies.add(copy.tabId);
			}
			unfinished = copy?.terminal?.kind !== 'started';
		}
		if (outcome.status.kind === 'duplicated' && unfinished) valid = false;
		if (!valid)
			context.addIssue({
				code: 'custom',
				message: 'Invalid duplicate membership, copy ordering or completion truth',
			});
	})
	.readonly();
