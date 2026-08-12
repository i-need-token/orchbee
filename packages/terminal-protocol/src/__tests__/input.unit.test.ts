import { describe, expect, it } from 'vitest';

import {
	decodeFocusInput,
	decodeKeyInput,
	decodeMouseInput,
	decodePasteInput,
	decodeTextInput,
	decodeWriteInput,
	encodeInputRequest,
	encodeFocusInput,
	encodeKeyInput,
	encodeMouseInput,
	encodePasteInput,
	encodeTextInput,
	encodeWriteInput,
	FocusInputSchema,
	KEY_INPUT_HEADER_BYTES,
	KeyAction,
	MouseAction,
	MouseButton,
	WRITE_INPUT_ATTACHMENT_BYTES,
} from '../input';
import { MessageKind } from '../envelope';
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

describe('input request encoding', () => {
	it('selects the binary codec from the message kind', () => {
		expect(encodeInputRequest(MessageKind.focusInput, { attachmentId, focused: true })).toEqual(
			expect.objectContaining({ ok: true }),
		);
		expect(encodeInputRequest(MessageKind.keyInput, { attachmentId, focused: true })).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
		expect(
			encodeInputRequest(MessageKind.writeInput, {
				attachmentId,
				bytes: new Uint8Array([0x61]),
			}),
		).toEqual(expect.objectContaining({ ok: true }));
	});

	it('rejects non-input message kinds', () => {
		expect(encodeInputRequest(MessageKind.inputAccepted, {})).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});
});

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

	it('rejects non-canonical and unconsumed modifier combinations', () => {
		const base = {
			action: KeyAction.press,
			attachmentId,
			composing: false,
			consumedModifiers: noModifiers,
			key: 'KeyA',
			modifiers: noModifiers,
			text: 'a',
			unshiftedCodepoint: 'a'.codePointAt(0) ?? 0,
		};
		expect(
			encodeKeyInput({
				...base,
				modifiers: { ...noModifiers, shiftRight: true },
			}),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }));
		expect(
			encodeKeyInput({
				...base,
				consumedModifiers: { ...noModifiers, control: true },
			}),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }));

		const encoded = encodeKeyInput(base);
		if (!encoded.ok) throw new Error(encoded.detail);
		const nonCanonical = encoded.value.slice();
		new DataView(nonCanonical.buffer).setUint16(18, 1 << 6, true);
		expect(decodeKeyInput(nonCanonical)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});

	it('classifies malformed key UTF-8 independently from an unknown key', () => {
		const encoded = encodeKeyInput({
			action: KeyAction.press,
			attachmentId,
			composing: false,
			consumedModifiers: noModifiers,
			key: 'KeyA',
			modifiers: noModifiers,
			text: '',
			unshiftedCodepoint: 'a'.codePointAt(0) ?? 0,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const malformed = encoded.value.slice();
		malformed[32] = 0xff;
		expect(decodeKeyInput(malformed)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidUtf8, ok: false }),
		);
	});

	it('rejects a BOM-prefixed key instead of canonicalizing it to F1', () => {
		const encoded = encodeKeyInput({
			action: KeyAction.press,
			attachmentId,
			composing: false,
			consumedModifiers: noModifiers,
			key: 'F1',
			modifiers: noModifiers,
			text: '',
			unshiftedCodepoint: 0,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const payload = new Uint8Array(encoded.value.byteLength + 3);
		payload.set(encoded.value.subarray(0, KEY_INPUT_HEADER_BYTES));
		new DataView(payload.buffer).setUint16(22, 5, true);
		payload.set([0xef, 0xbb, 0xbf, 0x46, 0x31], KEY_INPUT_HEADER_BYTES);

		expect(payload.subarray(KEY_INPUT_HEADER_BYTES)).toEqual(
			new Uint8Array([0xef, 0xbb, 0xbf, 0x46, 0x31]),
		);
		expect(decodeKeyInput(payload)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
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
		expect(encodeTextInput({ attachmentId, text: '\ud800' })).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
		expect(
			encodeKeyInput({
				action: KeyAction.press,
				attachmentId,
				composing: false,
				consumedModifiers: noModifiers,
				key: 'KeyA',
				modifiers: noModifiers,
				text: '\ud800',
				unshiftedCodepoint: 0,
			}),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }));
	});

	it('round-trips an exact leading U+FEFF through text and paste payloads', () => {
		const value = { attachmentId, text: '\ufeff' };
		const text = encodeTextInput(value);
		const paste = encodePasteInput(value);
		if (!text.ok) throw new Error(text.detail);
		if (!paste.ok) throw new Error(paste.detail);

		expect(text.value.subarray(-3)).toEqual(new Uint8Array([0xef, 0xbb, 0xbf]));
		expect(paste.value.subarray(-3)).toEqual(new Uint8Array([0xef, 0xbb, 0xbf]));
		expect(decodeTextInput(text.value)).toEqual({ ok: true, value });
		expect(decodePasteInput(paste.value)).toEqual({ ok: true, value });
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

	it('round-trips the exact finite float32 coordinate boundary', () => {
		const maximumFloat32 = 3.4028234663852886e38;
		const input = {
			action: MouseAction.release,
			attachmentId,
			button: MouseButton.left,
			modifiers: noModifiers,
			pressedButtons: 0,
			x: maximumFloat32,
			y: -maximumFloat32,
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

	it('requires a concrete button for mouse press and release', () => {
		const press = encodeMouseInput({
			action: MouseAction.press,
			attachmentId,
			button: null,
			modifiers: noModifiers,
			pressedButtons: 0,
			x: 0,
			y: 0,
		});
		expect(press).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);

		const motion = encodeMouseInput({
			action: MouseAction.motion,
			attachmentId,
			button: null,
			modifiers: noModifiers,
			pressedButtons: 0,
			x: 0,
			y: 0,
		});
		if (!motion.ok) throw new Error(motion.detail);
		motion.value[16] = 0;
		expect(decodeMouseInput(motion.value)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});

	it('matches mouse motion button identity while keeping wheel presses stateless', () => {
		for (const input of [
			{
				action: MouseAction.motion,
				attachmentId,
				button: null,
				modifiers: noModifiers,
				pressedButtons: 1,
				x: 0,
				y: 0,
			},
			{
				action: MouseAction.motion,
				attachmentId,
				button: MouseButton.left,
				modifiers: noModifiers,
				pressedButtons: 0,
				x: 0,
				y: 0,
			},
			{
				action: MouseAction.motion,
				attachmentId,
				button: MouseButton.right,
				modifiers: noModifiers,
				pressedButtons: 1,
				x: 0,
				y: 0,
			},
		]) {
			expect(encodeMouseInput(input).ok).toBe(false);
		}

		const wheel = encodeMouseInput({
			action: MouseAction.press,
			attachmentId,
			button: MouseButton.four,
			modifiers: noModifiers,
			pressedButtons: 0,
			x: 0,
			y: 0,
		});
		expect(wheel.ok).toBe(true);
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

describe('semantic-input rejection parity with the native mirror', () => {
	const fPlusOneKeyPayload = new Uint8Array([
		0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
		0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
		0x46, 0x2b, 0x31,
	]);

	it('rejects the non-contract F+1 key code as invalid-field', () => {
		expect(decodeKeyInput(fPlusOneKeyPayload)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});

	it('reports length-mismatch for a zero-declared text length with a body', () => {
		const encoded = encodeTextInput({ attachmentId, text: 'abc' });
		if (!encoded.ok) throw new Error(encoded.detail);
		const zeroLength = encoded.value.slice();
		zeroLength[16] = 0;
		zeroLength[17] = 0;
		zeroLength[18] = 0;
		zeroLength[19] = 0;
		expect(decodeTextInput(zeroLength)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.lengthMismatch, ok: false }),
		);
		expect(decodePasteInput(zeroLength)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.lengthMismatch, ok: false }),
		);
	});

	it('rejects a header-only text payload as invalid-field', () => {
		const headerOnly = new Uint8Array(20);
		headerOnly.set(new Uint8Array(Array.from({ length: 16 }, () => 0x22)));
		expect(decodeTextInput(headerOnly)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});

	it('classifies pressed-buttons overflow as invalid-field', () => {
		const encoded = encodeMouseInput({
			action: MouseAction.motion,
			attachmentId,
			button: MouseButton.left,
			modifiers: { ...noModifiers, alt: true },
			pressedButtons: 1,
			x: 10.5,
			y: -2.25,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const bit11 = encoded.value.slice();
		bit11[29] = (bit11[29] ?? 0) | 0x08; // pressed-buttons bit 11 (0x0800)
		expect(decodeMouseInput(bit11)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});

	it('retains unknown-required-flag for reserved and unknown modifier bits', () => {
		const encoded = encodeMouseInput({
			action: MouseAction.motion,
			attachmentId,
			button: MouseButton.left,
			modifiers: { ...noModifiers, alt: true },
			pressedButtons: 1,
			x: 10.5,
			y: -2.25,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const reserved = encoded.value.slice();
		reserved[30] = 1;
		expect(decodeMouseInput(reserved)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unknownRequiredFlag, ok: false }),
		);
		const unknownModifier = encoded.value.slice();
		unknownModifier[18] = 0x00;
		unknownModifier[19] = 0x10; // modifier bit 12
		expect(decodeMouseInput(unknownModifier)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unknownRequiredFlag, ok: false }),
		);
	});
});
