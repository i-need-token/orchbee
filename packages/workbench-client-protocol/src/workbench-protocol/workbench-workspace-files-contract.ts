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
/** Content bytes one read returns; a larger file is read chunk by chunk. */
const WORKSPACE_FILE_CHUNK_BYTES = 256 * 1024;
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

const OffsetSchema = z.number().int().min(0).max(Number.MAX_SAFE_INTEGER);
const WorkspaceFileSourceSchema = z.enum(['worktree', 'index', 'head']);
const WorkspaceFileEncodingSchema = z.enum(['text', 'bytes']);
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
			offset: OffsetSchema,
			encoding: WorkspaceFileEncodingSchema,
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
const BASE64 = /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/u;
/** The byte count standard padded base64 decodes to. */
const base64Length = (encoded: string): number => {
	const padding = encoded.length - encoded.replace(/=+$/u, '').length;
	return (encoded.length / 4) * 3 - padding;
};
const WorkspaceFileContentSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({
			kind: z.literal('text'),
			offset: OffsetSchema,
			text: z.string().max(WORKSPACE_FILE_CHUNK_BYTES),
			nextOffset: OffsetSchema.nullable(),
		})
		.readonly(),
	z
		.strictObject({
			kind: z.literal('bytes'),
			offset: OffsetSchema,
			base64: z
				.string()
				.max(Math.ceil(WORKSPACE_FILE_CHUNK_BYTES / 3) * 4)
				.regex(BASE64),
			nextOffset: OffsetSchema.nullable(),
		})
		.readonly(),
	z.strictObject({ kind: z.literal('binary') }).readonly(),
	z.strictObject({ kind: z.literal('too-large') }).readonly(),
	z.strictObject({ kind: z.literal('absent') }).readonly(),
]);
/**
 * The version a working-tree file was read at: its modification time in nanoseconds, and, on the
 * first chunk of a file small enough to edit, the SHA-256 of the whole file.
 */
const WorkspaceFileVersionSchema = z
	.strictObject({
		modified: z.string().regex(/^[0-9]{1,39}$/u),
		sha256: z
			.string()
			.regex(/^[0-9a-f]{64}$/u)
			.nullable(),
	})
	.readonly();
/**
 * A chunk of `length` bytes at `offset`: at most one chunk long, inside the file, and either its
 * end or followed by the next chunk exactly where it stops.
 */
const isChunk = (
	fileLength: number,
	offset: number,
	length: number,
	nextOffset: number | null,
): boolean => {
	const end = offset + length;
	if (length > WORKSPACE_FILE_CHUNK_BYTES || end > fileLength) return false;
	return nextOffset === null
		? end === fileLength
		: nextOffset === end && length > 0 && nextOffset < fileLength;
};
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
			byteLength: OffsetSchema,
			version: WorkspaceFileVersionSchema.nullable(),
			content: WorkspaceFileContentSchema,
		})
		.refine(({ byteLength: fileLength, content, version }) => {
			switch (content.kind) {
				case 'text':
					return isChunk(fileLength, content.offset, byteLength(content.text), content.nextOffset);
				case 'bytes':
					return isChunk(
						fileLength,
						content.offset,
						base64Length(content.base64),
						content.nextOffset,
					);
				case 'absent':
					return fileLength === 0 && version === null;
				case 'binary':
				case 'too-large':
					return true;
			}
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
export type WorkspaceFileEncoding = z.infer<typeof WorkspaceFileEncodingSchema>;
export type WorkspaceFileVersion = z.infer<typeof WorkspaceFileVersionSchema>;
export type WorkspaceGitStatusEntry = z.infer<typeof WorkspaceGitStatusEntrySchema>;
export type WorkspaceGitStatusCode = z.infer<typeof WorkspaceGitStatusCodeSchema>;
