import { readFile } from 'node:fs/promises';
import Ajv2020 from 'ajv/dist/2020';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';

import {
	FILES_OPERATIONS,
	FILES_READ_CHUNK,
	FILES_RELAY_STREAM,
	FILES_ROOT_STREAM,
	FILES_SCM_STREAM,
} from '../files-contract';
import { readFilesJson } from '../files-json';

async function readSchemaFile(name: string): Promise<unknown> {
	return JSON.parse(
		await readFile(new URL(`../../../../../../schemas/files/v1/${name}`, import.meta.url), 'utf8'),
	);
}

const documentCase = z.looseObject({
	json: z.unknown().optional(),
	kind: z.enum(['snapshot', 'update']).optional(),
	name: z.string(),
	op: z.string().optional(),
	text: z.string().optional(),
});
const documents = z.strictObject({
	invalid: z.array(documentCase),
	unsupported: z.array(documentCase).optional(),
	valid: z.array(documentCase),
});
const corpus = z
	.looseObject({ notices: documents, replies: documents, requests: documents })
	.parse(await readSchemaFile('files-conformance-v1.json'));
const validateRequest = new Ajv2020({ strict: false, validateFormats: false }).compile(
	z.record(z.string(), z.unknown()).parse(await readSchemaFile('files.schema.json')),
);

const encoder = new TextEncoder();
const RESULT = 2;
const NOTICE = 3;
const WORKSPACE = '0123456789abcdef0123456789abcdef';

function bodyOf({ json, text }: z.output<typeof documentCase>): Uint8Array {
	return encoder.encode(text ?? JSON.stringify(json));
}

/** Frames a body as the Server would, so it reaches the decoder a session uses. */
function frame(kind: number, body: Uint8Array): Uint8Array {
	const record = new Uint8Array(12 + body.byteLength);
	const view = new DataView(record.buffer);
	record.set([0x4f, 0x42, 0x46, 0x42]);
	view.setUint16(4, 1);
	view.setUint16(6, kind);
	view.setUint32(8, body.byteLength);
	record.set(body, 12);
	return record;
}

interface BoundStream {
	readonly decodeFailure: (record: Uint8Array) => unknown;
	readonly decodeNotice: (kind: 'snapshot' | 'update', record: Uint8Array) => unknown;
	readonly encode: (request: never) => Uint8Array | null;
}

const EVENTS_STREAMS: ReadonlyMap<string, BoundStream> = new Map<string, BoundStream>([
	['fs.subscribe', FILES_ROOT_STREAM],
	['relay.subscribe', FILES_RELAY_STREAM],
	['scm.subscribe', FILES_SCM_STREAM],
]);

/** How this binding reads each operation's Result. */
const replyReaders = new Map<string, (record: Uint8Array) => unknown>([
	...Object.entries(FILES_OPERATIONS).map(
		([op, operation]) => [op, (record: Uint8Array) => operation.decode(record)] as const,
	),
	['fs.read-chunk', (record) => FILES_READ_CHUNK.decode(record)],
	...[...EVENTS_STREAMS].map(
		([op, stream]) => [op, (record: Uint8Array) => stream.decodeFailure(record)] as const,
	),
]);

function readReply(document: z.output<typeof documentCase>): unknown {
	const read = replyReaders.get(document.op ?? '');
	if (read === undefined) throw new Error(`No reader for ${document.op ?? 'a missing op'}.`);
	return read(frame(RESULT, bodyOf(document)));
}

function readNotice(document: z.output<typeof documentCase>): unknown {
	const stream = EVENTS_STREAMS.get(document.op ?? '');
	if (stream === undefined) throw new Error(`No stream for ${document.op ?? 'a missing op'}.`);
	return stream.decodeNotice(document.kind ?? 'snapshot', frame(NOTICE, bodyOf(document)));
}

/** How this binding frames each operation's request. */
const requestEncoders = new Map<string, (request: never) => Uint8Array | null>([
	...Object.entries(FILES_OPERATIONS).map(
		([op, operation]) => [op, (request: never) => operation.encode(request)] as const,
	),
	['fs.read-chunk', (request) => FILES_READ_CHUNK.encode(request)],
	...[...EVENTS_STREAMS].map(
		([op, stream]) => [op, (request: never) => stream.encode(request)] as const,
	),
]);
/** Booleans a request requires, which its encoder writes even when false. */
const REQUIRED_FLAGS: ReadonlyMap<string, readonly string[]> = new Map([
	['client.focus', ['focused']],
	['git.commit', ['amend', 'noVerify', 'signoff']],
	['git.fetch', ['background', 'prune']],
	['git.pull', ['rebase']],
	['git.push', ['setUpstream', 'tags']],
	['git.sync', ['rebase']],
]);
const requestOp = z.looseObject({ op: z.string() });

function encodeRequest({ json }: z.output<typeof documentCase>): Uint8Array | null {
	const op = requestOp.parse(json).op;
	const encode = requestEncoders.get(op);
	if (encode === undefined) throw new Error(`No encoder for ${op}.`);
	return encode(json as never);
}

const boundRequest = ({ json, text }: z.output<typeof documentCase>): boolean => {
	const parsed = requestOp.safeParse(json);
	return text === undefined && parsed.success && requestEncoders.has(parsed.data.op);
};
const boundReply = ({ op }: z.output<typeof documentCase>): boolean => replyReaders.has(op ?? '');
const boundNotice = ({ op }: z.output<typeof documentCase>): boolean =>
	EVENTS_STREAMS.has(op ?? '');

describe('File Bar contract binding', () => {
	it.each(corpus.replies.valid.filter(boundReply))('admits the $op reply $name', (document) => {
		expect(readReply(document)).not.toBeNull();
	});

	it.each(corpus.replies.invalid.filter(boundReply))('refuses the $op reply $name', (document) => {
		expect(readReply(document)).toBeNull();
	});

	it.each(corpus.notices.valid.filter(boundNotice))('admits the $op notice $name', (document) => {
		expect(readNotice(document)).not.toBeNull();
	});

	it.each(corpus.notices.invalid.filter(boundNotice))(
		'refuses the $op notice $name',
		(document) => {
			expect(readNotice(document)).toBeNull();
		},
	);

	// The corpus's text cases break the JSON rules every body follows, whatever it carries.
	it.each(
		[...corpus.requests.invalid, ...corpus.replies.invalid].filter(
			({ text }) => text !== undefined,
		),
	)('refuses the JSON of $name', (document) => {
		expect(readFilesJson(bodyOf(document))).toEqual({ kind: 'malformed' });
	});

	it.each([
		['32 levels of nesting', `${'['.repeat(31)}${']'.repeat(31)}`, true],
		['33 levels of nesting', `${'['.repeat(32)}${']'.repeat(32)}`, false],
		['an escaped surrogate pair', '"\\ud83d\\ude00"', true],
		['an escaped lone surrogate', '"\\ud83d"', false],
		['a number beyond binary64', '1e400', false],
	])('reads a body with %s by the JSON rules', (_case, member, admitted) => {
		const read = readFilesJson(encoder.encode(`{"member":${member}}`));

		expect(read.kind === 'object').toBe(admitted);
	});

	it('reads an unknown error code as failed and an unknown reason as absent', () => {
		const read = (error: unknown) =>
			FILES_OPERATIONS['fs.stat'].decode(frame(RESULT, encoder.encode(JSON.stringify({ error }))));

		expect(read({ code: 'quota-exceeded', detail: 'x' })).toEqual({
			error: { code: 'failed', detail: 'x' },
			kind: 'error',
		});
		expect(read({ code: 'unavailable', reason: 'maintenance' })).toEqual({
			error: { code: 'unavailable' },
			kind: 'error',
		});
	});

	it.each(corpus.requests.valid.filter(boundRequest))('frames the request $name', (document) => {
		const record = encodeRequest(document);
		const json: unknown = JSON.parse(new TextDecoder().decode(record?.subarray(12)));

		expect(validateRequest(json)).toBe(true);
		// Encoders omit an optional boolean that is false.
		const members = z.record(z.string(), z.unknown()).parse(json);
		const required = REQUIRED_FLAGS.get(requestOp.parse(json).op) ?? [];
		const optional = Object.entries(members).filter(([name]) => !required.includes(name));
		expect(optional.map(([, value]) => value)).not.toContain(false);
	});

	it.each(corpus.requests.invalid.filter(boundRequest))(
		'refuses to frame the request $name',
		(document) => {
			expect(encodeRequest(document)).toBeNull();
		},
	);

	it.each([
		['a lone surrogate', ['/a\ud800']],
		[
			'a request beyond 16 KiB',
			Array.from({ length: 256 }, (_, index) => `/${index}`.padEnd(80, 'x')),
		],
	])('refuses to frame %s', (_case, paths) => {
		expect(
			FILES_OPERATIONS['fs.stat'].encode({ op: 'fs.stat', paths, workspaceId: WORKSPACE }),
		).toBeNull();
	});
});
