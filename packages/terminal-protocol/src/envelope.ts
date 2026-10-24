import { z } from 'zod';

import { BinaryReader, BinaryWriter, checkedAdd } from './binary';
import {
	CorrelationIdSchema,
	HostEpochSchema,
	IDENTIFIER_BYTE_LENGTH,
	identifierBytes,
	identifierText,
	parseCorrelationId,
	parseHostEpoch,
} from './identifiers';
import type { CorrelationId, HostEpoch } from './identifiers';
import { PROTOCOL_LIMITS } from './limits';
import { isKnownMessageKind, messagePayloadEncoding, PayloadEncoding } from './message-contract';
import type { MessageRole, PayloadEncoding as PayloadEncodingValue } from './message-contract';
import { ProtocolErrorCode, protocolFailure, protocolSuccess } from './protocol-result';
import type { ProtocolResult } from './protocol-result';
import { ProtocolVersionSchema } from './version';
import type { ProtocolVersion } from './version';

export const TRANSPORT_ENVELOPE_HEADER_BYTES = 64;
export const TRANSPORT_ENVELOPE_MAGIC = new Uint8Array([0x4f, 0x42, 0x54, 0x50]);

export {
	isControlRequestKind,
	isInputRequestKind,
	messagePayloadEncoding,
	MessageKind,
	PayloadEncoding,
	requestMessageContract,
} from './message-contract';
export type { MessageRole } from './message-contract';

const payloadEncodingSchema = z.enum(PayloadEncoding);
const uint16Schema = z.number().int().min(0).max(65_535);

const ProtocolEnvelopeSchema = z
	.strictObject({
		correlationId: CorrelationIdSchema.nullable(),
		event: z.boolean(),
		hostEpoch: HostEpochSchema.nullable(),
		kind: uint16Schema,
		optionalMessage: z.boolean(),
		payload: z.instanceof(Uint8Array),
		payloadEncoding: payloadEncodingSchema,
		protocol: ProtocolVersionSchema,
		response: z.boolean(),
	})
	.superRefine((value, context) => {
		if (value.event && value.response) {
			context.addIssue({
				code: 'custom',
				message: 'An envelope cannot be both an event and a response',
				path: ['event'],
			});
		}
		if (!isKnownMessageKind(value.kind) && !value.optionalMessage) {
			context.addIssue({
				code: 'custom',
				message: 'An unknown message kind must be marked optional',
				path: ['kind'],
			});
		}
		if (!(value.event && value.response)) {
			let role: MessageRole = 'request';
			if (value.event) role = 'event';
			else if (value.response) role = 'response';
			const expectedEncoding = messagePayloadEncoding(value.kind, role);
			if (isKnownMessageKind(value.kind) && expectedEncoding === null) {
				context.addIssue({
					code: 'custom',
					message: `Message kind ${value.kind} cannot be used as a ${role}`,
					path: ['kind'],
				});
			} else if (expectedEncoding !== null && value.payloadEncoding !== expectedEncoding) {
				context.addIssue({
					code: 'custom',
					message: `Message kind ${value.kind} requires ${expectedEncoding} for ${role}`,
					path: ['payloadEncoding'],
				});
			}
		}
		const maximum = maximumPayloadBytes(value.payloadEncoding);
		if (value.payload.byteLength > maximum) {
			context.addIssue({
				code: 'custom',
				message: `${value.payloadEncoding} payload exceeds ${maximum} bytes`,
				path: ['payload'],
			});
		}
	});

export interface DecodedProtocolEnvelope {
	readonly correlationId: CorrelationId | null;
	readonly event: boolean;
	readonly hostEpoch: HostEpoch | null;
	readonly ignoredOptionalFlags: number;
	readonly kind: number;
	readonly knownMessage: boolean;
	readonly optionalMessage: boolean;
	readonly payload: Uint8Array;
	readonly payloadEncoding: PayloadEncodingValue;
	readonly protocol: ProtocolVersion;
	readonly response: boolean;
}

const EnvelopeFlag = {
	event: 1 << 2,
	knownMask: 0b111,
	optionalMessage: 1,
	optionalPartitionMask: 0x00ff,
	response: 1 << 1,
	requiredPartitionMask: 0xff00,
} as const;

function maximumPayloadBytes(encoding: PayloadEncodingValue): number {
	switch (encoding) {
		case PayloadEncoding.graphicsImage:
			return PROTOCOL_LIMITS.maxGraphicsImageBytes;
		case PayloadEncoding.controlJson:
			return PROTOCOL_LIMITS.maxControlJsonBytes;
		case PayloadEncoding.rawBytes:
			return PROTOCOL_LIMITS.maxRawPayloadBytes;
		case PayloadEncoding.rowWindow:
			return PROTOCOL_LIMITS.maxRowWindowBytes;
		case PayloadEncoding.terminalFrame:
			return PROTOCOL_LIMITS.maxFrameBytes;
	}
}

function payloadEncodingCode(encoding: PayloadEncodingValue): number {
	switch (encoding) {
		case PayloadEncoding.graphicsImage:
			return 5;
		case PayloadEncoding.controlJson:
			return 1;
		case PayloadEncoding.rawBytes:
			return 2;
		case PayloadEncoding.rowWindow:
			return 4;
		case PayloadEncoding.terminalFrame:
			return 3;
	}
}

function payloadEncodingFromCode(code: number): ProtocolResult<PayloadEncodingValue> {
	switch (code) {
		case 5:
			return protocolSuccess(PayloadEncoding.graphicsImage);
		case 1:
			return protocolSuccess(PayloadEncoding.controlJson);
		case 2:
			return protocolSuccess(PayloadEncoding.rawBytes);
		case 3:
			return protocolSuccess(PayloadEncoding.terminalFrame);
		case 4:
			return protocolSuccess(PayloadEncoding.rowWindow);
		default:
			return protocolFailure(ProtocolErrorCode.invalidField, `Unknown payload encoding ${code}`);
	}
}

function isMagic(bytes: Uint8Array): boolean {
	return TRANSPORT_ENVELOPE_MAGIC.every((value, index) => bytes[index] === value);
}

function isZeroIdentifier(bytes: Uint8Array): boolean {
	for (const byte of bytes) if (byte !== 0) return false;
	return true;
}

function decodeOptionalCorrelationId(bytes: Uint8Array): ProtocolResult<CorrelationId | null> {
	if (isZeroIdentifier(bytes)) return protocolSuccess(null);
	const text = identifierText(bytes);
	if (!text.ok) return text;
	return parseCorrelationId(text.value);
}

function decodeOptionalHostEpoch(bytes: Uint8Array): ProtocolResult<HostEpoch | null> {
	if (isZeroIdentifier(bytes)) return protocolSuccess(null);
	const text = identifierText(bytes);
	if (!text.ok) return text;
	return parseHostEpoch(text.value);
}

export function encodeProtocolEnvelope(value: unknown): ProtocolResult<Uint8Array> {
	const parsed = ProtocolEnvelopeSchema.safeParse(value);
	if (!parsed.success) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			`Envelope validation failed: ${parsed.error.issues[0]?.message ?? 'unknown field error'}`,
		);
	}

	const totalLength = checkedAdd(TRANSPORT_ENVELOPE_HEADER_BYTES, parsed.data.payload.byteLength);
	if (!totalLength.ok) return totalLength;
	if (totalLength.value > PROTOCOL_LIMITS.maxTransportRecordBytes) {
		return protocolFailure(
			ProtocolErrorCode.limitExceeded,
			`Transport record exceeds ${PROTOCOL_LIMITS.maxTransportRecordBytes} bytes`,
		);
	}

	let flags = 0;
	if (parsed.data.optionalMessage) flags |= EnvelopeFlag.optionalMessage;
	if (parsed.data.response) flags |= EnvelopeFlag.response;
	if (parsed.data.event) flags |= EnvelopeFlag.event;

	const writer = new BinaryWriter(totalLength.value);
	writer.writeBytes(TRANSPORT_ENVELOPE_MAGIC);
	writer.writeUint16(TRANSPORT_ENVELOPE_HEADER_BYTES);
	writer.writeUint16(flags);
	writer.writeUint32(totalLength.value);
	writer.writeUint32(parsed.data.payload.byteLength);
	writer.writeUint16(parsed.data.protocol.major);
	writer.writeUint16(parsed.data.protocol.minor);
	writer.writeUint16(parsed.data.kind);
	writer.writeUint8(payloadEncodingCode(parsed.data.payloadEncoding));
	writer.writeUint8(0);
	writer.writeBytes(
		parsed.data.correlationId
			? identifierBytes(parsed.data.correlationId)
			: new Uint8Array(IDENTIFIER_BYTE_LENGTH),
	);
	writer.writeBytes(
		parsed.data.hostEpoch
			? identifierBytes(parsed.data.hostEpoch)
			: new Uint8Array(IDENTIFIER_BYTE_LENGTH),
	);
	writer.writeUint16(parsed.data.protocol.patch);
	writer.writeBytes(new Uint8Array(6));
	writer.writeBytes(parsed.data.payload);
	return protocolSuccess(writer.bytes());
}

export function decodeProtocolEnvelope(bytes: Uint8Array): ProtocolResult<DecodedProtocolEnvelope> {
	if (bytes.byteLength < TRANSPORT_ENVELOPE_HEADER_BYTES) {
		return protocolFailure(
			ProtocolErrorCode.truncated,
			`Transport envelope requires at least ${TRANSPORT_ENVELOPE_HEADER_BYTES} bytes`,
		);
	}
	if (bytes.byteLength > PROTOCOL_LIMITS.maxTransportRecordBytes) {
		return protocolFailure(
			ProtocolErrorCode.limitExceeded,
			`Transport record exceeds ${PROTOCOL_LIMITS.maxTransportRecordBytes} bytes`,
		);
	}
	if (!isMagic(bytes)) {
		return protocolFailure(ProtocolErrorCode.invalidMagic, 'Transport envelope magic is not OBTP');
	}

	const reader = new BinaryReader(bytes, TRANSPORT_ENVELOPE_MAGIC.byteLength);
	const headerLength = reader.readUint16();
	const flags = reader.readUint16();
	const totalLength = reader.readUint32();
	const payloadLength = reader.readUint32();
	const major = reader.readUint16();
	const minor = reader.readUint16();
	const kind = reader.readUint16();
	const encodingCode = reader.readUint8();
	const reservedByte = reader.readUint8();
	const correlationBytes = reader.readBytes(IDENTIFIER_BYTE_LENGTH);
	const hostEpochBytes = reader.readBytes(IDENTIFIER_BYTE_LENGTH);
	const patch = reader.readUint16();
	const reserved = reader.readBytes(6);
	if (reader.failure) return reader.failure;

	if (headerLength !== TRANSPORT_ENVELOPE_HEADER_BYTES) {
		return protocolFailure(
			ProtocolErrorCode.unsupportedVersion,
			`Unsupported transport envelope header length ${headerLength}`,
		);
	}
	if (totalLength !== bytes.byteLength || payloadLength !== bytes.byteLength - headerLength) {
		return protocolFailure(
			ProtocolErrorCode.lengthMismatch,
			'Transport envelope length fields do not match the received record',
		);
	}
	if (reservedByte !== 0 || reserved.some((value) => value !== 0)) {
		return protocolFailure(
			ProtocolErrorCode.unknownRequiredFlag,
			'Transport envelope reserved header bytes must be zero',
		);
	}
	if ((flags & EnvelopeFlag.requiredPartitionMask) !== 0) {
		return protocolFailure(
			ProtocolErrorCode.unknownRequiredFlag,
			'Transport envelope contains an unknown required flag',
		);
	}
	if ((flags & EnvelopeFlag.event) !== 0 && (flags & EnvelopeFlag.response) !== 0) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			'Transport envelope cannot be both an event and a response',
		);
	}

	const optionalMessage = (flags & EnvelopeFlag.optionalMessage) !== 0;
	const knownMessage = isKnownMessageKind(kind);
	if (!knownMessage && !optionalMessage) {
		return protocolFailure(
			ProtocolErrorCode.unknownRequiredMessage,
			`Unknown required message kind ${kind}`,
		);
	}

	const encoding = payloadEncodingFromCode(encodingCode);
	if (!encoding.ok) return encoding;
	if (payloadLength > maximumPayloadBytes(encoding.value)) {
		return protocolFailure(
			ProtocolErrorCode.limitExceeded,
			`${encoding.value} payload exceeds its negotiated hard limit`,
		);
	}

	const correlationId = decodeOptionalCorrelationId(correlationBytes);
	if (!correlationId.ok) return correlationId;
	const hostEpoch = decodeOptionalHostEpoch(hostEpochBytes);
	if (!hostEpoch.ok) return hostEpoch;

	return protocolSuccess({
		correlationId: correlationId.value,
		event: (flags & EnvelopeFlag.event) !== 0,
		hostEpoch: hostEpoch.value,
		ignoredOptionalFlags: flags & EnvelopeFlag.optionalPartitionMask & ~EnvelopeFlag.knownMask,
		kind,
		knownMessage,
		optionalMessage,
		payload: bytes.subarray(headerLength),
		payloadEncoding: encoding.value,
		protocol: { major, minor, patch },
		response: (flags & EnvelopeFlag.response) !== 0,
	});
}
