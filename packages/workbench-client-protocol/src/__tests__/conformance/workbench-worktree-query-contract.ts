import { z } from 'zod';
import {
	SessionIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
	WorkbenchProjectIdSchema,
	WorkbenchWorkspaceIdSchema,
	WorkbenchDirectoryPathSchema,
	WorkbenchLabelSchema,
	WorkerIdSchema,
} from './workbench-semantic-contract';
import { WorktreeSourceSchema } from './workbench-worktree-contract';

export const WORKBENCH_MAXIMUM_ENCODED_WORKTREE_QUERY_RESULT_BYTES = 32 * 1024;
const stageSchema = z.enum(['probe', 'list']);
const branchSchema = z
	.string()
	.min(12)
	.startsWith('refs/heads/')
	.refine(
		(value) => !value.includes('\0') && !/[\uD800-\uDFFF]/u.test(value),
		'Git branch must be valid UTF-8 without NUL',
	);
const checkoutSchema = z
	.strictObject({
		worktreePath: WorkbenchDirectoryPathSchema,
		head: z
			.string()
			.regex(/^(?:[0-9a-f]{40}|[0-9a-f]{64})$/u)
			.nullable()
			.optional()
			.transform((value) => value ?? null),
		branch: branchSchema
			.nullable()
			.optional()
			.transform((value) => value ?? null),
		bare: z.boolean(),
		detached: z.boolean(),
		locked: z.boolean(),
		prunable: z.boolean(),
	})
	.refine(
		(value) =>
			value.bare
				? value.head === null && value.branch === null && !value.detached
				: value.head !== null && value.detached === (value.branch === null),
		'Bare and detached Git facts must agree with HEAD and branch',
	)
	.readonly();

const WorktreeQuerySourceSchema = z
	.strictObject({
		requestedPath: WorkbenchDirectoryPathSchema,
		repositoryRoot: WorkbenchDirectoryPathSchema,
		commonDirectory: WorkbenchDirectoryPathSchema,
		checkoutPath: WorkbenchDirectoryPathSchema,
		workspaceId: WorkbenchWorkspaceIdSchema.nullable(),
	})
	.readonly();
const WorktreeWorkspaceAssociationSchema = z
	.strictObject({
		projectId: WorkbenchProjectIdSchema.nullable(),
		workspaceId: WorkbenchWorkspaceIdSchema,
		label: WorkbenchLabelSchema,
	})
	.readonly();
const WorktreeQueryEntrySchema = z
	.strictObject({
		checkout: checkoutSchema,
		linked: z.boolean(),
		workspaces: z.array(WorktreeWorkspaceAssociationSchema).readonly(),
	})
	.readonly();
const WorktreeQueryFailureSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.enum([
				'not-found',
				'denied',
				'unsupported',
				'source-changed',
				'timeout',
				'transport-loss',
				'malformed-listing',
			]),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('unavailable'),
			reason: z.enum(['server-busy', 'worker-unavailable', 'job-dead', 'job-unknown']),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('git-failed'),
			stage: stageSchema,
			status: z
				.number()
				.int()
				.min(-2147483648)
				.max(2147483647)
				.transform((value) => (value === 0 ? 0 : value))
				.nullable(),
		})
		.readonly(),
	z
		.strictObject({ kind: z.enum(['spawn-unavailable', 'spawn-failed']), stage: stageSchema })
		.readonly(),
	z
		.strictObject({
			kind: z.literal('too-large'),
			origin: z.enum(['git-probe', 'git-list', 'worker-result', 'workbench-result']),
		})
		.readonly(),
	z.strictObject({ kind: z.literal('owner-io'), stage: stageSchema.nullable() }).readonly(),
]);
const WorkbenchWorktreeQueryOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('found'),
			sessionId: SessionIdSchema,
			workbenchRevision: WorkbenchRevisionSchema,
			workerId: WorkerIdSchema,
			source: WorktreeQuerySourceSchema,
			entries: z.array(WorktreeQueryEntrySchema).min(1).readonly(),
		})
		.superRefine((value, context) => {
			let valid = value.entries[0]?.checkout.worktreePath === value.source.repositoryRoot;
			let sourceFound = false;
			const paths = new Set<string>();
			const workspaces = new Set<string>();
			for (const [index, entry] of value.entries.entries()) {
				const path = entry.checkout.worktreePath;
				if (
					paths.has(path) ||
					entry.linked !== (index !== 0) ||
					(index !== 0 && entry.checkout.bare)
				)
					valid = false;
				paths.add(path);
				if (path === value.source.checkoutPath) {
					sourceFound = true;
					if (
						entry.checkout.bare ||
						(value.source.workspaceId !== null &&
							!entry.workspaces.some(
								(workspace) => workspace.workspaceId === value.source.workspaceId,
							))
					)
						valid = false;
				}
				for (const workspace of entry.workspaces) {
					if (workspaces.has(workspace.workspaceId)) valid = false;
					workspaces.add(workspace.workspaceId);
				}
			}
			if (!valid || !sourceFound)
				context.addIssue({
					code: 'custom',
					message: 'Worktree source, listing and Workspace associations must agree',
				});
		})
		.readonly(),
	z.strictObject({ kind: z.literal('failed'), reason: WorktreeQueryFailureSchema }).readonly(),
]);
export const WorkbenchWorktreeQuerySchema = z
	.strictObject({
		kind: z.literal('workbench.worktree.query'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		sessionId: SessionIdSchema,
		source: WorktreeSourceSchema,
		trustRepository: z.boolean(),
	})
	.readonly();
export const WorkbenchWorktreeQueryResultSchema = z
	.strictObject({
		kind: z.literal('workbench.worktree.query.result'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		outcome: WorkbenchWorktreeQueryOutcomeSchema,
	})
	.readonly();
