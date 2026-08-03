import { CanonicalUint64TextSchema, PROTOCOL_LIMITS } from '@orchbee/terminal-protocol';
import {
	ProjectedTerminalCommandSchema,
	ProjectedTerminalEventSchema,
	type ProjectedTerminalCommand,
	type ProjectedTerminalEvent,
} from '@orchbee/workbench-client-protocol';
import type { z } from 'zod';

import { GATEWAY_LIMITS } from './gateway-control-contract';
import type { GatewayDecodeResult } from './gateway-json-codec';

const HEADER_BYTES = 16;
const MAGIC = 0x4f_42_47_57;
const WIRE_VERSION = 1;
const MAX_METADATA_BYTES = 128;
const textDecoder = new TextDecoder('utf-8', { fatal: true });
const textEncoder = new TextEncoder();

const AttachmentFrameKind = {
	clientCommandJson: 1,
	serverEventJson: 2,
	serverFrame: 3,
	serverRows: 4,
	serverSelectionCopyContent: 5,
} as const;

type AttachmentFrameKind = (typeof AttachmentFrameKind)[keyof typeof AttachmentFrameKind];

interface DecodedWireFrame {
	readonly kind: number;
	readonly metadata: Uint8Array;
	readonly payload: Uint8Array;
}

export function encodeGatewayAttachmentClientFrame(command: ProjectedTerminalCommand): ArrayBuffer {
	const canonical = ProjectedTerminalCommandSchema.parse(command);
	return encodeWireFrame(
		AttachmentFrameKind.clientCommandJson,
		new Uint8Array(),
		encodeJson(canonical),
	);
}

export function decodeGatewayAttachmentClientFrame(
	frame: ArrayBuffer,
): GatewayDecodeResult<ProjectedTerminalCommand> {
	const decoded = decodeWireFrame(frame);
	if (!decoded.ok) return decoded;
	if (
		decoded.value.kind !== AttachmentFrameKind.clientCommandJson ||
		decoded.value.metadata.byteLength !== 0 ||
		decoded.value.payload.byteLength > GATEWAY_LIMITS.maxAttachmentJsonBytes
	) {
		return { detail: 'Gateway attachment client frame has an invalid role or size.', ok: false };
	}
	return decodeJson(decoded.value.payload, ProjectedTerminalCommandSchema);
}

export function encodeGatewayAttachmentServerFrame(event: ProjectedTerminalEvent): ArrayBuffer {
	const canonical = ProjectedTerminalEventSchema.parse(event);
	switch (canonical.kind) {
		case 'frame':
			return encodeWireFrame(
				AttachmentFrameKind.serverFrame,
				new Uint8Array(),
				new Uint8Array(canonical.frame),
			);
		case 'rows':
			return encodeWireFrame(
				AttachmentFrameKind.serverRows,
				new Uint8Array(),
				new Uint8Array(canonical.rows),
			);
		case 'selection-copy-content':
			return encodeWireFrame(
				AttachmentFrameKind.serverSelectionCopyContent,
				textEncoder.encode(canonical.requestSequence),
				new Uint8Array(canonical.content),
			);
		default:
			return encodeWireFrame(
				AttachmentFrameKind.serverEventJson,
				new Uint8Array(),
				encodeJson(canonical),
			);
	}
}

export function decodeGatewayAttachmentServerFrame(
	frame: ArrayBuffer,
): GatewayDecodeResult<ProjectedTerminalEvent> {
	const decoded = decodeWireFrame(frame);
	if (!decoded.ok) return decoded;
	const { kind, metadata, payload } = decoded.value;
	switch (kind) {
		case AttachmentFrameKind.serverEventJson:
			if (metadata.byteLength !== 0 || payload.byteLength > GATEWAY_LIMITS.maxAttachmentJsonBytes) {
				return { detail: 'Gateway attachment JSON event has invalid metadata or size.', ok: false };
			}
			return decodeJson(payload, ProjectedTerminalEventSchema);
		case AttachmentFrameKind.serverFrame:
			if (metadata.byteLength !== 0 || payload.byteLength > PROTOCOL_LIMITS.maxFrameBytes) {
				return { detail: 'Gateway terminal frame has invalid metadata or size.', ok: false };
			}
			return parseEvent({ frame: copyArrayBuffer(payload), kind: 'frame' });
		case AttachmentFrameKind.serverRows:
			if (metadata.byteLength !== 0 || payload.byteLength > PROTOCOL_LIMITS.maxRowWindowBytes) {
				return { detail: 'Gateway row window has invalid metadata or size.', ok: false };
			}
			return parseEvent({ kind: 'rows', rows: copyArrayBuffer(payload) });
		case AttachmentFrameKind.serverSelectionCopyContent: {
			if (
				metadata.byteLength === 0 ||
				metadata.byteLength > MAX_METADATA_BYTES ||
				payload.byteLength > PROTOCOL_LIMITS.maxSelectionExportBytes
			) {
				return { detail: 'Gateway selection content has invalid metadata or size.', ok: false };
			}
			let requestSequence: string;
			try {
				requestSequence = textDecoder.decode(metadata);
			} catch {
				return { detail: 'Gateway selection metadata is not valid UTF-8.', ok: false };
			}
			const sequence = CanonicalUint64TextSchema.safeParse(requestSequence);
			if (!sequence.success) {
				return { detail: 'Gateway selection metadata has an invalid sequence.', ok: false };
			}
			return parseEvent({
				content: copyArrayBuffer(payload),
				kind: 'selection-copy-content',
				requestSequence: sequence.data,
			});
		}
		default:
			return { detail: 'Gateway attachment server frame has an invalid role.', ok: false };
	}
}

function decodeWireFrame(frame: ArrayBuffer): GatewayDecodeResult<DecodedWireFrame> {
	if (frame.byteLength < HEADER_BYTES) {
		return { detail: 'Gateway attachment frame is truncated.', ok: false };
	}
	const view = new DataView(frame);
	if (
		view.getUint32(0) !== MAGIC ||
		view.getUint8(4) !== WIRE_VERSION ||
		view.getUint8(6) !== 0 ||
		view.getUint8(7) !== 0
	) {
		return { detail: 'Gateway attachment frame header is invalid.', ok: false };
	}
	const metadataLength = view.getUint32(8);
	const payloadLength = view.getUint32(12);
	if (
		metadataLength > MAX_METADATA_BYTES ||
		HEADER_BYTES + metadataLength + payloadLength !== frame.byteLength
	) {
		return { detail: 'Gateway attachment frame lengths are invalid.', ok: false };
	}
	const bytes = new Uint8Array(frame);
	return {
		ok: true,
		value: {
			kind: view.getUint8(5),
			metadata: bytes.subarray(HEADER_BYTES, HEADER_BYTES + metadataLength),
			payload: bytes.subarray(HEADER_BYTES + metadataLength),
		},
	};
}

function encodeWireFrame(
	kind: AttachmentFrameKind,
	metadata: Uint8Array,
	payload: Uint8Array,
): ArrayBuffer {
	if (metadata.byteLength > MAX_METADATA_BYTES) {
		throw new Error('Gateway attachment metadata exceeds its limit.');
	}
	const frame = new ArrayBuffer(HEADER_BYTES + metadata.byteLength + payload.byteLength);
	const view = new DataView(frame);
	view.setUint32(0, MAGIC);
	view.setUint8(4, WIRE_VERSION);
	view.setUint8(5, kind);
	view.setUint8(6, 0);
	view.setUint8(7, 0);
	view.setUint32(8, metadata.byteLength);
	view.setUint32(12, payload.byteLength);
	const bytes = new Uint8Array(frame);
	bytes.set(metadata, HEADER_BYTES);
	bytes.set(payload, HEADER_BYTES + metadata.byteLength);
	return frame;
}

function encodeJson(value: unknown): Uint8Array {
	const encoded = textEncoder.encode(JSON.stringify(value));
	if (encoded.byteLength > GATEWAY_LIMITS.maxAttachmentJsonBytes) {
		throw new Error('Gateway attachment JSON exceeds its limit.');
	}
	return encoded;
}

function decodeJson<Value>(
	payload: Uint8Array,
	schema: z.ZodType<Value>,
): GatewayDecodeResult<Value> {
	let text: string;
	try {
		text = textDecoder.decode(payload);
	} catch {
		return { detail: 'Gateway attachment JSON is not valid UTF-8.', ok: false };
	}
	let value: unknown;
	try {
		value = JSON.parse(text);
	} catch {
		return { detail: 'Gateway attachment JSON is malformed.', ok: false };
	}
	const parsed = schema.safeParse(value);
	if (!parsed.success) {
		return { detail: 'Gateway attachment JSON does not match its role.', ok: false };
	}
	return { ok: true, value: parsed.data };
}

function parseEvent(value: unknown): GatewayDecodeResult<ProjectedTerminalEvent> {
	const parsed = ProjectedTerminalEventSchema.safeParse(value);
	return parsed.success
		? { ok: true, value: parsed.data }
		: { detail: 'Gateway attachment event is invalid.', ok: false };
}

function copyArrayBuffer(bytes: Uint8Array): ArrayBuffer {
	const copy = new Uint8Array(bytes.byteLength);
	copy.set(bytes);
	return copy.buffer;
}
