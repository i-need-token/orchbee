import { z } from 'zod';
import {
	SessionIdSchema,
	WorkbenchRequestIdSchema,
	WorkbenchWorkspaceIdSchema,
	WORKBENCH_PROTOCOL_VERSION,
} from './workbench-semantic-contract';

const MAXIMUM_PATH_BYTES = 4096;
const MAXIMUM_NAME_BYTES = 1024;
const MAXIMUM_ENTRIES = 5000;
const MAXIMUM_TEXT_BYTES = 2 * 1024 * 1024;
const encoder = new TextEncoder();
const byteLength = (value: string): number => encoder.encode(value).length;
const isEntryName = (name: string): boolean =>
	name.length > 0 &&
	byteLength(name) <= MAXIMUM_NAME_BYTES &&
	name !== '.' &&
	name !== '..' &&
	!/[/\\\p{Cc}\uD800-\uDFFF]/u.test(name);
/** A Workspace-relative path: `/`-separated names, none `.` or `..`, no backslash or control. */
const isWorkspaceRelativePath = (path: string): boolean =>
	path.length > 0 && byteLength(path) <= MAXIMUM_PATH_BYTES && path.split('/').every(isEntryName);

const RelativePathSchema = z.string().refine(isWorkspaceRelativePath);
const EntryNameSchema = z.string().refine(isEntryName);
const identity = {
	protocolVersion: z.literal(WORKBENCH_PROTOCOL_VERSION),
	requestId: WorkbenchRequestIdSchema,
	sessionId: SessionIdSchema,
	workspaceId: WorkbenchWorkspaceIdSchema,
};

const WorkspaceFileSourceSchema = z.enum(['worktree', 'index', 'head']);
const WorkspaceFilesOperationSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('list-directory'),
			path: z.union([z.literal(''), RelativePathSchema]),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('read-file'),
			path: RelativePathSchema,
			source: WorkspaceFileSourceSchema,
		})
		.readonly(),
	z.strictObject({ kind: z.literal('git-status') }).readonly(),
]);
export const WorkspaceFilesRequestSchema = z
	.strictObject({
		...identity,
		kind: z.literal('workbench.workspace-files'),
		operation: WorkspaceFilesOperationSchema,
	})
	.readonly();

const WorkspaceDirectoryEntrySchema = z
	.strictObject({
		name: EntryNameSchema,
		entryKind: z.enum(['file', 'directory', 'other']),
		symbolicLink: z.boolean(),
		ignored: z.boolean(),
	})
	.readonly();
const WorkspaceFileContentSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('text'),
			text: z.string().refine((text) => byteLength(text) <= MAXIMUM_TEXT_BYTES),
		})
		.readonly(),
	z.strictObject({ kind: z.literal('binary') }).readonly(),
	z.strictObject({ kind: z.literal('too-large') }).readonly(),
	z.strictObject({ kind: z.literal('absent') }).readonly(),
]);
const WorkspaceGitStatusCodeSchema = z.enum([
	'unmodified',
	'modified',
	'type-changed',
	'added',
	'deleted',
	'renamed',
	'copied',
	'unmerged',
	'untracked',
]);
const WorkspaceGitStatusEntrySchema = z
	.strictObject({
		path: RelativePathSchema,
		originalPath: RelativePathSchema.nullable(),
		index: WorkspaceGitStatusCodeSchema,
		worktree: WorkspaceGitStatusCodeSchema,
		conflicted: z.boolean(),
	})
	.readonly();
const WorkspaceFilesOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('directory'),
			entries: z.array(WorkspaceDirectoryEntrySchema).max(MAXIMUM_ENTRIES).readonly(),
			truncated: z.boolean(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('file'),
			byteLength: z.number().int().min(0).max(Number.MAX_SAFE_INTEGER),
			content: WorkspaceFileContentSchema,
		})
		.refine((file) => {
			if (file.content.kind === 'text') return byteLength(file.content.text) === file.byteLength;
			return file.content.kind !== 'absent' || file.byteLength === 0;
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('git-status'),
			branch: z
				.string()
				.min(1)
				.refine((v) => byteLength(v) <= 1024 && !/[\p{Cc}\uD800-\uDFFF]/u.test(v))
				.nullable(),
			entries: z.array(WorkspaceGitStatusEntrySchema).max(MAXIMUM_ENTRIES).readonly(),
			truncated: z.boolean(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('failed'),
			reason: z.enum([
				'denied',
				'unavailable',
				'not-found',
				'not-repository',
				'timeout',
				'malformed',
			]),
		})
		.readonly(),
]);
export const WorkspaceFilesResultSchema = z
	.strictObject({
		...identity,
		kind: z.literal('workbench.workspace-files.result'),
		outcome: WorkspaceFilesOutcomeSchema,
	})
	.readonly();

type WorkspaceFilesRequest = z.infer<typeof WorkspaceFilesRequestSchema>;
export type WorkspaceFilesOperation = WorkspaceFilesRequest['operation'];
type WorkspaceFilesResult = z.infer<typeof WorkspaceFilesResultSchema>;
export type WorkspaceFilesOutcome = WorkspaceFilesResult['outcome'];
export type WorkspaceDirectoryEntry = z.infer<typeof WorkspaceDirectoryEntrySchema>;
export type WorkspaceFileSource = z.infer<typeof WorkspaceFileSourceSchema>;
export type WorkspaceGitStatusEntry = z.infer<typeof WorkspaceGitStatusEntrySchema>;
export type WorkspaceGitStatusCode = z.infer<typeof WorkspaceGitStatusCodeSchema>;
