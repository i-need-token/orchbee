import type { z } from 'zod';
import { BinaryReader } from '../../binary';
import { IDENTIFIER_BYTE_LENGTH, type AttachmentId } from '../../identifiers';
import type { CanonicalUint64Text } from '../../control';
import { PROTOCOL_LIMITS } from '../../limits';
import {
	ProtocolErrorCode,
	protocolFailure,
	protocolSuccess,
	type ProtocolResult,
} from '../../protocol-result';
import { MouseInputSchema, MouseAction, type MouseInput, type FocusInput } from '../../input';
import {
	WRITE_INPUT_HEADER_BYTES,
	PASTE_AUTO_INPUT_HEADER_BYTES,
	MOUSE_INPUT_BYTES,
	FOCUS_INPUT_BYTES,
	PasteAutoInputSchema,
	decodeAttachmentId,
	decodeCapabilityRevision,
	decodeModifiers,
	validationFailure,
} from '../../input-fields';
type PasteAutoInput = z.infer<typeof PasteAutoInputSchema>;

interface DecodedWriteInput {
	readonly attachmentId: AttachmentId;
	readonly bytes: Uint8Array;
	readonly capabilityRevision: CanonicalUint64Text;
}

function mouseActionFromCode(code: number): ProtocolResult<MouseAction> {
	switch (code) {
		case 0:
			return protocolSuccess(MouseAction.press);
		case 1:
			return protocolSuccess(MouseAction.release);
		case 2:
			return protocolSuccess(MouseAction.motion);
		default:
			return protocolFailure(ProtocolErrorCode.invalidField, `Unknown mouse action ${code}`);
	}
}

export function decodeWriteInput(payload: Uint8Array): ProtocolResult<DecodedWriteInput> {
	if (payload.byteLength <= WRITE_INPUT_HEADER_BYTES) {
		return protocolFailure(
			ProtocolErrorCode.truncated,
			'WriteInput requires an AttachmentId and at least one input byte',
		);
	}
	if (payload.byteLength > PROTOCOL_LIMITS.maxWriteInputPayloadBytes) {
		return protocolFailure(
			ProtocolErrorCode.limitExceeded,
			`WriteInput payload exceeds ${PROTOCOL_LIMITS.maxWriteInputPayloadBytes} bytes`,
		);
	}

	const reader = new BinaryReader(payload);
	const attachmentId = decodeAttachmentId(reader.readBytes(IDENTIFIER_BYTE_LENGTH));
	if (!attachmentId.ok) return attachmentId;
	const capabilityRevision = decodeCapabilityRevision(reader);
	if (!capabilityRevision.ok) return capabilityRevision;
	return protocolSuccess({
		attachmentId: attachmentId.value,
		bytes: payload.subarray(WRITE_INPUT_HEADER_BYTES),
		capabilityRevision: capabilityRevision.value,
	});
}

export function decodePasteAutoInput(payload: Uint8Array): ProtocolResult<PasteAutoInput> {
	if (payload.byteLength < PASTE_AUTO_INPUT_HEADER_BYTES) {
		return protocolFailure(ProtocolErrorCode.truncated, 'PasteAutoInput header is truncated');
	}
	if (payload.byteLength > PASTE_AUTO_INPUT_HEADER_BYTES + PROTOCOL_LIMITS.maxPasteAutoInputBytes) {
		return protocolFailure(
			ProtocolErrorCode.limitExceeded,
			'PasteAutoInput payload exceeds its limit',
		);
	}
	const reader = new BinaryReader(payload);
	const attachmentId = decodeAttachmentId(reader.readBytes(IDENTIFIER_BYTE_LENGTH));
	if (!attachmentId.ok) return attachmentId;
	const capabilityRevision = decodeCapabilityRevision(reader);
	if (!capabilityRevision.ok) return capabilityRevision;
	const flags = reader.readUint8();
	const reservedByte = reader.readUint8();
	const reservedWord = reader.readUint16();
	const contentLength = reader.readUint32();
	if (reader.failure) return reader.failure;
	if (flags !== 0 || reservedByte !== 0 || reservedWord !== 0) {
		return protocolFailure(
			ProtocolErrorCode.unknownRequiredFlag,
			'PasteAutoInput has unknown flags or reserved bytes',
		);
	}
	if (contentLength !== payload.byteLength - PASTE_AUTO_INPUT_HEADER_BYTES) {
		return protocolFailure(
			ProtocolErrorCode.lengthMismatch,
			'PasteAutoInput length does not match payload',
		);
	}
	const content = reader.readBytes(contentLength);
	if (reader.failure) return reader.failure;
	const parsed = PasteAutoInputSchema.safeParse({
		attachmentId: attachmentId.value,
		capabilityRevision: capabilityRevision.value,
		content,
	});
	if (!parsed.success) return validationFailure('PasteAutoInput', parsed.error.issues);
	return protocolSuccess(parsed.data);
}

export function decodeMouseInput(payload: Uint8Array): ProtocolResult<MouseInput> {
	if (payload.byteLength !== MOUSE_INPUT_BYTES) {
		return protocolFailure(
			payload.byteLength < MOUSE_INPUT_BYTES
				? ProtocolErrorCode.truncated
				: ProtocolErrorCode.lengthMismatch,
			`MouseInput must be exactly ${MOUSE_INPUT_BYTES} bytes`,
		);
	}
	const reader = new BinaryReader(payload);
	const attachmentId = decodeAttachmentId(reader.readBytes(IDENTIFIER_BYTE_LENGTH));
	if (!attachmentId.ok) return attachmentId;
	const capabilityRevision = decodeCapabilityRevision(reader);
	if (!capabilityRevision.ok) return capabilityRevision;
	const action = mouseActionFromCode(reader.readUint8());
	if (!action.ok) return action;
	const buttonCode = reader.readUint8();
	const modifiers = decodeModifiers(reader.readUint16());
	if (!modifiers.ok) return modifiers;
	const x = reader.readFloat32();
	const y = reader.readFloat32();
	const pressedButtons = reader.readUint16();
	const reserved = reader.readUint16();
	if (reader.failure) return reader.failure;
	if (reserved !== 0) {
		return protocolFailure(ProtocolErrorCode.unknownRequiredFlag, 'MouseInput has reserved bits');
	}
	const parsed = MouseInputSchema.safeParse({
		action: action.value,
		attachmentId: attachmentId.value,
		button: buttonCode === 0 ? null : buttonCode,
		capabilityRevision: capabilityRevision.value,
		modifiers: modifiers.value,
		pressedButtons,
		x,
		y,
	});
	if (!parsed.success) return validationFailure('MouseInput', parsed.error.issues);
	return protocolSuccess(parsed.data);
}

export function decodeFocusInput(payload: Uint8Array): ProtocolResult<FocusInput> {
	if (payload.byteLength !== FOCUS_INPUT_BYTES) {
		return protocolFailure(
			payload.byteLength < FOCUS_INPUT_BYTES
				? ProtocolErrorCode.truncated
				: ProtocolErrorCode.lengthMismatch,
			`FocusInput must be exactly ${FOCUS_INPUT_BYTES} bytes`,
		);
	}
	const reader = new BinaryReader(payload);
	const attachmentId = decodeAttachmentId(reader.readBytes(IDENTIFIER_BYTE_LENGTH));
	if (!attachmentId.ok) return attachmentId;
	const capabilityRevision = decodeCapabilityRevision(reader);
	if (!capabilityRevision.ok) return capabilityRevision;
	const focused = reader.readUint8();
	const reserved = reader.readBytes(3);
	if (reader.failure) return reader.failure;
	if (
		(focused !== 0 && focused !== 1) ||
		reserved[0] !== 0 ||
		reserved[1] !== 0 ||
		reserved[2] !== 0
	) {
		return protocolFailure(ProtocolErrorCode.unknownRequiredFlag, 'FocusInput has invalid flags');
	}
	return protocolSuccess({
		attachmentId: attachmentId.value,
		capabilityRevision: capabilityRevision.value,
		focused: focused === 1,
	});
}
