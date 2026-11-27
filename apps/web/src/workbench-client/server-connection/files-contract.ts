/**
 * The web Client's binding of the File Bar operations `schemas/files` states, for the operations
 * its file sessions send: listing, stat, grants, reads and the root stream. It frames requests only
 * once they meet the contract, so a request that would break it fails here instead of on the
 * Server, and it reads replies and notices under the contract's JSON rules and the Client's
 * compatibility rules: an error code it does not know reads as `failed`, an `unavailable` reason it
 * does not know is absent, and a notice it cannot read is absent. The Git operations join with the
 * Git adapter.
 */
import { z } from 'zod';

import { isWellFormedText, readFilesJson } from './files-json';
import { type FilesChunk, decodeFilesRecord, encodeFilesRequestRecord } from './files-record-codec';

const MAXIMUM_PATH_BYTES = 4096;
const MAXIMUM_NAME_BYTES = 1024;
const MAXIMUM_DETAIL_BYTES = 4096;
const MAXIMUM_U64 = 18_446_744_073_709_551_615n;

/** The error codes `schemas/files` names; `remote-disconnected` is the Client's own. */
const FILES_ERROR_CODES = [
	'busy',
	'cancelled',
	'content-newer',
	'denied',
	'disk-full',
	'exists',
	'failed',
	'git-failed',
	'invalid-name',
	'invalid-regex',
	'invalid-request',
	'is-directory',
	'link-target-outside',
	'not-directory',
	'not-found',
	'outside-scope',
	'permission-denied',
	'read-only',
	'stale-revision',
	'timeout',
	'too-large',
	'trash-failed',
	'trash-unavailable',
	'unavailable',
	'undo-expired',
	'unrepresentable-name',
	'unsafe-repository',
	'unsupported-location',
	'write-incomplete',
] as const;

/** The `unavailable` reasons this Client knows how to present. */
const FILES_UNAVAILABLE_REASONS = [
	'build-mismatch',
	'crashed-repeatedly',
	'restarting',
	'search-binary',
	'stuck-filesystem',
] as const;

/** A failed operation as the Client reads it. */
export interface FilesError {
	readonly code: (typeof FILES_ERROR_CODES)[number];
	/** Content Orchbee did not author, such as git's standard error, shown verbatim. */
	readonly detail?: string | undefined;
	/** VS Code's `GitErrorCodes` name in kebab-case, such as `repository-is-locked`. */
	readonly gitErrorCode?: string | undefined;
	/** The operating system's error number. */
	readonly osError?: number | undefined;
	/** Why the service is unavailable; only with `unavailable`. */
	readonly reason?: (typeof FILES_UNAVAILABLE_REASONS)[number] | undefined;
}

/** What a Result answered: the operation's value, or its failure. */
export type FilesReply<Value> =
	| { readonly error: FilesError; readonly kind: 'error' }
	| { readonly kind: 'ok'; readonly value: Value };

/** A request-mode operation: how its request is framed and its Result read. */
export interface FilesOperation<Request, Value> {
	/** The Result record it was answered with; null when the record cannot be read as one. */
	decode(record: Uint8Array): FilesReply<Value> | null;
	/** The request as an OBFB record; null when it breaks the contract's rules or bounds. */
	encode(request: Request): Uint8Array | null;
}

function utf8ByteLength(value: string): number {
	let bytes = 0;
	for (let index = 0; index < value.length; index += 1) {
		const code = value.charCodeAt(index);
		if (code < 0x80) bytes += 1;
		else if (code < 0x800) bytes += 2;
		else if (code >= 0xd800 && code <= 0xdbff) {
			// A pair is one 4-byte scalar; well-formed text pairs every high surrogate.
			bytes += 4;
			index += 1;
		} else bytes += 3;
	}
	return bytes;
}

/** Text without NUL and within a bound the binding counts in UTF-8 bytes, as the contract states. */
const textSchema = (maximumBytes: number, minimumLength: number) =>
	z
		.string()
		.min(minimumLength)
		.refine(
			(value) =>
				!value.includes('\u0000') &&
				isWellFormedText(value) &&
				utf8ByteLength(value) <= maximumBytes,
		);

const u64TextSchema = (pattern: RegExp) =>
	z
		.string()
		.regex(pattern)
		.refine((value) => BigInt(value) <= MAXIMUM_U64);

const identitySchema = z
	.string()
	.regex(/^[0-9a-f]{32}$/)
	.refine((value) => /[1-9a-f]/.test(value));
const pathSchema = textSchema(MAXIMUM_PATH_BYTES, 1);
const pathsSchema = z.array(pathSchema).min(1).max(256);
const pageCursorSchema = z.string().regex(/^[0-9a-f]{32}$/);
const encodingSchema = z.string().regex(/^[a-z0-9]{1,24}$/);
const tokenSchema = z
	.string()
	.max(64)
	.regex(/^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/);
const transferSchema = u64TextSchema(/^[1-9][0-9]{0,19}$/);
const counterSchema = u64TextSchema(/^(?:0|[1-9][0-9]{0,19})$/);
/** An optional flag, which an encoder omits when false. */
const flagSchema = z
	.boolean()
	.optional()
	.transform((flag) => (flag === true ? true : undefined));

const errorSchema = z
	.strictObject({
		code: tokenSchema,
		detail: textSchema(MAXIMUM_DETAIL_BYTES, 0).optional(),
		gitErrorCode: tokenSchema.optional(),
		osError: z.int().optional(),
		reason: tokenSchema.optional(),
	})
	.refine(
		({ code, reason }) =>
			code !== 'remote-disconnected' && (code === 'unavailable') === (reason !== undefined),
	)
	.transform((wire): FilesError => {
		const code = z.enum(FILES_ERROR_CODES).safeParse(wire.code);
		const reason = z.enum(FILES_UNAVAILABLE_REASONS).safeParse(wire.reason);
		return {
			code: code.success ? code.data : 'failed',
			detail: wire.detail,
			gitErrorCode: wire.gitErrorCode,
			osError: wire.osError,
			reason: reason.success ? reason.data : undefined,
		};
	});
const errorReplySchema = z.strictObject({ error: errorSchema });
const emptyResultSchema = z.strictObject({}).transform(() => null);

const entrySchema = z
	.strictObject({
		mtimeMs: z.int(),
		name: textSchema(MAXIMUM_NAME_BYTES, 0),
		readonly: z.boolean(),
		size: z.int().min(0),
		symlink: z.boolean(),
		type: z.enum(['directory', 'file', 'unknown']),
		unrepresentable: z.boolean().optional(),
	})
	.transform((entry) => ({ ...entry, unrepresentable: entry.unrepresentable === true }));
const entriesSchema = z.array(entrySchema).max(2000);

const fsListResultSchema = z
	.strictObject({
		cursor: pageCursorSchema.optional(),
		descendants: z
			.array(z.strictObject({ entries: entriesSchema, path: pathSchema }))
			.min(1)
			.max(64)
			.optional(),
		entries: entriesSchema,
	})
	.transform(({ cursor, descendants, entries }) => ({
		cursor: cursor ?? null,
		descendants: descendants ?? [],
		entries,
	}));
const fsStatResultSchema = z
	.strictObject({
		items: z
			.array(
				z.union([
					z
						.strictObject({ entry: entrySchema, path: pathSchema })
						.transform((item) => ({ ...item, kind: 'entry' as const })),
					z
						.strictObject({ error: errorSchema, path: pathSchema })
						.transform((item) => ({ ...item, kind: 'error' as const })),
				]),
			)
			.min(1)
			.max(256),
	})
	.transform(({ items }) => items);
const fsGrantResultSchema = z.strictObject({ entry: entrySchema }).transform(({ entry }) => entry);
const readResultSchema = z.strictObject({
	binary: z.boolean(),
	bom: z.boolean(),
	encoding: encodingSchema.nullable(),
	mtimeMs: z.int(),
	size: z.int().min(0),
});
const fsGuessEncodingResultSchema = z
	.strictObject({ encoding: encodingSchema.nullable() })
	.transform(({ encoding }) => encoding);

const rootSchema = z.union([
	z.strictObject({ path: pathSchema, state: z.enum(['missing', 'present']) }),
	z.strictObject({ state: z.literal('none') }),
]);
const watchSchema = z.enum(['degraded', 'full', 'stopped']);
const fsSnapshotSchema = z.strictObject({
	caseSensitive: z.boolean(),
	os: z.enum(['linux', 'macos', 'windows']),
	root: rootSchema,
	separator: z.enum(['/', '\\']),
	watch: watchSchema,
});
const fsUpdateSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		changes: z
			.array(
				z.strictObject({
					directory: z.boolean(),
					path: pathSchema,
					type: z.enum(['added', 'changed', 'deleted']),
				}),
			)
			.min(1)
			.max(500),
		kind: z.literal('changes'),
	}),
	z.strictObject({ kind: z.literal('rescan'), paths: z.array(pathSchema).min(1).max(500) }),
	z.strictObject({ kind: z.literal('root'), root: rootSchema }),
	z.strictObject({ kind: z.literal('watch'), watch: watchSchema }),
]);

/** The root facts an `fs.subscribe` stream starts and restarts with. */
export type FilesRootSnapshot = z.output<typeof fsSnapshotSchema>;
/** One change an `fs.subscribe` stream reports after its snapshot. */
export type FilesRootUpdate = z.output<typeof fsUpdateSchema>;

const pathsRequestSchema = <Op extends 'fs.release' | 'fs.stat' | 'fs.ungrant'>(op: Op) =>
	z.strictObject({ op: z.literal(op), paths: pathsSchema, workspaceId: identitySchema });
const repositoryRequestSchema = <Op extends 'repository.close' | 'repository.consent'>(op: Op) =>
	z.strictObject({ op: z.literal(op), repository: pathSchema, workspaceId: identitySchema });

const utf8Encoder = new TextEncoder();

function encodeRequest<Schema extends z.ZodType>(
	schema: Schema,
	request: z.input<Schema>,
): Uint8Array | null {
	const parsed = schema.safeParse(request);
	if (!parsed.success) return null;
	// Absent members, including false flags, are left out; integers are written without a fraction.
	return encodeFilesRequestRecord(utf8Encoder.encode(JSON.stringify(parsed.data)));
}

/** Reads a Result's JSON: its failure, or the value `result` reads, which null never admits. */
function decodeResult<Value>(
	record: Uint8Array,
	result: z.ZodType<Value> | null,
): FilesReply<Value> | null {
	const decoded = decodeFilesRecord(record, 'result-frame');
	if (decoded?.kind !== 'result') return null;
	const read = readFilesJson(decoded.body);
	if (read.kind === 'malformed') return null;
	const failure = errorReplySchema.safeParse(read.value);
	if (failure.success) return { error: failure.data.error, kind: 'error' };
	if (result === null) return null;
	const parsed = result.safeParse(read.value);
	return parsed.success ? { kind: 'ok', value: parsed.data } : null;
}

function requestOperation<RequestSchema extends z.ZodType, Value>(
	request: RequestSchema,
	result: z.ZodType<Value>,
): FilesOperation<z.input<RequestSchema>, Value> {
	return {
		decode: (record) => decodeResult(record, result),
		encode: (value) => encodeRequest(request, value),
	};
}

/** The request-mode operations whose Result carries JSON, by their wire names. */
export const FILES_OPERATIONS = {
	'fs.grant': requestOperation(
		z.strictObject({
			op: z.literal('fs.grant'),
			path: pathSchema,
			source: z.enum(['diff', 'os-open', 'restored', 'save-as', 'terminal-link', 'tree']),
			workspaceId: identitySchema,
		}),
		fsGrantResultSchema,
	),
	'fs.guess-encoding': requestOperation(
		z.strictObject({
			op: z.literal('fs.guess-encoding'),
			path: pathSchema,
			workspaceId: identitySchema,
		}),
		fsGuessEncodingResultSchema,
	),
	'fs.list': requestOperation(
		z.strictObject({
			cursor: pageCursorSchema.optional(),
			op: z.literal('fs.list'),
			path: pathSchema,
			singleChild: flagSchema,
			workspaceId: identitySchema,
		}),
		fsListResultSchema,
	),
	'fs.read': requestOperation(
		z.strictObject({
			encoding: encodingSchema.optional(),
			forceText: flagSchema,
			limit: z.int().min(0),
			mode: z.enum(['bytes', 'text']),
			op: z.literal('fs.read'),
			path: pathSchema,
			transfer: transferSchema,
			workspaceId: identitySchema,
		}),
		readResultSchema,
	),
	'fs.release': requestOperation(pathsRequestSchema('fs.release'), emptyResultSchema),
	'fs.stat': requestOperation(pathsRequestSchema('fs.stat'), fsStatResultSchema),
	'fs.ungrant': requestOperation(pathsRequestSchema('fs.ungrant'), emptyResultSchema),
	'repository.close': requestOperation(
		repositoryRequestSchema('repository.close'),
		emptyResultSchema,
	),
	'repository.consent': requestOperation(
		repositoryRequestSchema('repository.consent'),
		emptyResultSchema,
	),
	'transfer.close': requestOperation(
		z.strictObject({
			op: z.literal('transfer.close'),
			transfer: transferSchema,
			workspaceId: identitySchema,
		}),
		emptyResultSchema,
	),
} as const;

const fsReadChunkRequestSchema = z.strictObject({
	length: z.int().min(16_384).max(131_072),
	offset: counterSchema,
	op: z.literal('fs.read-chunk'),
	transfer: transferSchema,
	workspaceId: identitySchema,
});

/** `fs.read-chunk`, which a Chunk answers and a Result answers only to fail. */
export const FILES_READ_CHUNK = {
	/** The chunk or failure; null when the record is neither. */
	decode(
		record: Uint8Array,
	):
		| { readonly chunk: FilesChunk; readonly kind: 'chunk' }
		| { readonly error: FilesError; readonly kind: 'error' }
		| null {
		const decoded = decodeFilesRecord(record, 'result-frame');
		if (decoded?.kind === 'chunk') return decoded;
		const failure = decodeResult(record, null);
		return failure?.kind === 'error' ? failure : null;
	},
	encode(request: z.input<typeof fsReadChunkRequestSchema>): Uint8Array | null {
		return encodeRequest(fsReadChunkRequestSchema, request);
	},
};

const fsSubscribeRequestSchema = z.strictObject({
	op: z.literal('fs.subscribe'),
	workspaceId: identitySchema,
});

/** `fs.subscribe`, an Events stream that a Result answers only to fail. */
export const FILES_ROOT_STREAM = {
	/** The failure a Result reported; null when the record reports none. */
	decodeFailure(record: Uint8Array): FilesError | null {
		const failure = decodeResult(record, null);
		return failure?.kind === 'error' ? failure.error : null;
	},
	/** A Snapshot or Update frame's notice; null for a notice this Client cannot read. */
	decodeNotice(
		kind: 'snapshot' | 'update',
		record: Uint8Array,
	):
		| { readonly kind: 'snapshot'; readonly snapshot: FilesRootSnapshot }
		| { readonly kind: 'update'; readonly update: FilesRootUpdate }
		| null {
		const decoded = decodeFilesRecord(record, 'notice-frame');
		if (decoded?.kind !== 'notice') return null;
		const read = readFilesJson(decoded.body);
		if (read.kind === 'malformed') return null;
		if (kind === 'snapshot') {
			const snapshot = fsSnapshotSchema.safeParse(read.value);
			return snapshot.success ? { kind, snapshot: snapshot.data } : null;
		}
		const update = fsUpdateSchema.safeParse(read.value);
		return update.success ? { kind, update: update.data } : null;
	},
	encode(request: z.input<typeof fsSubscribeRequestSchema>): Uint8Array | null {
		return encodeRequest(fsSubscribeRequestSchema, request);
	},
};
