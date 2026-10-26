import { z } from 'zod';
import {
	SessionIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchRequestIdSchema,
	WorkbenchProjectIdSchema,
	WorkbenchWorkspaceIdSchema,
	WorkbenchDirectoryPathSchema,
	WorkbenchLabelSchema,
} from './workbench-semantic-contract';
import {
	WorkbenchTerminalGeometrySchema,
	WorkbenchTerminalOutcomeSchema,
} from './workbench-terminal-launch-contract';

export const WORKBENCH_MAXIMUM_ENCODED_WORKTREE_OPERATION_BYTES = 96 * 1024;
const WORKBENCH_MAXIMUM_WORKTREE_OPERATION_ATTEMPTS = 64;

const WorktreeReferenceSchema = z
	.string()
	.min(1)
	.max(1024)
	.refine(
		(value) =>
			!value.startsWith('-') &&
			!/[\uD800-\uDFFF]/u.test(value) &&
			[...value].every((character) => {
				const code = character.codePointAt(0);
				return code !== undefined && code >= 32 && code !== 127;
			}) &&
			new TextEncoder().encode(value).length <= 1024,
		'Git reference must be bounded UTF-8 without control characters or a leading dash',
	)
	.brand<'WorktreeReference'>();
const attemptSchema = z.number().int().min(1).max(WORKBENCH_MAXIMUM_WORKTREE_OPERATION_ATTEMPTS);
export const WorktreeSourceSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({ kind: z.literal('workspace'), workspaceId: WorkbenchWorkspaceIdSchema })
		.readonly(),
	z.strictObject({ kind: z.literal('directory'), path: WorkbenchDirectoryPathSchema }).readonly(),
]);
const WorktreeSelectorSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('path'), path: WorkbenchDirectoryPathSchema }).readonly(),
	z.strictObject({ kind: z.literal('branch'), branch: WorktreeReferenceSchema }).readonly(),
]);
const launch = {
	source: WorktreeSourceSchema,
	label: WorkbenchLabelSchema.optional(),
	geometry: WorkbenchTerminalGeometrySchema,
	trustRepository: z.boolean(),
};
const WorktreeIntentSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('create'),
			...launch,
			branch: WorktreeReferenceSchema.optional(),
			base: WorktreeReferenceSchema.optional(),
			path: WorkbenchDirectoryPathSchema.optional(),
		})
		.readonly(),
	z
		.strictObject({ kind: z.literal('open'), ...launch, selector: WorktreeSelectorSchema })
		.readonly(),
	z
		.strictObject({
			kind: z.literal('remove'),
			workspaceId: WorkbenchWorkspaceIdSchema,
			force: z.boolean(),
			trustRepository: z.boolean(),
		})
		.readonly(),
]);
const WorkbenchWorktreeActionSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('start'),
			operationId: WorkbenchRequestIdSchema,
			projectId: WorkbenchProjectIdSchema.nullable(),
			intent: WorktreeIntentSchema,
		})
		.refine(
			(value) =>
				value.projectId === null ||
				(value.intent.kind !== 'remove' && value.intent.source.kind === 'directory'),
			'Only a Directory source accepts an explicit destination Project',
		)
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
export const WorkbenchWorktreeRequestSchema = z
	.strictObject({
		kind: z.literal('workbench.worktree.operation'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		sessionId: SessionIdSchema,
		action: WorkbenchWorktreeActionSchema,
	})
	.readonly();
const WorktreeCheckoutFactsSchema = z
	.strictObject({
		repositoryRoot: WorkbenchDirectoryPathSchema,
		commonDirectory: WorkbenchDirectoryPathSchema,
		path: WorkbenchDirectoryPathSchema,
		branch: WorktreeReferenceSchema.nullable(),
		dirty: z.boolean(),
	})
	.readonly();
const WorktreeWorkspaceDispositionSchema = z.discriminatedUnion('kind', [
	z.strictObject({ kind: z.literal('unassociated') }).readonly(),
	z.strictObject({ kind: z.literal('open'), workspaceId: WorkbenchWorkspaceIdSchema }).readonly(),
	z
		.strictObject({
			kind: z.enum(['closing', 'closed']),
			workspaceId: WorkbenchWorkspaceIdSchema,
			closeRequestId: WorkbenchRequestIdSchema,
		})
		.readonly(),
]);
const WorktreeOperationFailureSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('git'),
			stage: z.enum(['probe', 'select', 'check-branch', 'dirty-check', 'mutate', 'verify']),
			error: z.enum([
				'git-failed',
				'spawn-unavailable',
				'spawn-failed',
				'invalid-repository',
				'checkout-not-found',
				'main-checkout',
				'dirty',
				'invalid-reference',
				'too-large',
				'timeout',
				'cancelled',
				'io',
				'repository-changed',
				'ambiguous-checkout',
			]),
			status: z
				.number()
				.int()
				.min(-2_147_483_648)
				.max(2_147_483_647)
				.transform((value) => (value === 0 ? 0 : value))
				.nullable(),
			diagnostics: z
				.array(
					z
						.number()
						.int()
						.min(0)
						.max(255)
						.transform((value) => (value === 0 ? 0 : value)),
				)
				.max(2048)
				.readonly(),
		})
		.refine(
			(failure) => failure.error === 'git-failed' || failure.status === null,
			'Only a Git process failure carries exit status',
		)
		.readonly(),
	z
		.strictObject({
			kind: z.enum([
				'worker-unavailable',
				'worker-busy',
				'worker-capacity',
				'unsupported',
				'transport-unknown',
				'physical-unknown',
				'source-changed',
				'checkout-in-use',
				'association-failed',
				'close-incomplete',
				'session-stopping',
				'internal',
			]),
		})
		.readonly(),
]);
const shellCountSchema = z
	.number()
	.int()
	.min(0)
	.max(4096)
	.transform((value) => (value === 0 ? 0 : value));
const WorktreeShellRestorationSchema = z
	.strictObject({
		total: shellCountSchema,
		attempted: shellCountSchema,
		started: shellCountSchema,
		failed: shellCountSchema,
		unknown: shellCountSchema,
		failure: WorktreeOperationFailureSchema.nullable(),
	})
	.refine((value) => {
		const completed = value.started + value.failed + value.unknown;
		return (
			completed <= value.attempted &&
			value.attempted <= value.total &&
			value.attempted - completed <= 1
		);
	}, 'Restoration counts must describe one serial launch prefix')
	.readonly();
const WorktreeProgressSchema = z
	.strictObject({
		path: WorkbenchDirectoryPathSchema.nullable(),
		checkout: WorktreeCheckoutFactsSchema.nullable(),
		workspace: WorktreeWorkspaceDispositionSchema,
		gitEffect: z.enum(['none', 'may-have-changed', 'changed']),
		restoration: WorktreeShellRestorationSchema.optional(),
	})
	.refine(
		(progress) => progress.checkout === null || progress.checkout.path === progress.path,
		'Checkout facts must match the retained path',
	)
	.readonly();
const WorktreeOperationStatusSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('pending'),
			phase: z.enum([
				'observing-source',
				'creating',
				'opening',
				'attaching',
				'inspecting-remove',
				'closing',
				'removing',
			]),
			progress: WorktreeProgressSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('opened'),
			checkout: WorktreeCheckoutFactsSchema,
			workspaceId: WorkbenchWorkspaceIdSchema,
			reused: z.boolean(),
			terminalOutcome: WorkbenchTerminalOutcomeSchema.nullable(),
		})
		.refine(
			(status) => status.reused === (status.terminalOutcome === null),
			'Only a reused Workspace omits terminal launch outcome',
		)
		.readonly(),
	z
		.strictObject({
			kind: z.literal('removed'),
			path: WorkbenchDirectoryPathSchema,
			workspaceId: WorkbenchWorkspaceIdSchema,
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('failed'),
			reason: WorktreeOperationFailureSchema,
			progress: WorktreeProgressSchema,
			resumable: z.boolean(),
		})
		.refine((status) => {
			const recovery = status.progress.restoration;
			return (
				recovery === undefined ||
				(recovery.attempted === recovery.started + recovery.failed + recovery.unknown &&
					(recovery.unknown === 0 || !status.resumable))
			);
		}, 'Final recovery has no pending launch and unknown launch is not resumable')
		.readonly(),
]);
export const WorkbenchWorktreeOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('available'),
			projectId: WorkbenchProjectIdSchema.nullable(),
			attempt: attemptSchema,
			status: WorktreeOperationStatusSchema,
		})
		.readonly(),
	z.strictObject({ kind: z.enum(['not-found', 'conflict', 'denied']) }).readonly(),
	z
		.strictObject({
			kind: z.literal('unavailable'),
			reason: z.enum([
				'busy',
				'capacity',
				'worker-unavailable',
				'source-unavailable',
				'service-unavailable',
				'unsupported',
			]),
		})
		.readonly(),
]);
export const WorkbenchWorktreeResultSchema = z
	.strictObject({
		kind: z.literal('workbench.worktree.operation.result'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		operationId: WorkbenchRequestIdSchema,
		outcome: WorkbenchWorktreeOutcomeSchema,
	})
	.readonly();
