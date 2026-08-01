import { describe, expect, it } from 'vitest';

import {
	decodeFocusInput,
	decodeKeyInput,
	decodeMouseInput,
	decodePasteInput,
	decodeTextInput,
	decodeWriteInput,
	encodeFocusInput,
	encodeKeyInput,
	encodeMouseInput,
	encodePasteInput,
	encodeTextInput,
	encodeWriteInput,
	FocusInputSchema,
	KeyAction,
	MouseAction,
	MouseButton,
	WRITE_INPUT_ATTACHMENT_BYTES,
} from '../input';
import { PROTOCOL_LIMITS } from '../limits';
import { ProtocolErrorCode } from '../protocol-result';

const attachmentId = '22222222222222222222222222222222';

const noModifiers = {
	alt: false,
	altRight: false,
	capsLock: false,
	control: false,
	controlRight: false,
	meta: false,
	metaRight: false,
	numLock: false,
	shift: false,
	shiftRight: false,
};

describe('WriteInput binary payload', () => {
	it('round-trips an attachment-scoped byte sequence without copying the decoded input', () => {
		const input = new Uint8Array([0, 0x1b, 0xf0, 0x9f, 0x90, 0x9d]);
		const encoded = encodeWriteInput({ attachmentId, bytes: input });
		if (!encoded.ok) throw new Error(encoded.detail);
		expect(encoded.value).toHaveLength(WRITE_INPUT_ATTACHMENT_BYTES + input.byteLength);

		const decoded = decodeWriteInput(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		expect(decoded.value).toEqual({ attachmentId, bytes: input });
		expect(decoded.value.bytes.buffer).toBe(encoded.value.buffer);
	});

	it('rejects missing, zero, and oversized attachment-scoped input', () => {
		expect(decodeWriteInput(new Uint8Array(WRITE_INPUT_ATTACHMENT_BYTES))).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.truncated, ok: false }),
		);
		expect(decodeWriteInput(new Uint8Array(WRITE_INPUT_ATTACHMENT_BYTES + 1))).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidIdentifier, ok: false }),
		);
		expect(decodeWriteInput(new Uint8Array(PROTOCOL_LIMITS.maxWriteInputPayloadBytes + 1))).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.limitExceeded, ok: false }),
		);
	});

	it('keeps the negotiated terminal byte limit separate from the attachment prefix', () => {
		const encoded = encodeWriteInput({
			attachmentId,
			bytes: new Uint8Array(PROTOCOL_LIMITS.maxInputBytes),
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		expect(encoded.value.byteLength).toBe(PROTOCOL_LIMITS.maxWriteInputPayloadBytes);
	});
});

describe('semantic terminal input payloads', () => {
	it('round-trips a protocol-owned key identity and its layout text', () => {
		const input = {
			action: KeyAction.press,
			attachmentId,
			composing: false,
			consumedModifiers: { ...noModifiers, shift: true },
			key: 'KeyA',
			modifiers: { ...noModifiers, shift: true, shiftRight: true },
			text: 'A',
			unshiftedCodepoint: 'a'.codePointAt(0) ?? 0,
		};
		const encoded = encodeKeyInput(input);
		if (!encoded.ok) throw new Error(encoded.detail);
		expect(decodeKeyInput(encoded.value)).toEqual({ ok: true, value: input });
	});

	it('keeps committed text separate from approved paste intent', () => {
		const committed = { attachmentId, text: '中文' };
		const text = encodeTextInput(committed);
		const paste = encodePasteInput({ attachmentId, text: 'line 1\nline 2' });
		if (!text.ok) throw new Error(text.detail);
		if (!paste.ok) throw new Error(paste.detail);
		expect(decodeTextInput(text.value)).toEqual({ ok: true, value: committed });
		expect(decodePasteInput(paste.value)).toEqual({
			ok: true,
			value: { attachmentId, text: 'line 1\nline 2' },
		});
	});

	it('round-trips finite terminal-space mouse coordinates and pressed-button state', () => {
		const input = {
			action: MouseAction.motion,
			attachmentId,
			button: MouseButton.left,
			modifiers: { ...noModifiers, alt: true },
			pressedButtons: 1,
			x: 10.5,
			y: -2.25,
		};
		const encoded = encodeMouseInput(input);
		if (!encoded.ok) throw new Error(encoded.detail);
		expect(decodeMouseInput(encoded.value)).toEqual({ ok: true, value: input });
	});

	it('rejects mouse coordinates that cannot survive float32 encoding', () => {
		const encoded = encodeMouseInput({
			action: MouseAction.motion,
			attachmentId,
			button: null,
			modifiers: noModifiers,
			pressedButtons: 0,
			x: Number.MAX_VALUE,
			y: 0,
		});
		expect(encoded.ok).toBe(false);
	});

	it('round-trips focus and rejects reserved focus flags', () => {
		const encoded = encodeFocusInput({ attachmentId, focused: true });
		if (!encoded.ok) throw new Error(encoded.detail);
		expect(decodeFocusInput(encoded.value)).toEqual({
			ok: true,
			value: { attachmentId, focused: true },
		});
		const malformed = encoded.value.slice();
		malformed[17] = 1;
		expect(decodeFocusInput(malformed)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unknownRequiredFlag, ok: false }),
		);
		expect(FocusInputSchema.safeParse({ attachmentId, focused: true }).success).toBe(true);
	});
});
