import { z } from 'zod';
import {
	SessionIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchRequestIdSchema,
	WorkbenchProjectIdSchema,
	WorkbenchWorkspaceIdSchema,
	WorkbenchLabelSchema,
} from './workbench-semantic-contract';
import {
	WorkbenchTabCloseAttemptGenerationSchema,
	WorkbenchTabCloseStatusSchema,
} from './workbench-tab-close-contract';

export const WORKBENCH_MAXIMUM_ENCODED_WORKSPACE_GROUP_CLOSE_RESULT_BYTES = 1024 * 1024;
const WORKBENCH_MAXIMUM_WORKSPACE_GROUP_CLOSE_MEMBERS = 256;
const WORKBENCH_MAXIMUM_WORKSPACE_GROUP_CLOSE_ATTEMPTS = 64;
const attemptSchema = z.number().int().min(1).max(WORKBENCH_MAXIMUM_WORKSPACE_GROUP_CLOSE_ATTEMPTS);
const WorkspaceGroupCloseActionSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('start'),
			operationId: WorkbenchRequestIdSchema,
			projectId: WorkbenchProjectIdSchema,
		})
		.readonly(),
	z.strictObject({ kind: z.literal('read'), operationId: WorkbenchRequestIdSchema }).readonly(),
	z
		.strictObject({
			kind: z.literal('resume'),
			operationId: WorkbenchRequestIdSchema,
			expectedAttempt: attemptSchema,
		})
		.readonly(),
]);
export const WorkbenchWorkspaceGroupCloseRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.workspace-group-close'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		sessionId: SessionIdSchema,
		action: WorkspaceGroupCloseActionSchema,
	})
	.readonly();
const WorkspaceGroupCloseObservationSchema = z
	.strictObject({
		attemptGeneration: WorkbenchTabCloseAttemptGenerationSchema,
		status: WorkbenchTabCloseStatusSchema,
	})
	.readonly();
const WorkspaceGroupCloseChildSchema = z
	.strictObject({
		requestId: WorkbenchRequestIdSchema,
		observation: WorkspaceGroupCloseObservationSchema.nullable(),
	})
	.readonly();
const WorkspaceGroupCloseMemberSchema = z
	.strictObject({
		workspaceId: WorkbenchWorkspaceIdSchema,
		label: WorkbenchLabelSchema,
		close: WorkspaceGroupCloseChildSchema.nullable(),
	})
	.readonly();
const WorkspaceGroupCloseFailureSchema = z.enum([
	'source-changed',
	'worker-unavailable',
	'child-admission',
	'child-blocked',
	'outcome-unknown',
	'deadline',
	'session-stopping',
	'internal',
]);
const WorkspaceGroupCloseStatusSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.enum(['pending', 'closed']) }).readonly(),
	z
		.strictObject({
			kind: z.literal('failed'),
			reason: WorkspaceGroupCloseFailureSchema,
			resumable: z.boolean(),
		})
		.readonly(),
]);
const WorkspaceGroupCloseUnavailableSchema = z.enum([
	'unsupported',
	'service-unavailable',
	'busy',
	'capacity',
	'source-unavailable',
	'ambiguous-membership',
	'worker-unavailable',
	'source-changed',
	'not-resumable',
	'stale-attempt',
	'attempt-exhausted',
]);
const WorkspaceGroupCloseOutcomeSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.enum(['not-found', 'conflict', 'denied']) }).readonly(),
	z
		.strictObject({ kind: z.literal('unavailable'), reason: WorkspaceGroupCloseUnavailableSchema })
		.readonly(),
	z
		.strictObject({
			kind: z.literal('available'),
			projectId: WorkbenchProjectIdSchema,
			attempt: attemptSchema,
			status: WorkspaceGroupCloseStatusSchema,
			members: z
				.array(WorkspaceGroupCloseMemberSchema)
				.max(WORKBENCH_MAXIMUM_WORKSPACE_GROUP_CLOSE_MEMBERS)
				.readonly(),
		})
		.readonly(),
]);
export const WorkbenchWorkspaceGroupCloseResultSchema = z
	.strictObject({
		kind: z.literal('workbench.workspace-group-close.result'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		operationId: WorkbenchRequestIdSchema,
		outcome: WorkspaceGroupCloseOutcomeSchema,
	})
	.superRefine((value, context) => {
		const { outcome } = value;
		if (outcome.kind !== 'available') return;
		let valid = true;
		let blocked = false;
		let resumable = true;
		const workspaces = new Set<string>();
		const requests = new Set<string>();
		for (const member of outcome.members) {
			if (workspaces.has(member.workspaceId)) valid = false;
			workspaces.add(member.workspaceId);
			const child = member.close;
			if (child === null) {
				blocked = true;
				continue;
			}
			if (blocked || child.requestId === value.operationId || requests.has(child.requestId))
				valid = false;
			requests.add(child.requestId);
			const status = child.observation?.status;
			blocked = status?.kind !== 'closed';
			if (
				status === undefined ||
				status.kind === 'outcome-unknown' ||
				((status.kind === 'cleanup-blocked' || status.kind === 'disposed-awaiting-publication') &&
					!status.resumable)
			)
				resumable = false;
		}
		if (outcome.status.kind === 'closed' && blocked) valid = false;
		if (
			outcome.status.kind === 'failed' &&
			outcome.status.resumable &&
			(!resumable ||
				!['worker-unavailable', 'child-admission', 'child-blocked', 'deadline'].includes(
					outcome.status.reason,
				))
		)
			valid = false;
		if (!valid)
			context.addIssue({
				code: 'custom',
				message: 'Invalid group membership, child ordering or recovery truth',
			});
	})
	.readonly();
