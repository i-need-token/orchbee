/**
 * The OBAP frame around the Agent reads this Client sends: a session list page, one session, and the
 * names people gave Agents.
 * `crates/agent-observation-protocol` is the peer binding, and
 * `schemas/agent-observation/v1/agent-observation.schema.json` owns the values; each shape here is
 * as closed as that schema's, so a record of another cohort fails here instead of further in.
 */
import { z } from 'zod';

const MAGIC = [0x4f, 0x42, 0x41, 0x50] as const; // OBAP
const VERSION = 1;
const HEADER_BYTES = 12;
const MAXIMUM_REQUEST_BYTES = 16 * 1024;
const LIST_REQUEST_KIND = 1;
const LIST_RESULT_KIND = 2;
const GET_REQUEST_KIND = 3;
const GET_RESULT_KIND = 4;
const NAMES_REQUEST_KIND = 9;
const NAMES_RESULT_KIND = 10;
/** The largest body each result kind may carry. */
const MAXIMUM_RESULT_BYTES: ReadonlyMap<number, number> = new Map([
	[LIST_RESULT_KIND, 1024 * 1024],
	[GET_RESULT_KIND, 128 * 1024],
	[NAMES_RESULT_KIND, 256 * 1024],
]);
/** Bindings and launch reservations together; the Server lists at most this many names. */
const MAXIMUM_NAMES = 512;

const SafeIntegerSchema = z.number().int().min(0).max(Number.MAX_SAFE_INTEGER);
const OpaqueIdentitySchema = z.string().regex(/^[A-Za-z0-9._:-]{1,128}$/u);
/** A request or Presentation identity: 32 lowercase hexadecimal characters, never all zero. */
const NonzeroIdentitySchema = z
	.string()
	.regex(/^[0-9a-f]{32}$/u)
	.refine((value) => /[1-9a-f]/u.test(value));
const ProviderIdSchema = z
	.string()
	.max(64)
	.regex(/^[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?$/u);
const CursorSchema = z
	.string()
	.max(107)
	.regex(/^v1\.[A-Za-z0-9_-]{59}\.[A-Za-z0-9_-]{43}$/u);
const ActivitySchema = z.enum([
	'thinking',
	'tool',
	'editing',
	'command',
	'subagent',
	'compacting',
	'retrying',
	'other',
]);
const PhaseSchema = z.enum(['queued', 'working', 'waiting-external']);
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
const LifecycleSchema = z.enum(['live', 'ended']);

const AttachmentSchema = z.strictObject({
	generation: OpaqueIdentitySchema,
	navigable: z.boolean(),
	presentationId: NonzeroIdentitySchema,
	visibility: z.enum(['visible', 'hidden', 'unknown']),
});
const ObservationSchema = z.strictObject({
	fidelity: z.enum(['rich', 'basic', 'heuristic']),
	freshness: z.enum(['fresh', 'stale', 'disconnected']),
	generation: SafeIntegerSchema,
	sequence: SafeIntegerSchema,
	source: z.enum(['native', 'hook', 'terminal', 'heuristic']),
});
const ExecutionLocationSchema = z.strictObject({ executionLocationId: OpaqueIdentitySchema });
const StatusSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		count: z.number().int().min(1).max(Number.MAX_SAFE_INTEGER),
		kind: z.literal('needs-attention'),
	}),
	z.strictObject({ kind: z.literal('failed') }),
	z.strictObject({ kind: z.literal('succeeded') }),
	z.strictObject({ kind: z.literal('canceled') }),
	z.strictObject({ kind: z.literal('interrupted') }),
	z.strictObject({
		activity: ActivitySchema.nullable(),
		kind: z.literal('working'),
		phase: PhaseSchema,
	}),
	z.strictObject({ kind: z.literal('idle') }),
	z.strictObject({ activityHint: ActivitySchema.nullable(), kind: z.literal('unknown') }),
	z.strictObject({ kind: z.literal('ended') }),
]);
const AttachmentsSchema = z.array(AttachmentSchema).max(32);

const SessionSummarySchema = z.strictObject({
	executionLocation: ExecutionLocationSchema,
	lifecycle: LifecycleSchema,
	observation: ObservationSchema,
	parentSessionId: OpaqueIdentitySchema.nullable(),
	pendingRequestCount: z.number().int().min(0).max(128),
	presentationAttachments: AttachmentsSchema,
	providerId: ProviderIdSchema,
	sessionId: OpaqueIdentitySchema,
	status: StatusSchema,
});

const CapabilitiesSchema = z
	.array(
		z.enum([
			'sessionLifecycle',
			'turnLifecycle',
			'questions',
			'approvals',
			'authentication',
			'activity',
			'subagents',
			'progress',
		]),
	)
	.max(10)
	.refine((capabilities) => new Set(capabilities).size === capabilities.length);

const SessionSnapshotSchema = z.strictObject({
	activityHint: ActivitySchema.nullable(),
	capabilities: CapabilitiesSchema,
	executionLocation: ExecutionLocationSchema,
	lastOutcome: z
		.strictObject({
			eventId: OpaqueIdentitySchema,
			kind: z.enum(['succeeded', 'failed', 'canceled', 'interrupted']),
			turnId: OpaqueIdentitySchema,
		})
		.nullable(),
	lifecycle: LifecycleSchema,
	observation: ObservationSchema,
	parentSessionId: OpaqueIdentitySchema.nullable(),
	pendingRequests: z
		.array(
			z.strictObject({
				actionPath: z.literal('navigate-only'),
				kind: z.enum(['question', 'approval', 'authentication', 'unclassified']),
				requestId: OpaqueIdentitySchema,
				turnId: OpaqueIdentitySchema.nullable(),
			}),
		)
		.max(128),
	presentationAttachments: AttachmentsSchema,
	providerId: ProviderIdSchema,
	sessionId: OpaqueIdentitySchema,
	turn: z.discriminatedUnion('kind', [
		z.strictObject({ kind: z.literal('unknown') }),
		z.strictObject({ kind: z.literal('idle') }),
		z.strictObject({
			activity: ActivitySchema.nullable(),
			kind: z.literal('active'),
			phase: PhaseSchema,
			turnId: OpaqueIdentitySchema,
		}),
	]),
});

const UnavailableSchema = z.strictObject({
	kind: z.literal('unavailable'),
	reason: z.literal('runtime-unavailable'),
});

const ListRequestSchema = z.strictObject({
	cursor: CursorSchema.optional(),
	includeEnded: z.boolean().optional(),
	kind: z.literal('agent.session.list.request'),
	limit: z.number().int().min(1).max(100).optional(),
	requestId: NonzeroIdentitySchema,
});
const GetRequestSchema = z.strictObject({
	kind: z.literal('agent.session.get.request'),
	requestId: NonzeroIdentitySchema,
	sessionId: OpaqueIdentitySchema,
});
const NamesRequestSchema = z.strictObject({
	kind: z.literal('agent.session.names.list.request'),
	requestId: NonzeroIdentitySchema,
});

const ListResultSchema = z.strictObject({
	kind: z.literal('agent.session.list.result'),
	requestId: NonzeroIdentitySchema,
	result: z.strictObject({
		outcome: z.discriminatedUnion('kind', [
			z.strictObject({
				items: z.array(SessionSummarySchema).max(100),
				kind: z.literal('listed'),
				nextCursor: CursorSchema.nullable(),
				revision: SafeIntegerSchema,
			}),
			z.strictObject({
				kind: z.literal('rejected'),
				reason: z.enum(['cursor-invalid', 'cursor-stale', 'read-denied']),
			}),
			UnavailableSchema,
		]),
	}),
});
const GetResultSchema = z.strictObject({
	kind: z.literal('agent.session.get.result'),
	requestId: NonzeroIdentitySchema,
	result: z.strictObject({
		outcome: z.discriminatedUnion('kind', [
			z.strictObject({
				kind: z.literal('found'),
				revision: SafeIntegerSchema,
				session: z.strictObject({ snapshot: SessionSnapshotSchema, status: StatusSchema }),
			}),
			z.strictObject({
				kind: z.literal('rejected'),
				reason: z.enum(['not-found', 'read-denied']),
			}),
			UnavailableSchema,
		]),
	}),
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

export type AgentSessionSummary = z.infer<typeof SessionSummarySchema>;
export type AgentReadRequest =
	| z.input<typeof ListRequestSchema>
	| z.input<typeof GetRequestSchema>
	| z.input<typeof NamesRequestSchema>;
export type AgentReadResult =
	| z.infer<typeof ListResultSchema>
	| z.infer<typeof GetResultSchema>
	| z.infer<typeof NamesResultSchema>;

const REQUEST_SHAPES = {
	'agent.session.get.request': { frameKind: GET_REQUEST_KIND, schema: GetRequestSchema },
	'agent.session.list.request': { frameKind: LIST_REQUEST_KIND, schema: ListRequestSchema },
	'agent.session.names.list.request': { frameKind: NAMES_REQUEST_KIND, schema: NamesRequestSchema },
} as const satisfies Record<AgentReadRequest['kind'], unknown>;
/** The frame kind chooses the shape, so the two always agree; null when the payload is not it. */
function parseResult(kind: number, payload: unknown): AgentReadResult | null {
	switch (kind) {
		case LIST_RESULT_KIND:
			return ListResultSchema.safeParse(payload).data ?? null;
		case GET_RESULT_KIND:
			return GetResultSchema.safeParse(payload).data ?? null;
		default:
			return NamesResultSchema.safeParse(payload).data ?? null;
	}
}
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
	const { frameKind, schema } = REQUEST_SHAPES[request.kind];
	const payload = utf8Encoder.encode(JSON.stringify(schema.parse(request)));
	if (payload.byteLength > MAXIMUM_REQUEST_BYTES) {
		throw new AgentCarrierError('OBAP request exceeds its bound.');
	}
	const frame = new Uint8Array(HEADER_BYTES + payload.byteLength);
	const view = new DataView(frame.buffer);
	frame.set(MAGIC);
	view.setUint16(4, VERSION);
	view.setUint16(6, frameKind);
	view.setUint32(8, payload.byteLength);
	frame.set(payload, HEADER_BYTES);
	return frame;
}

/** Decodes one complete read result; the frame kind chooses the shape, so the two always agree. */
export function decodeAgentReadResult(record: Uint8Array): AgentReadResult {
	const view = new DataView(record.buffer, record.byteOffset, record.byteLength);
	const kind = record.byteLength < HEADER_BYTES ? 0 : view.getUint16(6);
	const bound = MAXIMUM_RESULT_BYTES.get(kind) ?? 0;
	if (
		bound === 0 ||
		MAGIC.some((byte, index) => record[index] !== byte) ||
		view.getUint16(4) !== VERSION ||
		view.getUint32(8) > bound ||
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
	const parsed = parseResult(kind, payload);
	if (parsed === null) throw new AgentCarrierError('Invalid OBAP read result payload.');
	return parsed;
}
