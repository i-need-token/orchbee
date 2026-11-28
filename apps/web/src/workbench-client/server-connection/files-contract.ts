/**
 * The web Client's binding of the File Bar operations `schemas/files` states, for the operations
 * its file sessions send: listing, stat, grants, reads, Source Control and the Events streams. It
 * frames requests only once they meet the contract, so a request that would break it fails here
 * instead of on the Server, and it reads replies and notices under the contract's JSON rules and the
 * Client's compatibility rules: an error code it does not know reads as `failed`, an `unavailable`
 * reason it does not know is absent, and a notice it cannot read is absent.
 */
import { z } from 'zod';

import { isWellFormedText, readFilesJson } from './files-json';
import {
	type FilesChunk,
	decodeFilesRecord,
	encodeFilesPutRecord,
	encodeFilesRequestRecord,
} from './files-record-codec';

const MAXIMUM_PATH_BYTES = 4096;
const MAXIMUM_NAME_BYTES = 1024;
const MAXIMUM_DETAIL_BYTES = 4096;
const MAXIMUM_PROMPT_BYTES = 4096;
const MAXIMUM_MESSAGE_BYTES = 16_384;
const MAXIMUM_OUTPUT_BYTES = 131_072;
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

/** A counter the Client counts with, which must stay within a safe integer. */
const countSchema = counterSchema.transform(Number).refine(Number.isSafeInteger);
const safeIntegerSchema = z.int().min(0);
const commitSchema = z.string().regex(/^(?:[0-9a-f]{40}|[0-9a-f]{64})$/);
/** A remote or branch name, which never starts with `-`, so git never reads it as an option. */
const gitNameSchema = textSchema(MAXIMUM_NAME_BYTES, 1).refine((name) => !name.startsWith('-'));

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

const statusLetterSchema = z.enum(['.', 'M', 'T', 'A', 'D', 'R', 'C', 'U', '?', '!']);
const statusEntrySchema = z
	.strictObject({
		original: pathSchema.optional(),
		path: pathSchema,
		submodule: z.boolean(),
		x: statusLetterSchema,
		y: statusLetterSchema,
	})
	.transform(({ original, ...entry }) => ({ ...entry, original: original ?? null }));
const gitStatusPageResultSchema = z
	.strictObject({
		cursor: pageCursorSchema.optional(),
		entries: z.array(statusEntrySchema).max(2000),
	})
	.transform(({ cursor, entries }) => ({ cursor: cursor ?? null, entries }));

const scmRepositorySchema = z
	.strictObject({
		busy: z
			.enum([
				'clean',
				'commit',
				'discard',
				'fetch',
				'pull',
				'push',
				'stage',
				'stage-content',
				'sync',
				'unstage',
			])
			.optional(),
		counts: z.strictObject({
			index: safeIntegerSchema,
			merge: safeIntegerSchema,
			untracked: safeIntegerSchema,
			workingTree: safeIntegerSchema,
		}),
		head: z.strictObject({
			branch: textSchema(MAXIMUM_NAME_BYTES, 1).optional(),
			commit: commitSchema.optional(),
			detached: z.boolean(),
			unborn: z.boolean(),
		}),
		kind: z.enum(['file', 'nested', 'parent', 'root']),
		operation: z.enum(['cherry-pick', 'merge', 'none', 'rebase', 'revert']),
		repository: pathSchema,
		revision: countSchema,
		stale: z.boolean(),
		truncated: z.boolean(),
		upstream: z
			.strictObject({
				ahead: safeIntegerSchema,
				behind: safeIntegerSchema,
				name: textSchema(MAXIMUM_NAME_BYTES, 1),
			})
			.optional(),
		warm: z.boolean(),
	})
	.transform(({ busy, head, upstream, ...repository }) => ({
		...repository,
		busy: busy ?? null,
		head: {
			branch: head.branch ?? null,
			commit: head.commit ?? null,
			detached: head.detached,
			unborn: head.unborn,
		},
		upstream: upstream ?? null,
	}));
const scmSummarySchema = z.strictObject({
	candidates: z.array(pathSchema).max(256),
	git: z
		.strictObject({
			state: z.enum(['missing', 'ok', 'too-old']),
			version: textSchema(64, 1).optional(),
		})
		.transform(({ state, version }) => ({ state, version: version ?? null })),
	repositories: z.array(scmRepositorySchema).max(256),
	repositoriesTruncated: z.boolean(),
	unsafe: z.array(pathSchema).max(256),
});

const promptSchema = z
	.strictObject({
		id: identitySchema,
		kind: z.enum(['editor', 'host-key', 'passphrase', 'password', 'username']),
		path: pathSchema.optional(),
		repository: pathSchema,
		text: textSchema(MAXIMUM_PROMPT_BYTES, 0),
	})
	.transform(({ path, ...prompt }) => ({ ...prompt, path: path ?? null }));
const relaySnapshotSchema = z
	.strictObject({ prompts: z.array(promptSchema).max(16) })
	.transform(({ prompts }) => prompts);
const relayUpdateSchema = z.union([
	z.strictObject({ kind: z.literal('prompt'), prompt: promptSchema }),
	z.strictObject({ id: identitySchema, kind: z.literal('closed') }),
]);

const gitRefsResultSchema = z
	.strictObject({
		cursor: pageCursorSchema.optional(),
		refs: z
			.array(
				z
					.strictObject({
						commit: commitSchema,
						name: textSchema(MAXIMUM_NAME_BYTES, 1),
						type: z.enum(['head', 'remote-head', 'tag']),
						upstream: z.strictObject({ name: gitNameSchema, remote: gitNameSchema }).optional(),
					})
					.transform(({ upstream, ...ref }) => ({
						...ref,
						upstream: upstream === undefined ? null : `${upstream.remote}/${upstream.name}`,
					})),
			)
			.max(2000),
		remotes: z
			.array(
				z
					.strictObject({
						fetchUrl: textSchema(MAXIMUM_PATH_BYTES, 1).optional(),
						name: gitNameSchema,
						pushUrl: textSchema(MAXIMUM_PATH_BYTES, 1).optional(),
					})
					.transform(({ fetchUrl, name, pushUrl }) => ({
						fetchUrl: fetchUrl ?? null,
						name,
						pushUrl: pushUrl ?? null,
					})),
			)
			.max(256),
	})
	.transform(({ cursor, refs, remotes }) => ({ cursor: cursor ?? null, refs, remotes }));
const gitUserConfigResultSchema = z
	.strictObject({
		email: textSchema(MAXIMUM_NAME_BYTES, 0).optional(),
		name: textSchema(MAXIMUM_NAME_BYTES, 0).optional(),
	})
	.transform(({ email, name }) => ({ email: email ?? null, name: name ?? null }));
const gitOutputResultSchema = z.strictObject({
	cursor: counterSchema,
	dropped: countSchema,
	text: textSchema(MAXIMUM_OUTPUT_BYTES, 0),
});

const gitCleanItemsSchema = z
	.array(
		z
			.strictObject({ error: errorSchema.optional(), path: pathSchema })
			.transform(({ error, path }) => ({ error: error ?? null, path })),
	)
	.min(1)
	.max(256);
const doneSchema = emptyResultSchema.transform(() => ({ kind: 'done' as const }));
const gitCleanResultSchema = z
	.strictObject({ items: gitCleanItemsSchema })
	.transform(({ items }) => ({ items, kind: 'cleaned' as const }));
const gitCommitResultSchema = z
	.strictObject({ commit: commitSchema })
	.transform(({ commit }) => ({ commit, kind: 'committed' as const }));
const gitInitResultSchema = z
	.strictObject({ repository: pathSchema })
	.transform(({ repository }) => ({ kind: 'initialized' as const, repository }));
/** A mutation's recorded Result, as `request.outcome` repeats it. */
const mutationReplySchema = z.union([
	errorReplySchema.transform(({ error }) => ({ error, kind: 'error' as const })),
	z
		.union([doneSchema, gitCleanResultSchema, gitCommitResultSchema, gitInitResultSchema])
		.transform((value) => ({ kind: 'ok' as const, value })),
]);
const requestOutcomeResultSchema = z.union([
	z.strictObject({ result: mutationReplySchema, state: z.literal('completed') }),
	z.strictObject({ state: z.enum(['running', 'unknown']) }),
]);

const gitPathsRequestSchema = <Op extends 'git.discard' | 'git.stage' | 'git.unstage'>(op: Op) =>
	z.strictObject({
		op: z.literal(op),
		paths: pathsSchema,
		repository: pathSchema,
		requestId: identitySchema,
		workspaceId: identitySchema,
	});
const gitPullRequestSchema = <Op extends 'git.pull' | 'git.sync'>(op: Op) =>
	z.strictObject({
		op: z.literal(op),
		rebase: z.boolean(),
		repository: pathSchema,
		requestId: identitySchema,
		workspaceId: identitySchema,
	});
const gitPathRequestSchema = <Op extends 'git.init' | 'git.mark-safe'>(op: Op) =>
	z.strictObject({
		op: z.literal(op),
		path: pathSchema,
		requestId: identitySchema,
		workspaceId: identitySchema,
	});
/** A false member that only names the other form of a request, which an encoder omits. */
const otherFormSchema = z
	.literal(false)
	.optional()
	.transform(() => undefined);
const commitFields = {
	all: z.enum(['all', 'none', 'tracked']),
	amend: z.boolean(),
	noVerify: z.boolean(),
	op: z.literal('git.commit'),
	repository: pathSchema,
	requestId: identitySchema,
	signoff: z.boolean(),
	workspaceId: identitySchema,
};
const fetchFields = {
	background: z.boolean(),
	op: z.literal('git.fetch'),
	prune: z.boolean(),
	repository: pathSchema,
	requestId: identitySchema,
	workspaceId: identitySchema,
};
const answerFields = {
	op: z.literal('relay.answer'),
	prompt: identitySchema,
	workspaceId: identitySchema,
};

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
	'client.focus': requestOperation(
		z.strictObject({
			focused: z.boolean(),
			op: z.literal('client.focus'),
			workspaceId: identitySchema,
		}),
		emptyResultSchema,
	),
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
	'git.check-ignore': requestOperation(
		z.strictObject({
			op: z.literal('git.check-ignore'),
			paths: z.array(pathSchema).min(1).max(1000),
			repository: pathSchema,
			workspaceId: identitySchema,
		}),
		z.strictObject({ ignored: z.array(pathSchema).max(1000) }).transform(({ ignored }) => ignored),
	),
	'git.clean': requestOperation(
		z.strictObject({
			mode: z.enum(['permanent', 'trash']),
			op: z.literal('git.clean'),
			paths: pathsSchema,
			repository: pathSchema,
			requestId: identitySchema,
			workspaceId: identitySchema,
		}),
		gitCleanResultSchema,
	),
	'git.commit': requestOperation(
		z.union([
			z.strictObject({
				...commitFields,
				editor: otherFormSchema,
				message: textSchema(MAXIMUM_MESSAGE_BYTES, 0),
			}),
			z.strictObject({ ...commitFields, editor: z.literal(true) }),
		]),
		gitCommitResultSchema,
	),
	'git.discard': requestOperation(gitPathsRequestSchema('git.discard'), doneSchema),
	'git.fetch': requestOperation(
		z.union([
			z.strictObject({ ...fetchFields, all: otherFormSchema, remote: gitNameSchema }),
			z.strictObject({ ...fetchFields, all: flagSchema }),
		]),
		doneSchema,
	),
	'git.init': requestOperation(gitPathRequestSchema('git.init'), gitInitResultSchema),
	'git.mark-safe': requestOperation(gitPathRequestSchema('git.mark-safe'), doneSchema),
	'git.output': requestOperation(
		z.strictObject({
			cursor: counterSchema.optional(),
			op: z.literal('git.output'),
			workspaceId: identitySchema,
		}),
		gitOutputResultSchema,
	),
	'git.pull': requestOperation(gitPullRequestSchema('git.pull'), doneSchema),
	'git.push': requestOperation(
		z.strictObject({
			branch: gitNameSchema.optional(),
			force: z.enum(['force', 'lease', 'none']),
			op: z.literal('git.push'),
			remote: gitNameSchema.optional(),
			repository: pathSchema,
			requestId: identitySchema,
			setUpstream: z.boolean(),
			tags: z.boolean(),
			workspaceId: identitySchema,
		}),
		doneSchema,
	),
	'git.refs': requestOperation(
		z.strictObject({
			cursor: pageCursorSchema.optional(),
			op: z.literal('git.refs'),
			repository: pathSchema,
			workspaceId: identitySchema,
		}),
		gitRefsResultSchema,
	),
	'git.show': requestOperation(
		z.strictObject({
			encoding: encodingSchema.optional(),
			limit: z.int().min(0),
			mode: z.enum(['bytes', 'text']),
			op: z.literal('git.show'),
			path: pathSchema,
			ref: z.string().regex(/^(?:HEAD|index|[0-9a-f]{40}|[0-9a-f]{64})$/),
			repository: pathSchema,
			transfer: transferSchema,
			workspaceId: identitySchema,
		}),
		readResultSchema,
	),
	'git.stage': requestOperation(gitPathsRequestSchema('git.stage'), doneSchema),
	'git.stage-content': requestOperation(
		z.strictObject({
			op: z.literal('git.stage-content'),
			path: pathSchema,
			repository: pathSchema,
			requestId: identitySchema,
			size: safeIntegerSchema,
			transfer: transferSchema,
			workspaceId: identitySchema,
		}),
		doneSchema,
	),
	'git.status-page': requestOperation(
		z.strictObject({
			cursor: pageCursorSchema.optional(),
			op: z.literal('git.status-page'),
			repository: pathSchema,
			revision: counterSchema,
			workspaceId: identitySchema,
		}),
		gitStatusPageResultSchema,
	),
	'git.sync': requestOperation(gitPullRequestSchema('git.sync'), doneSchema),
	'git.unstage': requestOperation(gitPathsRequestSchema('git.unstage'), doneSchema),
	'git.user-config': requestOperation(
		z.strictObject({
			op: z.literal('git.user-config'),
			repository: pathSchema,
			workspaceId: identitySchema,
		}),
		gitUserConfigResultSchema,
	),
	'relay.answer': requestOperation(
		z.union([
			z.strictObject({
				...answerFields,
				cancelled: otherFormSchema,
				value: textSchema(MAXIMUM_PROMPT_BYTES, 0),
			}),
			z.strictObject({ ...answerFields, cancelled: flagSchema }),
		]),
		emptyResultSchema,
	),
	'repository.close': requestOperation(
		repositoryRequestSchema('repository.close'),
		emptyResultSchema,
	),
	'repository.consent': requestOperation(
		repositoryRequestSchema('repository.consent'),
		emptyResultSchema,
	),
	'request.outcome': requestOperation(
		z.strictObject({
			op: z.literal('request.outcome'),
			requestId: identitySchema,
			workspaceId: identitySchema,
		}),
		requestOutcomeResultSchema,
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

/** A Put, one piece of an upload behind its opening request, which a Result answers. */
export const FILES_PUT = {
	decode: (record: Uint8Array): FilesReply<null> | null => decodeResult(record, emptyResultSchema),
	encode: encodeFilesPutRecord,
};

/** An Events stream, which a Result answers only to fail. */
interface FilesEventsStream<Request, Snapshot, Update> {
	/** The failure a Result reported; null when the record reports none. */
	decodeFailure(record: Uint8Array): FilesError | null;
	/** A Snapshot or Update frame's notice; null for a notice this Client cannot read. */
	decodeNotice(
		kind: 'snapshot' | 'update',
		record: Uint8Array,
	):
		| { readonly kind: 'snapshot'; readonly snapshot: Snapshot }
		| { readonly kind: 'update'; readonly update: Update }
		| null;
	encode(request: Request): Uint8Array | null;
}

function eventsStream<RequestSchema extends z.ZodType, Snapshot, Update>(
	request: RequestSchema,
	snapshot: z.ZodType<Snapshot>,
	update: z.ZodType<Update>,
): FilesEventsStream<z.input<RequestSchema>, Snapshot, Update> {
	return {
		decodeFailure(record) {
			const failure = decodeResult(record, null);
			return failure?.kind === 'error' ? failure.error : null;
		},
		decodeNotice(kind, record) {
			const decoded = decodeFilesRecord(record, 'notice-frame');
			if (decoded?.kind !== 'notice') return null;
			const read = readFilesJson(decoded.body);
			if (read.kind === 'malformed') return null;
			if (kind === 'snapshot') {
				const parsed = snapshot.safeParse(read.value);
				return parsed.success ? { kind, snapshot: parsed.data } : null;
			}
			const parsed = update.safeParse(read.value);
			return parsed.success ? { kind, update: parsed.data } : null;
		},
		encode: (value) => encodeRequest(request, value),
	};
}

const subscribeRequestSchema = <Op extends 'fs.subscribe' | 'relay.subscribe' | 'scm.subscribe'>(
	op: Op,
) => z.strictObject({ op: z.literal(op), workspaceId: identitySchema });

/** `fs.subscribe`: the root's facts, then its changes. */
export const FILES_ROOT_STREAM = eventsStream(
	subscribeRequestSchema('fs.subscribe'),
	fsSnapshotSchema,
	fsUpdateSchema,
);

/** `scm.subscribe`: the whole Source Control summary, again on every change. */
export const FILES_SCM_STREAM = eventsStream(
	subscribeRequestSchema('scm.subscribe'),
	scmSummarySchema,
	scmSummarySchema,
);

/** `relay.subscribe`: the prompts open for this connection, then each prompt and its closing. */
export const FILES_RELAY_STREAM = eventsStream(
	subscribeRequestSchema('relay.subscribe'),
	relaySnapshotSchema,
	relayUpdateSchema,
);
