import { z } from 'zod';

export const SSH_ENVIRONMENT_FRAME_MAGIC = 'OBSE';
export const SSH_ENVIRONMENT_FRAME_HEADER_BYTES = 16;
export const SSH_ENVIRONMENT_MAX_FRAME_BYTES = 1024 * 1024;
export const SSH_ENVIRONMENT_PROTOCOL_VERSION = Object.freeze({
	major: 0,
	minor: 2,
	patch: 0,
});
const SSH_PROJECT_ROOT_SERVICE_ID_LITERAL = 'project-root-v1';
const SSH_TERMINAL_HOST_SERVICE_ID_LITERAL = 'terminal-host-v1';
export const SSH_TERMINAL_HOST_INITIAL_CREDIT_BYTES = 65_536;
export const SSH_TERMINAL_HOST_MAX_CHUNK_BYTES = 65_536;

const MAX_FRAME_PAYLOAD_BYTES =
	SSH_ENVIRONMENT_MAX_FRAME_BYTES - SSH_ENVIRONMENT_FRAME_HEADER_BYTES;
const MAX_CHANNEL_DATA_BYTES = MAX_FRAME_PAYLOAD_BYTES - Uint32Array.BYTES_PER_ELEMENT;
const MAX_CREDIT_BYTES = 16 * 1024 * 1024;
const MAX_RUNTIME_VERSION_LENGTH = 64;
const MAX_FAILURE_DETAIL_LENGTH = 1024;

const unsignedIntegerSchema = z.number().int().min(0).max(0xffff_ffff);
const positiveUnsignedIntegerSchema = unsignedIntegerSchema.min(1);
const boundedByteCountSchema = positiveUnsignedIntegerSchema.max(MAX_CREDIT_BYTES);

export const SshEnvironmentProtocolVersionSchema = z.strictObject({
	major: z.literal(SSH_ENVIRONMENT_PROTOCOL_VERSION.major),
	minor: z.literal(SSH_ENVIRONMENT_PROTOCOL_VERSION.minor),
	patch: z.literal(SSH_ENVIRONMENT_PROTOCOL_VERSION.patch),
});
export const SshEnvironmentRuntimeInstanceIdSchema = z
	.string()
	.length(32)
	.regex(/^[0-9a-f]+$/u)
	.brand<'SshEnvironmentRuntimeInstanceId'>();
export const SshEnvironmentRuntimeEpochSchema = z
	.string()
	.length(32)
	.regex(/^[0-9a-f]+$/u)
	.brand<'SshEnvironmentRuntimeEpoch'>();
export const SshEnvironmentClientNonceSchema = z
	.string()
	.length(32)
	.regex(/^[0-9a-f]+$/u)
	.brand<'SshEnvironmentClientNonce'>();
export const SshEnvironmentBuildDigestSchema = z
	.string()
	.length(64)
	.regex(/^[0-9a-f]+$/u)
	.brand<'SshEnvironmentBuildDigest'>();
export const SshEnvironmentReleaseCohortDigestSchema = z
	.string()
	.length(64)
	.regex(/^[0-9a-f]+$/u)
	.brand<'SshEnvironmentReleaseCohortDigest'>();
export const SshEnvironmentRuntimeVersionSchema = z
	.string()
	.min(1)
	.max(MAX_RUNTIME_VERSION_LENGTH)
	.regex(/^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$/u)
	.brand<'SshEnvironmentRuntimeVersion'>();
export const SshEnvironmentTerminalHostVersionSchema = z
	.string()
	.min(1)
	.max(MAX_RUNTIME_VERSION_LENGTH)
	.regex(/^[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z.-]+)?$/u)
	.brand<'SshEnvironmentTerminalHostVersion'>();
export const SshEnvironmentPlatformSchema = z.enum(['darwin-arm64', 'linux-x64']);
export const SshEnvironmentCapabilitySchema = z.union([
	z.literal(SSH_PROJECT_ROOT_SERVICE_ID_LITERAL),
	z.literal(SSH_TERMINAL_HOST_SERVICE_ID_LITERAL),
]);
export const SshEnvironmentServiceIdSchema =
	SshEnvironmentCapabilitySchema.brand<'SshEnvironmentServiceId'>();
export const SSH_PROJECT_ROOT_SERVICE_ID = SshEnvironmentServiceIdSchema.parse(
	SSH_PROJECT_ROOT_SERVICE_ID_LITERAL,
);
export const SSH_TERMINAL_HOST_SERVICE_ID = SshEnvironmentServiceIdSchema.parse(
	SSH_TERMINAL_HOST_SERVICE_ID_LITERAL,
);
export const SSH_ENVIRONMENT_REQUIRED_CAPABILITIES = Object.freeze([
	SSH_PROJECT_ROOT_SERVICE_ID,
	SSH_TERMINAL_HOST_SERVICE_ID,
] as const);
export const SshEnvironmentChannelIdSchema =
	positiveUnsignedIntegerSchema.brand<'SshEnvironmentChannelId'>();

export const SshEnvironmentHelloSchema = z.strictObject({
	protocolVersion: SshEnvironmentProtocolVersionSchema,
	runtimeInstanceId: SshEnvironmentRuntimeInstanceIdSchema,
	clientNonce: SshEnvironmentClientNonceSchema,
	maxFrameBytes: z.literal(SSH_ENVIRONMENT_MAX_FRAME_BYTES),
	initialChannelCreditBytes: boundedByteCountSchema,
});
export const SshEnvironmentHelloAcceptedSchema = z.strictObject({
	protocolVersion: SshEnvironmentProtocolVersionSchema,
	runtimeInstanceId: SshEnvironmentRuntimeInstanceIdSchema,
	runtimeEpoch: SshEnvironmentRuntimeEpochSchema,
	runtimeVersion: SshEnvironmentRuntimeVersionSchema,
	runtimeBuildDigest: SshEnvironmentBuildDigestSchema,
	releaseCohortDigest: SshEnvironmentReleaseCohortDigestSchema,
	terminalHostVersion: SshEnvironmentTerminalHostVersionSchema,
	terminalHostBuildDigest: SshEnvironmentBuildDigestSchema,
	platform: SshEnvironmentPlatformSchema,
	capabilities: z.array(SshEnvironmentCapabilitySchema).max(16),
	heartbeatIntervalMs: z.number().int().min(5_000).max(60_000),
	heartbeatMissLimit: z.number().int().min(1).max(10),
	maxFrameBytes: z.literal(SSH_ENVIRONMENT_MAX_FRAME_BYTES),
	initialChannelCreditBytes: boundedByteCountSchema,
});
export const SshEnvironmentHeartbeatSchema = z.strictObject({
	runtimeEpoch: SshEnvironmentRuntimeEpochSchema,
	sequence: unsignedIntegerSchema,
});
export const SshEnvironmentHeartbeatAcknowledgedSchema = z.strictObject({
	runtimeEpoch: SshEnvironmentRuntimeEpochSchema,
	sequence: unsignedIntegerSchema,
});
export const SshEnvironmentFailureSchema = z.strictObject({
	code: z.enum([
		'protocol-invalid',
		'protocol-version-unsupported',
		'runtime-instance-mismatch',
		'runtime-attachment-conflict',
		'compatibility-unsupported',
		'service-unsupported',
		'channel-invalid',
		'internal-failure',
	]),
	detail: z.string().min(1).max(MAX_FAILURE_DETAIL_LENGTH).optional(),
});
export const SshEnvironmentChannelOpenSchema = z.strictObject({
	runtimeEpoch: SshEnvironmentRuntimeEpochSchema,
	channelId: SshEnvironmentChannelIdSchema,
	serviceId: SshEnvironmentServiceIdSchema,
	initialCreditBytes: boundedByteCountSchema,
});
export const SshEnvironmentChannelOpenedSchema = z.strictObject({
	runtimeEpoch: SshEnvironmentRuntimeEpochSchema,
	channelId: SshEnvironmentChannelIdSchema,
	initialCreditBytes: boundedByteCountSchema,
});
export const SshEnvironmentChannelCloseSchema = z.strictObject({
	runtimeEpoch: SshEnvironmentRuntimeEpochSchema,
	channelId: SshEnvironmentChannelIdSchema,
	reason: z.enum([
		'completed',
		'cancelled',
		'attachment-lost',
		'protocol-failure',
		'service-busy',
		'service-unavailable',
		'compatibility-unsupported',
	]),
});
export const SshEnvironmentChannelClosedSchema = z.strictObject({
	runtimeEpoch: SshEnvironmentRuntimeEpochSchema,
	channelId: SshEnvironmentChannelIdSchema,
	reason: z.enum([
		'completed',
		'cancelled',
		'attachment-lost',
		'protocol-failure',
		'service-busy',
		'service-unavailable',
		'compatibility-unsupported',
	]),
});

export type SshEnvironmentProtocolVersion = z.infer<typeof SshEnvironmentProtocolVersionSchema>;
export type SshEnvironmentRuntimeInstanceId = z.infer<typeof SshEnvironmentRuntimeInstanceIdSchema>;
export type SshEnvironmentRuntimeEpoch = z.infer<typeof SshEnvironmentRuntimeEpochSchema>;
export type SshEnvironmentClientNonce = z.infer<typeof SshEnvironmentClientNonceSchema>;
export type SshEnvironmentBuildDigest = z.infer<typeof SshEnvironmentBuildDigestSchema>;
export type SshEnvironmentReleaseCohortDigest = z.infer<
	typeof SshEnvironmentReleaseCohortDigestSchema
>;
export type SshEnvironmentRuntimeVersion = z.infer<typeof SshEnvironmentRuntimeVersionSchema>;
export type SshEnvironmentTerminalHostVersion = z.infer<
	typeof SshEnvironmentTerminalHostVersionSchema
>;
export type SshEnvironmentPlatform = z.infer<typeof SshEnvironmentPlatformSchema>;
export type SshEnvironmentCapability = z.infer<typeof SshEnvironmentCapabilitySchema>;
export type SshEnvironmentServiceId = z.infer<typeof SshEnvironmentServiceIdSchema>;
export type SshEnvironmentChannelId = z.infer<typeof SshEnvironmentChannelIdSchema>;
export type SshEnvironmentHello = z.infer<typeof SshEnvironmentHelloSchema>;
export type SshEnvironmentHelloAccepted = z.infer<typeof SshEnvironmentHelloAcceptedSchema>;
export type SshEnvironmentHeartbeat = z.infer<typeof SshEnvironmentHeartbeatSchema>;
export type SshEnvironmentHeartbeatAcknowledged = z.infer<
	typeof SshEnvironmentHeartbeatAcknowledgedSchema
>;
export type SshEnvironmentFailure = z.infer<typeof SshEnvironmentFailureSchema>;
export type SshEnvironmentChannelOpen = z.infer<typeof SshEnvironmentChannelOpenSchema>;
export type SshEnvironmentChannelOpened = z.infer<typeof SshEnvironmentChannelOpenedSchema>;
export type SshEnvironmentChannelClose = z.infer<typeof SshEnvironmentChannelCloseSchema>;
export type SshEnvironmentChannelClosed = z.infer<typeof SshEnvironmentChannelClosedSchema>;

export type SshEnvironmentMessage =
	| { readonly type: 'hello'; readonly payload: SshEnvironmentHello }
	| { readonly type: 'hello-accepted'; readonly payload: SshEnvironmentHelloAccepted }
	| { readonly type: 'heartbeat'; readonly payload: SshEnvironmentHeartbeat }
	| {
			readonly type: 'heartbeat-acknowledged';
			readonly payload: SshEnvironmentHeartbeatAcknowledged;
	  }
	| { readonly type: 'failure'; readonly payload: SshEnvironmentFailure }
	| { readonly type: 'channel-open'; readonly payload: SshEnvironmentChannelOpen }
	| { readonly type: 'channel-opened'; readonly payload: SshEnvironmentChannelOpened }
	| {
			readonly type: 'channel-data';
			readonly channelId: SshEnvironmentChannelId;
			readonly data: Uint8Array;
	  }
	| {
			readonly type: 'channel-credit';
			readonly channelId: SshEnvironmentChannelId;
			readonly bytes: number;
	  }
	| { readonly type: 'channel-close'; readonly payload: SshEnvironmentChannelClose }
	| { readonly type: 'channel-closed'; readonly payload: SshEnvironmentChannelClosed };

const FRAME_KIND = Object.freeze({
	hello: 1,
	'hello-accepted': 2,
	heartbeat: 3,
	'heartbeat-acknowledged': 4,
	failure: 5,
	'channel-open': 16,
	'channel-opened': 17,
	'channel-data': 18,
	'channel-credit': 19,
	'channel-close': 20,
	'channel-closed': 21,
} satisfies Readonly<Record<SshEnvironmentMessage['type'], number>>);

type JsonMessage = Exclude<
	SshEnvironmentMessage,
	{ readonly type: 'channel-data' | 'channel-credit' }
>;

const JSON_PAYLOAD_SCHEMAS = Object.freeze({
	hello: SshEnvironmentHelloSchema,
	'hello-accepted': SshEnvironmentHelloAcceptedSchema,
	heartbeat: SshEnvironmentHeartbeatSchema,
	'heartbeat-acknowledged': SshEnvironmentHeartbeatAcknowledgedSchema,
	failure: SshEnvironmentFailureSchema,
	'channel-open': SshEnvironmentChannelOpenSchema,
	'channel-opened': SshEnvironmentChannelOpenedSchema,
	'channel-close': SshEnvironmentChannelCloseSchema,
	'channel-closed': SshEnvironmentChannelClosedSchema,
} satisfies Readonly<Record<JsonMessage['type'], z.ZodType>>);

export interface SshEnvironmentDecodeError {
	readonly code:
		| 'frame-too-short'
		| 'frame-too-large'
		| 'frame-length-mismatch'
		| 'magic-invalid'
		| 'version-unsupported'
		| 'reserved-bytes-nonzero'
		| 'kind-unsupported'
		| 'payload-invalid';
	readonly detail: string;
}

export type SshEnvironmentDecodeResult =
	| { readonly ok: true; readonly message: SshEnvironmentMessage }
	| { readonly ok: false; readonly error: SshEnvironmentDecodeError };

const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder('utf-8', { fatal: true });

function encodeJsonMessage(message: JsonMessage): Uint8Array {
	const schema = JSON_PAYLOAD_SCHEMAS[message.type];
	const payload = schema.parse(message.payload);
	return textEncoder.encode(JSON.stringify(payload));
}

function encodeChannelData(channelId: SshEnvironmentChannelId, data: Uint8Array): Uint8Array {
	SshEnvironmentChannelIdSchema.parse(channelId);
	if (data.byteLength > MAX_CHANNEL_DATA_BYTES) {
		throw new RangeError(`channel data exceeds ${MAX_CHANNEL_DATA_BYTES} bytes`);
	}

	const payload = new Uint8Array(Uint32Array.BYTES_PER_ELEMENT + data.byteLength);
	new DataView(payload.buffer).setUint32(0, channelId, false);
	payload.set(data, Uint32Array.BYTES_PER_ELEMENT);
	return payload;
}

function encodeChannelCredit(channelId: SshEnvironmentChannelId, bytes: number): Uint8Array {
	SshEnvironmentChannelIdSchema.parse(channelId);
	boundedByteCountSchema.parse(bytes);
	const payload = new Uint8Array(Uint32Array.BYTES_PER_ELEMENT * 2);
	const view = new DataView(payload.buffer);
	view.setUint32(0, channelId, false);
	view.setUint32(Uint32Array.BYTES_PER_ELEMENT, bytes, false);
	return payload;
}

export function encodeSshEnvironmentMessage(message: SshEnvironmentMessage): Uint8Array {
	let payload: Uint8Array;
	if (message.type === 'channel-data') {
		payload = encodeChannelData(message.channelId, message.data);
	} else if (message.type === 'channel-credit') {
		payload = encodeChannelCredit(message.channelId, message.bytes);
	} else {
		payload = encodeJsonMessage(message);
	}

	const frameLength = SSH_ENVIRONMENT_FRAME_HEADER_BYTES + payload.byteLength;
	if (frameLength > SSH_ENVIRONMENT_MAX_FRAME_BYTES) {
		throw new RangeError(`frame exceeds ${SSH_ENVIRONMENT_MAX_FRAME_BYTES} bytes`);
	}

	const frame = new Uint8Array(frameLength);
	frame.set(textEncoder.encode(SSH_ENVIRONMENT_FRAME_MAGIC), 0);
	frame[4] = SSH_ENVIRONMENT_PROTOCOL_VERSION.major;
	frame[5] = SSH_ENVIRONMENT_PROTOCOL_VERSION.minor;
	frame[6] = SSH_ENVIRONMENT_PROTOCOL_VERSION.patch;
	frame[7] = FRAME_KIND[message.type];
	new DataView(frame.buffer).setUint32(8, payload.byteLength, false);
	frame.set(payload, SSH_ENVIRONMENT_FRAME_HEADER_BYTES);
	return frame;
}

function decodeError(
	code: SshEnvironmentDecodeError['code'],
	detail: string,
): Extract<SshEnvironmentDecodeResult, { readonly ok: false }> {
	return { ok: false, error: { code, detail } };
}

type JsonPayloadDecodeResult<Payload> =
	| { readonly ok: true; readonly payload: Payload }
	| Extract<SshEnvironmentDecodeResult, { readonly ok: false }>;

function decodeJsonPayload<Payload>(
	type: string,
	schema: z.ZodType<Payload>,
	payloadBytes: Uint8Array,
): JsonPayloadDecodeResult<Payload> {
	try {
		const candidate: unknown = JSON.parse(textDecoder.decode(payloadBytes));
		const parsed = schema.safeParse(candidate);
		if (!parsed.success) {
			return decodeError('payload-invalid', `invalid ${type} payload`);
		}
		return { ok: true, payload: parsed.data };
	} catch {
		return decodeError('payload-invalid', `invalid ${type} JSON payload`);
	}
}

function decodeChannelData(payload: Uint8Array): SshEnvironmentDecodeResult {
	if (payload.byteLength < Uint32Array.BYTES_PER_ELEMENT) {
		return decodeError('payload-invalid', 'channel-data payload is missing its channel id');
	}
	const channelIdResult = SshEnvironmentChannelIdSchema.safeParse(
		new DataView(payload.buffer, payload.byteOffset, payload.byteLength).getUint32(0, false),
	);
	if (!channelIdResult.success) {
		return decodeError('payload-invalid', 'channel-data contains an invalid channel id');
	}
	return {
		ok: true,
		message: {
			type: 'channel-data',
			channelId: channelIdResult.data,
			data: payload.slice(Uint32Array.BYTES_PER_ELEMENT),
		},
	};
}

function decodeChannelCredit(payload: Uint8Array): SshEnvironmentDecodeResult {
	if (payload.byteLength !== Uint32Array.BYTES_PER_ELEMENT * 2) {
		return decodeError('payload-invalid', 'channel-credit payload must be exactly 8 bytes');
	}
	const view = new DataView(payload.buffer, payload.byteOffset, payload.byteLength);
	const channelIdResult = SshEnvironmentChannelIdSchema.safeParse(view.getUint32(0, false));
	const bytesResult = boundedByteCountSchema.safeParse(
		view.getUint32(Uint32Array.BYTES_PER_ELEMENT, false),
	);
	if (!channelIdResult.success || !bytesResult.success) {
		return decodeError('payload-invalid', 'channel-credit contains an invalid value');
	}
	return {
		ok: true,
		message: {
			type: 'channel-credit',
			channelId: channelIdResult.data,
			bytes: bytesResult.data,
		},
	};
}

function decodePayload(kind: number, payload: Uint8Array): SshEnvironmentDecodeResult {
	switch (kind) {
		case FRAME_KIND.hello: {
			const decoded = decodeJsonPayload('hello', SshEnvironmentHelloSchema, payload);
			if (!decoded.ok) return decoded;
			return { ok: true, message: { type: 'hello', payload: decoded.payload } };
		}
		case FRAME_KIND['hello-accepted']: {
			const decoded = decodeJsonPayload(
				'hello-accepted',
				SshEnvironmentHelloAcceptedSchema,
				payload,
			);
			if (!decoded.ok) return decoded;
			return { ok: true, message: { type: 'hello-accepted', payload: decoded.payload } };
		}
		case FRAME_KIND.heartbeat: {
			const decoded = decodeJsonPayload('heartbeat', SshEnvironmentHeartbeatSchema, payload);
			if (!decoded.ok) return decoded;
			return { ok: true, message: { type: 'heartbeat', payload: decoded.payload } };
		}
		case FRAME_KIND['heartbeat-acknowledged']: {
			const decoded = decodeJsonPayload(
				'heartbeat-acknowledged',
				SshEnvironmentHeartbeatAcknowledgedSchema,
				payload,
			);
			if (!decoded.ok) return decoded;
			return {
				ok: true,
				message: { type: 'heartbeat-acknowledged', payload: decoded.payload },
			};
		}
		case FRAME_KIND.failure: {
			const decoded = decodeJsonPayload('failure', SshEnvironmentFailureSchema, payload);
			if (!decoded.ok) return decoded;
			return { ok: true, message: { type: 'failure', payload: decoded.payload } };
		}
		case FRAME_KIND['channel-open']: {
			const decoded = decodeJsonPayload('channel-open', SshEnvironmentChannelOpenSchema, payload);
			if (!decoded.ok) return decoded;
			return { ok: true, message: { type: 'channel-open', payload: decoded.payload } };
		}
		case FRAME_KIND['channel-opened']: {
			const decoded = decodeJsonPayload(
				'channel-opened',
				SshEnvironmentChannelOpenedSchema,
				payload,
			);
			if (!decoded.ok) return decoded;
			return { ok: true, message: { type: 'channel-opened', payload: decoded.payload } };
		}
		case FRAME_KIND['channel-data']:
			return decodeChannelData(payload);
		case FRAME_KIND['channel-credit']:
			return decodeChannelCredit(payload);
		case FRAME_KIND['channel-close']: {
			const decoded = decodeJsonPayload('channel-close', SshEnvironmentChannelCloseSchema, payload);
			if (!decoded.ok) return decoded;
			return { ok: true, message: { type: 'channel-close', payload: decoded.payload } };
		}
		case FRAME_KIND['channel-closed']: {
			const decoded = decodeJsonPayload(
				'channel-closed',
				SshEnvironmentChannelClosedSchema,
				payload,
			);
			if (!decoded.ok) return decoded;
			return { ok: true, message: { type: 'channel-closed', payload: decoded.payload } };
		}
		default:
			return decodeError('kind-unsupported', `unsupported frame kind ${kind}`);
	}
}

export function decodeSshEnvironmentFrame(frame: Uint8Array): SshEnvironmentDecodeResult {
	if (frame.byteLength < SSH_ENVIRONMENT_FRAME_HEADER_BYTES) {
		return decodeError('frame-too-short', 'frame does not contain a complete header');
	}
	if (frame.byteLength > SSH_ENVIRONMENT_MAX_FRAME_BYTES) {
		return decodeError('frame-too-large', 'frame exceeds the maximum size');
	}

	for (let index = 0; index < SSH_ENVIRONMENT_FRAME_MAGIC.length; index += 1) {
		if (frame[index] !== SSH_ENVIRONMENT_FRAME_MAGIC.charCodeAt(index)) {
			return decodeError('magic-invalid', 'frame magic is not OBSE');
		}
	}
	if (
		frame[4] !== SSH_ENVIRONMENT_PROTOCOL_VERSION.major ||
		frame[5] !== SSH_ENVIRONMENT_PROTOCOL_VERSION.minor ||
		frame[6] !== SSH_ENVIRONMENT_PROTOCOL_VERSION.patch
	) {
		return decodeError('version-unsupported', 'frame protocol version is unsupported');
	}
	if (frame[12] !== 0 || frame[13] !== 0 || frame[14] !== 0 || frame[15] !== 0) {
		return decodeError('reserved-bytes-nonzero', 'reserved header bytes must be zero');
	}

	const payloadLength = new DataView(frame.buffer, frame.byteOffset, frame.byteLength).getUint32(
		8,
		false,
	);
	if (payloadLength > MAX_FRAME_PAYLOAD_BYTES) {
		return decodeError('frame-too-large', 'declared payload exceeds the maximum size');
	}
	if (frame.byteLength !== SSH_ENVIRONMENT_FRAME_HEADER_BYTES + payloadLength) {
		return decodeError('frame-length-mismatch', 'declared payload length does not match frame');
	}

	return decodePayload(frame[7] ?? 0, frame.subarray(SSH_ENVIRONMENT_FRAME_HEADER_BYTES));
}
