import { z } from 'zod';
import {
	SessionIdSchema,
	WORKBENCH_MAXIMUM_TABS_PER_CONTEXT,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchLabelSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkbenchTabIdSchema,
	WorkbenchTerminalContextIdSchema,
} from './workbench-semantic-contract';
import {
	WorkbenchTabCloseAttemptGenerationSchema,
	WorkbenchTabCloseStatusSchema,
} from './workbench-tab-close-contract';

export const WORKBENCH_MAXIMUM_ENCODED_TAB_SET_CLOSE_RESULT_BYTES = 1024 * 1024;
const TabSetCloseConditionSchema = z.enum(['if-idle', 'confirmed']);
const TabSetCloseActionSchema = z.discriminatedUnion('kind', [
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
					message: 'A Tab appears twice in the set',
				})
				.readonly(),
			expectedRevision: WorkbenchRevisionSchema,
			condition: TabSetCloseConditionSchema,
		})
		.readonly(),
	z.strictObject({ kind: z.literal('read'), operationId: WorkbenchRequestIdSchema }).readonly(),
]);
export const WorkbenchTabSetCloseRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.tab-set-close'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		sessionId: SessionIdSchema,
		action: TabSetCloseActionSchema,
	})
	.readonly();
const TabSetCloseChildSchema = z
	.strictObject({
		requestId: WorkbenchRequestIdSchema,
		observation: z
			.strictObject({
				attemptGeneration: WorkbenchTabCloseAttemptGenerationSchema,
				status: WorkbenchTabCloseStatusSchema,
			})
			.readonly()
			.nullable(),
	})
	.readonly();
const TabSetCloseMemberSchema = z
	.strictObject({
		tabId: WorkbenchTabIdSchema,
		label: WorkbenchLabelSchema,
		close: TabSetCloseChildSchema.nullable(),
		absent: z.boolean(),
	})
	.readonly();
const TabSetCloseStatusSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.enum(['pending', 'closed']) }).readonly(),
	z
		.strictObject({
			kind: z.literal('failed'),
			reason: z.enum([
				'source-changed',
				'worker-unavailable',
				'child-busy',
				'child-admission',
				'child-blocked',
				'outcome-unknown',
				'deadline',
				'session-stopping',
				'internal',
			]),
		})
		.readonly(),
]);
const TabSetCloseOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({ kind: z.enum(['confirmation-required', 'not-found', 'conflict', 'denied']) })
		.readonly(),
	z
		.strictObject({
			kind: z.literal('rejected'),
			revision: WorkbenchRevisionSchema,
			reason: z.enum([
				'revision-mismatch',
				'terminal-context-unavailable',
				'tab-unavailable',
				'tab-order-mismatch',
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
			condition: TabSetCloseConditionSchema,
			status: TabSetCloseStatusSchema,
			members: z
				.array(TabSetCloseMemberSchema)
				.min(1)
				.max(WORKBENCH_MAXIMUM_TABS_PER_CONTEXT)
				.readonly(),
		})
		.readonly(),
]);
export const WorkbenchTabSetCloseResultSchema = z
	.strictObject({
		kind: z.literal('workbench.tab-set-close.result'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		operationId: WorkbenchRequestIdSchema,
		outcome: TabSetCloseOutcomeSchema,
	})
	.superRefine((value, context) => {
		const { outcome } = value;
		if (outcome.kind !== 'available') return;
		let valid = true;
		let unfinished = false;
		const tabs = new Set<string>();
		const requests = new Set<string>();
		for (const member of outcome.members) {
			const child = member.close;
			if (tabs.has(member.tabId) || (member.absent && child !== null)) valid = false;
			tabs.add(member.tabId);
			if (unfinished && (member.absent || child !== null)) valid = false;
			if (child !== null) {
				if (child.requestId === value.operationId || requests.has(child.requestId)) valid = false;
				requests.add(child.requestId);
			}
			unfinished = !member.absent && child?.observation?.status.kind !== 'closed';
		}
		if (outcome.status.kind === 'closed' && unfinished) valid = false;
		if (!valid)
			context.addIssue({
				code: 'custom',
				message: 'Invalid set membership, child ordering or completion truth',
			});
	})
	.readonly();
