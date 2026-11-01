/**
 * The GUI Client's binding of the OBCS terminal contract: JSON control frames, the OBSR record a
 * Server returns for a Host record, and the OBCQ paste record. `schemas/client-server` is the
 * authority; its corpus checks this binding. Messages this Client never sends or reads, such as
 * `terminal.send`, stay out of the binding until a consumer needs them.
 */
import { z } from 'zod';

const FRAME_MAGIC = [0x4f, 0x42, 0x43, 0x53] as const; // OBCS
const RECORD_MAGIC = [0x4f, 0x42, 0x53, 0x52] as const; // OBSR
const PASTE_MAGIC = [0x4f, 0x42, 0x43, 0x51] as const; // OBCQ
const VERSION = 1;
const PROTOCOL_VERSION = 1;
const FRAME_HEADER_BYTES = 8;
const RECORD_HEADER_BYTES = 12;
const IDENTITY_BYTES = 16;
const MINIMUM_HOST_RECORD_BYTES = 64;
const MAXIMUM_HOST_RECORD_BYTES = 64 + 36 * 1024 * 1024;

export const MAXIMUM_CLIENT_SERVER_PAYLOAD_BYTES = 63 * 1024;
export const MAXIMUM_CONTROL_INPUT_BYTES = 4096;
/** One semantic input payload; its hexadecimal text must fit one OBCS frame. */
export const MAXIMUM_SEMANTIC_INPUT_BYTES = 16 * 1024;
export const MAXIMUM_PASTE_CONTENT_BYTES = 256 * 1024;

const utf8Encoder = new TextEncoder();
const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

const IdentitySchema = z
	.string()
	.regex(/^[0-9a-f]{32}$/u)
	.refine((value) => value !== '0'.repeat(32), 'Identity must be nonzero.');
const PositiveDecimalSchema = z
	.string()
	.regex(/^[1-9][0-9]{0,19}$/u)
	.refine((value) => BigInt(value) <= 0xffff_ffff_ffff_ffffn, 'Counter must fit 64 bits.');
const PositiveDimensionSchema = z.number().int().min(1).max(4096);
const CellPixelDimensionSchema = z.number().int().min(0).max(4096);

export const ClientServerGeometrySchema = z.strictObject({
	cellPixelHeight: CellPixelDimensionSchema,
	cellPixelWidth: CellPixelDimensionSchema,
	columns: PositiveDimensionSchema,
	rows: PositiveDimensionSchema,
});
export type ClientServerGeometry = z.infer<typeof ClientServerGeometrySchema>;

function resultEnvelope<const Kind extends string, Outcome extends z.ZodType>(
	kind: Kind,
	outcome: Outcome,
) {
	return z.strictObject({
		kind: z.literal(kind),
		outcome,
		protocolVersion: z.literal(PROTOCOL_VERSION),
		requestId: IdentitySchema,
	});
}

function reasonOutcome<
	const Kind extends string,
	const Reasons extends readonly [string, ...string[]],
>(kind: Kind, reasons: Reasons) {
	return z.strictObject({ kind: z.literal(kind), reason: z.enum(reasons) });
}

const OutcomeUnknownSchema = z.strictObject({ kind: z.literal('outcome-unknown') });

const CreateOutcomeSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('accepted'),
		state: z.discriminatedUnion('kind', [
			z.strictObject({ kind: z.literal('awaiting-start') }),
			z.strictObject({ geometry: ClientServerGeometrySchema, kind: z.literal('started') }),
		]),
	}),
	reasonOutcome('rejected', [
		'not-authorized',
		'worker-constraint-unsatisfied',
		'cwd-not-accepted',
	]),
	reasonOutcome('unavailable', [
		'no-compatible-worker',
		'terminal-runtime-unavailable',
		'server-busy',
	]),
	reasonOutcome('failed', ['terminal-create-failed']),
	OutcomeUnknownSchema,
]);

const ViewerOutcomeSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		attachmentId: IdentitySchema,
		creationId: IdentitySchema,
		hostEpoch: IdentitySchema,
		hostTerminalId: IdentitySchema,
		kind: z.literal('attached'),
	}),
	z.strictObject({ kind: z.literal('empty') }),
	z.strictObject({ kind: z.literal('feedback-accepted') }),
	z.strictObject({ kind: z.literal('detached') }),
	reasonOutcome('rejected', [
		'already-attached',
		'terminal-unavailable',
		'feedback-mismatch',
		'host-rejected',
	]),
	reasonOutcome('rows-rejected', [
		'stale-revision',
		'stale-generation',
		'history-evicted',
		'missing-resource',
		'invalid-field',
		'invalid-acknowledgement',
		'limit-exceeded',
		'resource-exhausted',
		'invalid-phase',
	]),
	reasonOutcome('unavailable', [
		'server-busy',
		'terminal-runtime-unavailable',
		'workbench-unavailable',
	]),
]);

const ControlOutcomeSchema = z.discriminatedUnion('kind', [
	z.strictObject({ controllerId: IdentitySchema, kind: z.literal('acquired') }),
	z.strictObject({ kind: z.literal('input-accepted') }),
	z.strictObject({ geometry: ClientServerGeometrySchema, kind: z.literal('resized') }),
	z.strictObject({
		acceptedBytes: z.number().int().min(1).max(1_048_576),
		kind: z.literal('pasted'),
	}),
	z.strictObject({ kind: z.literal('host-colors-accepted') }),
	z.strictObject({
		cleanupConfirmed: z.boolean(),
		generation: PositiveDecimalSchema,
		kind: z.literal('cleared'),
		revision: PositiveDecimalSchema,
	}),
	OutcomeUnknownSchema,
	reasonOutcome('rejected', [
		'viewer-unavailable',
		'controller-busy',
		'not-controller',
		'sequence-mismatch',
		'host-rejected',
		'host-clear-alternate-screen',
		'host-input-invalid',
		'host-input-limit-exceeded',
		'host-input-backpressure',
		'host-input-unavailable',
	]),
	reasonOutcome('unavailable', ['server-busy', 'terminal-runtime-unavailable']),
]);

const ServerMessageSchema = z.discriminatedUnion('kind', [
	resultEnvelope('terminal.create.result', CreateOutcomeSchema),
	resultEnvelope('terminal.viewer.result', ViewerOutcomeSchema),
	resultEnvelope('terminal.control.result', ControlOutcomeSchema),
]);

export type ClientServerCreateOutcome = z.infer<typeof CreateOutcomeSchema>;
export type ClientServerViewerOutcome = z.infer<typeof ViewerOutcomeSchema>;
export type ClientServerControlOutcome = z.infer<typeof ControlOutcomeSchema>;

export type ClientServerViewerAction =
	| { readonly kind: 'attach'; readonly terminalCreateRequestId: string }
	| {
			readonly kind: 'attach-binding';
			readonly sessionId: string;
			readonly terminalId: string;
			readonly workerId: string;
	  }
	| { readonly attachmentId: string; readonly kind: 'poll' | 'read-process-info' | 'detach' }
	| {
			readonly attachmentId: string;
			readonly generation: string;
			readonly kind: 'frame-applied';
			readonly revision: string;
			readonly sequence: string;
	  }
	| {
			readonly attachmentId: string;
			readonly kind: 'need-snapshot';
			readonly reason: string;
			readonly sequence: string;
	  }
	| {
			readonly attachmentId: string;
			readonly detail?: 'render';
			readonly generation: string;
			readonly kind: 'read-rows';
			readonly maximumBytes: number;
			readonly requestSequence: string;
			readonly revision: string;
			readonly rowCount: number;
			readonly startRow: string;
	  }
	| {
			readonly attachmentId: string;
			readonly demand: 'active' | 'paused';
			readonly kind: 'set-projection-demand';
	  }
	| {
			readonly attachmentId: string;
			readonly demand: 'following-bottom' | 'viewing-history';
			readonly kind: 'set-history-demand';
	  }
	| {
			readonly attachmentId: string;
			readonly detail?: 'render';
			readonly generation: string;
			readonly kind: 'read-viewport-rows';
			readonly maximumBytes: number;
			readonly origin:
				| { readonly kind: 'tail'; readonly rowsBefore: number }
				| { readonly delta: number; readonly kind: 'committed' };
			readonly requestSequence: string;
			readonly revision: string;
			readonly rowCount: number;
	  }
	| {
			readonly attachmentId: string;
			readonly generation: string;
			readonly kind: 'rows-applied';
			readonly requestSequence: string;
			readonly revision: string;
	  }
	| {
			readonly attachmentId: string;
			readonly generation: string;
			readonly kind: 'history-observed';
			readonly revision: string;
			readonly sequence: string;
	  };

/** The Host semantic input record kinds a `semantic-input` control action may carry. */
export type ClientServerSemanticInputKind = 'focus' | 'key' | 'mouse' | 'text';

export type ClientServerControlAction =
	| { readonly attachmentId: string; readonly kind: 'acquire' | 'clear' | 'takeover' }
	| {
			readonly attachmentId: string;
			readonly bytes: string;
			readonly controllerId: string;
			readonly kind: 'input';
			readonly sequence: string;
	  }
	| {
			readonly attachmentId: string;
			readonly controllerId: string;
			readonly inputKind: ClientServerSemanticInputKind;
			readonly kind: 'semantic-input';
			readonly payload: string;
			readonly sequence: string;
	  }
	| {
			readonly attachmentId: string;
			readonly controllerId: string;
			readonly geometry: ClientServerGeometry;
			readonly kind: 'resize';
			readonly sequence: string;
	  }
	| {
			readonly attachmentId: string;
			readonly background: number;
			readonly controllerId: string;
			readonly foreground: number;
			readonly kind: 'host-colors';
			readonly sequence: string;
	  };

export type ClientServerRequest =
	| {
			readonly cwd: string | null;
			readonly kind: 'terminal.create';
			readonly requestId: string;
			readonly startMode:
				| { readonly geometry: ClientServerGeometry; readonly kind: 'immediate' }
				| { readonly kind: 'deferred' };
			readonly workerConstraint: { readonly kind: 'exact'; readonly workerId: string } | null;
	  }
	| {
			readonly action: ClientServerViewerAction;
			readonly kind: 'terminal.viewer';
			readonly requestId: string;
	  }
	| {
			readonly action: ClientServerControlAction;
			readonly kind: 'terminal.control';
			readonly requestId: string;
	  };

export type ClientServerResponse =
	| z.infer<typeof ServerMessageSchema>
	| {
			/** The opaque OBTP record; valid only as long as the caller retains this response. */
			readonly hostRecord: Uint8Array;
			readonly kind: 'terminal.viewer.record';
			readonly requestId: string;
	  };

/** A response the binding refuses. The exact-cohort rule makes this a fatal carrier defect. */
export class ClientServerProtocolError extends Error {
	constructor(message: string) {
		super(message);
		this.name = 'ClientServerProtocolError';
	}
}

function hasPrefix(bytes: Uint8Array, prefix: readonly number[]): boolean {
	return prefix.every((value, index) => bytes[index] === value);
}

/** A fresh nonzero request identity; a zero draw is astronomically unlikely but not valid. */
export function createClientServerRequestId(): string {
	const bytes = crypto.getRandomValues(new Uint8Array(IDENTITY_BYTES));
	bytes[0] = (bytes[0] ?? 0) | 1;
	return bytesToLowercaseHex(bytes);
}

export function bytesToLowercaseHex(bytes: Uint8Array): string {
	let text = '';
	for (const byte of bytes) text += byte.toString(16).padStart(2, '0');
	return text;
}

function writeIdentity(target: Uint8Array, offset: number, identity: string): void {
	for (let index = 0; index < IDENTITY_BYTES; index += 1) {
		target[offset + index] = Number.parseInt(identity.slice(index * 2, index * 2 + 2), 16);
	}
}

export function encodeClientServerRequest(request: ClientServerRequest): Uint8Array {
	const payload = utf8Encoder.encode(
		JSON.stringify({ ...request, protocolVersion: PROTOCOL_VERSION }),
	);
	if (payload.byteLength > MAXIMUM_CLIENT_SERVER_PAYLOAD_BYTES) {
		throw new ClientServerProtocolError('OBCS request exceeds its frame limit.');
	}
	const frame = new Uint8Array(FRAME_HEADER_BYTES + payload.byteLength);
	const view = new DataView(frame.buffer);
	frame.set(FRAME_MAGIC);
	view.setUint16(4, VERSION);
	view.setUint16(6, payload.byteLength);
	frame.set(payload, FRAME_HEADER_BYTES);
	return frame;
}

export interface ClientServerPasteRecord {
	readonly attachmentId: string;
	readonly content: Uint8Array;
	readonly controllerId: string;
	readonly requestId: string;
	readonly sequence: bigint;
}

/** Paste content is never carried by OBCS JSON; a small paste uses this record as well. */
export function encodeClientServerPaste(record: ClientServerPasteRecord): Uint8Array {
	const length = record.content.byteLength;
	if (length === 0 || length > MAXIMUM_PASTE_CONTENT_BYTES) {
		throw new ClientServerProtocolError('Paste content is out of range.');
	}
	const bodyLength = IDENTITY_BYTES * 3 + 8 + length;
	const bytes = new Uint8Array(RECORD_HEADER_BYTES + bodyLength);
	const view = new DataView(bytes.buffer);
	bytes.set(PASTE_MAGIC);
	view.setUint16(4, VERSION);
	view.setUint32(8, bodyLength);
	writeIdentity(bytes, RECORD_HEADER_BYTES, record.requestId);
	writeIdentity(bytes, RECORD_HEADER_BYTES + IDENTITY_BYTES, record.attachmentId);
	writeIdentity(bytes, RECORD_HEADER_BYTES + IDENTITY_BYTES * 2, record.controllerId);
	view.setBigUint64(RECORD_HEADER_BYTES + IDENTITY_BYTES * 3, record.sequence);
	bytes.set(record.content, RECORD_HEADER_BYTES + IDENTITY_BYTES * 3 + 8);
	return bytes;
}

function decodeJsonFrame(bytes: Uint8Array): ClientServerResponse {
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	if (bytes.byteLength < FRAME_HEADER_BYTES || view.getUint16(4) !== VERSION) {
		throw new ClientServerProtocolError('Invalid OBCS header.');
	}
	const length = view.getUint16(6);
	if (
		length > MAXIMUM_CLIENT_SERVER_PAYLOAD_BYTES ||
		bytes.byteLength !== FRAME_HEADER_BYTES + length
	) {
		throw new ClientServerProtocolError('Invalid OBCS length.');
	}
	const parsed = parseServerMessage(bytes.subarray(FRAME_HEADER_BYTES));
	if (!parsed.success) throw new ClientServerProtocolError('Invalid OBCS message.');
	return parsed.data;
}

function parseServerMessage(payload: Uint8Array) {
	let value: unknown;
	try {
		value = JSON.parse(utf8Decoder.decode(payload));
	} catch {
		throw new ClientServerProtocolError('OBCS payload is not JSON.');
	}
	return ServerMessageSchema.safeParse(value);
}

function decodeHostRecord(bytes: Uint8Array): ClientServerResponse {
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	if (
		bytes.byteLength < RECORD_HEADER_BYTES ||
		view.getUint16(4) !== VERSION ||
		view.getUint16(6) !== 0
	) {
		throw new ClientServerProtocolError('Invalid OBSR header.');
	}
	const length = view.getUint32(8);
	if (
		length < IDENTITY_BYTES + MINIMUM_HOST_RECORD_BYTES ||
		length > IDENTITY_BYTES + MAXIMUM_HOST_RECORD_BYTES ||
		bytes.byteLength !== RECORD_HEADER_BYTES + length
	) {
		throw new ClientServerProtocolError('Invalid OBSR length.');
	}
	const body = RECORD_HEADER_BYTES + IDENTITY_BYTES;
	return {
		hostRecord: bytes.subarray(body),
		kind: 'terminal.viewer.record',
		requestId: bytesToLowercaseHex(bytes.subarray(RECORD_HEADER_BYTES, body)),
	};
}

/** Decodes one complete response record received as a stream result. */
export function decodeClientServerResponse(bytes: Uint8Array): ClientServerResponse {
	if (hasPrefix(bytes, RECORD_MAGIC)) return decodeHostRecord(bytes);
	if (hasPrefix(bytes, FRAME_MAGIC)) return decodeJsonFrame(bytes);
	throw new ClientServerProtocolError('Unknown response record.');
}

/** Validates one response payload without its frame; the shared corpus uses this boundary. */
export function clientServerResponsePayloadIsValid(payload: Uint8Array): boolean {
	try {
		return parseServerMessage(payload).success;
	} catch {
		return false;
	}
}
