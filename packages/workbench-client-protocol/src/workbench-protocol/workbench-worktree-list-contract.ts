import { z } from 'zod';
import {
	SessionIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
	WorkbenchDirectoryIdSchema,
	WorkbenchDirectoryPathSchema,
	WorkbenchRequestIdSchema,
	WorkbenchRevisionSchema,
} from './workbench-semantic-contract';

export const WORKBENCH_MAXIMUM_ENCODED_WORKTREE_LIST_RESULT_BYTES = 32 * 1024;
const textSchema = z.string().refine((value) => !/[\uD800-\uDFFF]/u.test(value));
const hexSchema = z.string().regex(/^(?:[0-9a-f]{2})*$/u);
const stageSchema = z.enum(['probe', 'list']);
const absolutePath = (value: string) =>
	/^(?:\/|(?:\\\\\?\\)?[A-Za-z]:[/\\])/u.test(value) && !value.includes('\0');

// Git observation text is not a registered Directory path and has no 4096-byte lexical cap.
const WorktreeEntrySchema = z
	.strictObject({
		worktreePath: textSchema.nullish().transform((value) => value ?? null),
		worktreePathBytesHex: hexSchema.nullish().transform((value) => value ?? null),
		head: z
			.string()
			.regex(/^(?:[0-9a-f]{40}|[0-9a-f]{64})$/u)
			.nullish()
			.transform((value) => value ?? null),
		branch: textSchema.nullish().transform((value) => value ?? null),
		branchBytesHex: hexSchema.nullish().transform((value) => value ?? null),
		bare: z.boolean(),
		detached: z.boolean(),
		locked: z.boolean(),
		prunable: z.boolean(),
	})
	.refine((entry) => {
		if ((entry.worktreePath !== null) === (entry.worktreePathBytesHex !== null)) return false;
		const path =
			entry.worktreePath ??
			(entry.worktreePathBytesHex?.match(/../gu) ?? [])
				.map((byte) => String.fromCharCode(Number.parseInt(byte, 16)))
				.join('');
		return (
			absolutePath(path) &&
			!(entry.branch !== null && entry.branchBytesHex !== null) &&
			(entry.bare
				? entry.head === null &&
					entry.branch === null &&
					entry.branchBytesHex === null &&
					!entry.detached
				: entry.head !== null) &&
			(!entry.detached || (entry.branch === null && entry.branchBytesHex === null))
		);
	}, 'Invalid observed Git entry')
	.transform((entry) => ({
		...(entry.worktreePath !== null
			? { worktreePath: entry.worktreePath }
			: { worktreePathBytesHex: entry.worktreePathBytesHex }),
		head: entry.head ?? null,
		...(entry.branchBytesHex !== null
			? { branchBytesHex: entry.branchBytesHex }
			: { branch: entry.branch ?? null }),
		bare: entry.bare,
		detached: entry.detached,
		locked: entry.locked,
		prunable: entry.prunable,
	}))
	.readonly();

export const WorkbenchWorktreeListReadSchema = z
	.strictObject({
		kind: z.literal('workbench.worktree-list.read'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		directoryId: WorkbenchDirectoryIdSchema,
	})
	.readonly();

const WorkbenchWorktreeListOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('found'),
			sessionId: SessionIdSchema,
			workbenchRevision: WorkbenchRevisionSchema,
			directoryId: WorkbenchDirectoryIdSchema,
			path: WorkbenchDirectoryPathSchema,
			entries: z.array(WorktreeEntrySchema).readonly(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.enum(['not-found', 'denied', 'timeout', 'transport-loss', 'malformed-listing']),
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
				.min(-2_147_483_648)
				.max(2_147_483_647)
				.nullish()
				.transform((value) => value ?? null),
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
	z
		.strictObject({
			kind: z.literal('owner-io'),
			stage: stageSchema.nullish().transform((value) => value ?? null),
		})
		.readonly(),
]);

export const WorkbenchWorktreeListResultSchema = z
	.strictObject({
		kind: z.literal('workbench.worktree-list.result'),
		protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
		requestId: WorkbenchRequestIdSchema,
		outcome: WorkbenchWorktreeListOutcomeSchema,
	})
	.readonly();
