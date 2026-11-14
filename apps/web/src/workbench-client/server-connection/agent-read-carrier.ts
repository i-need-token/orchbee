/**
 * The OBAP frame around the Agent read this Client sends: the names people gave Agents.
 * `crates/agent-observation-protocol` is the peer binding, and
 * `schemas/agent-observation/v1/agent-observation.schema.json` owns the values; each shape here is
 * as closed as that schema's, so a record of another cohort fails here instead of further in.
 */
import { z } from 'zod';

const MAGIC = [0x4f, 0x42, 0x41, 0x50] as const; // OBAP
const VERSION = 1;
const HEADER_BYTES = 12;
const MAXIMUM_REQUEST_BYTES = 16 * 1024;
const NAMES_REQUEST_KIND = 9;
const NAMES_RESULT_KIND = 10;
/** The largest body a names result may carry. */
const MAXIMUM_RESULT_BYTES = 256 * 1024;
/** Bindings and launch reservations together; the Server lists at most this many names. */
const MAXIMUM_NAMES = 512;

const OpaqueIdentitySchema = z.string().regex(/^[A-Za-z0-9._:-]{1,128}$/u);
/** A request or Presentation identity: 32 lowercase hexadecimal characters, never all zero. */
const NonzeroIdentitySchema = z
	.string()
	.regex(/^[0-9a-f]{32}$/u)
	.refine((value) => /[1-9a-f]/u.test(value));
const AgentNameSchema = z.string().regex(/^[a-z][a-z0-9_-]{0,31}$/u);
/** The exact terminal incarnation a name belongs to. */
const TerminalBindingSchema = z.strictObject({
	generation: NonzeroIdentitySchema,
	hostEpoch: NonzeroIdentitySchema,
	hostTerminalId: NonzeroIdentitySchema,
	presentationId: NonzeroIdentitySchema,
});
const LaunchReservationSchema = z.strictObject({
	binding: TerminalBindingSchema,
	name: AgentNameSchema,
	providerId: z
		.string()
		.max(64)
		.regex(/^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/u),
	remainingMs: z.number().int().min(1).max(300_000),
	reservationId: NonzeroIdentitySchema,
	settleRemainingMs: z.number().int().min(0).max(3000),
});
const UnavailableSchema = z.strictObject({
	kind: z.literal('unavailable'),
	reason: z.literal('runtime-unavailable'),
});

const NamesRequestSchema = z.strictObject({
	kind: z.literal('agent.session.names.list.request'),
	requestId: NonzeroIdentitySchema,
});

const NamesResultSchema = z.strictObject({
	kind: z.literal('agent.session.names.list.result'),
	requestId: NonzeroIdentitySchema,
	result: z.strictObject({
		outcome: z.discriminatedUnion('kind', [
			// A Server without terminal names lists sessions only.
			z
				.strictObject({
					bindings: z
						.array(
							z.strictObject({
								lifecycle: z.literal('live'),
								name: AgentNameSchema,
								sessionId: OpaqueIdentitySchema,
							}),
						)
						.max(MAXIMUM_NAMES),
					kind: z.literal('listed'),
					pending: z.array(LaunchReservationSchema).max(MAXIMUM_NAMES),
				})
				.refine(({ bindings, pending }) => bindings.length + pending.length <= MAXIMUM_NAMES),
			z.strictObject({ kind: z.literal('rejected'), reason: z.literal('read-denied') }),
			UnavailableSchema,
			z
				.strictObject({
					bindings: z
						.array(
							z.strictObject({
								name: AgentNameSchema,
								target: z.discriminatedUnion('kind', [
									z.strictObject({ kind: z.literal('session'), sessionId: OpaqueIdentitySchema }),
									z.strictObject({ binding: TerminalBindingSchema, kind: z.literal('terminal') }),
								]),
							}),
						)
						.max(MAXIMUM_NAMES),
					kind: z.literal('listed-targets'),
					pending: z.array(LaunchReservationSchema).max(MAXIMUM_NAMES),
				})
				.refine(({ bindings, pending }) => bindings.length + pending.length <= MAXIMUM_NAMES),
		]),
	}),
});

export type AgentReadRequest = z.input<typeof NamesRequestSchema>;
export type AgentReadResult = z.infer<typeof NamesResultSchema>;

const utf8Encoder = new TextEncoder();
const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

/** A Server of the same cohort never sends this. */
class AgentCarrierError extends Error {
	constructor(message: string) {
		super(message);
		this.name = 'AgentCarrierError';
	}
}

/** Encodes a read after checking it against the schema, so a malformed request never leaves. */
export function encodeAgentReadRequest(request: AgentReadRequest): Uint8Array {
	const payload = utf8Encoder.encode(JSON.stringify(NamesRequestSchema.parse(request)));
	if (payload.byteLength > MAXIMUM_REQUEST_BYTES) {
		throw new AgentCarrierError('OBAP request exceeds its bound.');
	}
	const frame = new Uint8Array(HEADER_BYTES + payload.byteLength);
	const view = new DataView(frame.buffer);
	frame.set(MAGIC);
	view.setUint16(4, VERSION);
	view.setUint16(6, NAMES_REQUEST_KIND);
	view.setUint32(8, payload.byteLength);
	frame.set(payload, HEADER_BYTES);
	return frame;
}

/** Decodes one complete read result; a frame of any other kind is refused. */
export function decodeAgentReadResult(record: Uint8Array): AgentReadResult {
	const view = new DataView(record.buffer, record.byteOffset, record.byteLength);
	const kind = record.byteLength < HEADER_BYTES ? 0 : view.getUint16(6);
	if (
		kind !== NAMES_RESULT_KIND ||
		MAGIC.some((byte, index) => record[index] !== byte) ||
		view.getUint16(4) !== VERSION ||
		view.getUint32(8) > MAXIMUM_RESULT_BYTES ||
		record.byteLength !== HEADER_BYTES + view.getUint32(8)
	) {
		throw new AgentCarrierError('Invalid OBAP read result frame.');
	}
	let payload: unknown;
	try {
		payload = JSON.parse(utf8Decoder.decode(record.subarray(HEADER_BYTES)));
	} catch {
		throw new AgentCarrierError('Invalid OBAP read result payload.');
	}
	const parsed = NamesResultSchema.safeParse(payload);
	if (!parsed.success) throw new AgentCarrierError('Invalid OBAP read result payload.');
	return parsed.data;
}
