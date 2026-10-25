import {
	decodeFocusInput,
	decodeMouseInput,
	decodePasteAutoInput,
	decodeWriteInput,
} from './conformance/input-decoders';
import { describe, expect, it } from 'vitest';

import {
	decodeKeyInput,
	decodePasteInput,
	decodeTextInput,
	encodeInputRequest,
	FocusInputSchema,
	KEY_INPUT_HEADER_BYTES,
	KeyAction,
	MouseAction,
	MouseButton,
} from '../input';
import { MessageKind } from '../envelope';
import { PROTOCOL_LIMITS } from '../limits';
import { ProtocolErrorCode } from '../protocol-result';

const attachmentId = '22222222222222222222222222222222';
const capabilityRevision = '7';

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
		expect(
			encodeInputRequest(MessageKind.focusInput, {
				attachmentId,
				capabilityRevision,
				focused: true,
			}),
		).toEqual(expect.objectContaining({ ok: true }));
		expect(encodeInputRequest(MessageKind.keyInput, { attachmentId, focused: true })).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
		expect(
			encodeInputRequest(MessageKind.writeInput, {
				attachmentId,
				bytes: new Uint8Array([0x61]),
				capabilityRevision,
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
		const encoded = encodeInputRequest(MessageKind.writeInput, {
			attachmentId,
			bytes: input,
			capabilityRevision,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		expect(encoded.value).toHaveLength(24 + input.byteLength);

		const decoded = decodeWriteInput(encoded.value);
		if (!decoded.ok) throw new Error(decoded.detail);
		expect(decoded.value).toEqual({ attachmentId, bytes: input, capabilityRevision });
		expect(new DataView(encoded.value.buffer).getBigUint64(16, true)).toBe(7n);
		expect(decoded.value.bytes.buffer).toBe(encoded.value.buffer);
	});

	it('rejects missing, zero, and oversized attachment-scoped input', () => {
		expect(decodeWriteInput(new Uint8Array(24))).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.truncated, ok: false }),
		);
		expect(decodeWriteInput(new Uint8Array(24 + 1))).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidIdentifier, ok: false }),
		);
		expect(decodeWriteInput(new Uint8Array(PROTOCOL_LIMITS.maxWriteInputPayloadBytes + 1))).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.limitExceeded, ok: false }),
		);
	});

	it('keeps the negotiated terminal byte limit separate from the attachment prefix', () => {
		const encoded = encodeInputRequest(MessageKind.writeInput, {
			attachmentId,
			bytes: new Uint8Array(PROTOCOL_LIMITS.maxInputBytes),
			capabilityRevision,
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
			capabilityRevision,
			composing: false,
			consumedModifiers: { ...noModifiers, shift: true },
			key: 'KeyA',
			modifiers: { ...noModifiers, shift: true, shiftRight: true },
			optionAsAlt: true,
			text: 'A',
			unshiftedCodepoint: 'a'.codePointAt(0) ?? 0,
		};
		const encoded = encodeInputRequest(MessageKind.keyInput, input);
		if (!encoded.ok) throw new Error(encoded.detail);
		expect(decodeKeyInput(encoded.value)).toEqual({ ok: true, value: input });
	});

	it('rejects non-canonical and unconsumed modifier combinations', () => {
		const base = {
			action: KeyAction.press,
			attachmentId,
			capabilityRevision,
			composing: false,
			consumedModifiers: noModifiers,
			key: 'KeyA',
			modifiers: noModifiers,
			optionAsAlt: false,
			text: 'a',
			unshiftedCodepoint: 'a'.codePointAt(0) ?? 0,
		};
		expect(
			encodeInputRequest(MessageKind.keyInput, {
				...base,
				modifiers: { ...noModifiers, shiftRight: true },
			}),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }));
		expect(
			encodeInputRequest(MessageKind.keyInput, {
				...base,
				consumedModifiers: { ...noModifiers, control: true },
			}),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }));

		const encoded = encodeInputRequest(MessageKind.keyInput, base);
		if (!encoded.ok) throw new Error(encoded.detail);
		const nonCanonical = encoded.value.slice();
		new DataView(nonCanonical.buffer).setUint16(26, 1 << 6, true);
		expect(decodeKeyInput(nonCanonical)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
		const unknownFlag = encoded.value.slice();
		unknownFlag[25] = 1 << 2;
		expect(decodeKeyInput(unknownFlag)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unknownRequiredFlag, ok: false }),
		);
	});

	it('classifies malformed key UTF-8 independently from an unknown key', () => {
		const encoded = encodeInputRequest(MessageKind.keyInput, {
			action: KeyAction.press,
			attachmentId,
			capabilityRevision,
			composing: false,
			consumedModifiers: noModifiers,
			key: 'KeyA',
			modifiers: noModifiers,
			optionAsAlt: false,
			text: '',
			unshiftedCodepoint: 'a'.codePointAt(0) ?? 0,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const malformed = encoded.value.slice();
		malformed[40] = 0xff;
		expect(decodeKeyInput(malformed)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidUtf8, ok: false }),
		);
	});

	it('rejects a BOM-prefixed key instead of canonicalizing it to F1', () => {
		const encoded = encodeInputRequest(MessageKind.keyInput, {
			action: KeyAction.press,
			attachmentId,
			capabilityRevision,
			composing: false,
			consumedModifiers: noModifiers,
			key: 'F1',
			modifiers: noModifiers,
			optionAsAlt: false,
			text: '',
			unshiftedCodepoint: 0,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const payload = new Uint8Array(encoded.value.byteLength + 3);
		payload.set(encoded.value.subarray(0, KEY_INPUT_HEADER_BYTES));
		new DataView(payload.buffer).setUint16(30, 5, true);
		payload.set([0xef, 0xbb, 0xbf, 0x46, 0x31], KEY_INPUT_HEADER_BYTES);

		expect(payload.subarray(KEY_INPUT_HEADER_BYTES)).toEqual(
			new Uint8Array([0xef, 0xbb, 0xbf, 0x46, 0x31]),
		);
		expect(decodeKeyInput(payload)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});

	it('keeps committed text separate from approved paste intent', () => {
		const committed = { attachmentId, capabilityRevision, text: '中文' };
		const text = encodeInputRequest(MessageKind.textInput, committed);
		const pasteValue = {
			attachmentId,
			bracketed: true,
			capabilityRevision,
			text: 'line 1\nline 2',
		};
		const paste = encodeInputRequest(MessageKind.pasteInput, pasteValue);
		if (!text.ok) throw new Error(text.detail);
		if (!paste.ok) throw new Error(paste.detail);
		expect(decodeTextInput(text.value)).toEqual({ ok: true, value: committed });
		expect(paste.value.subarray(24, 28)).toEqual(new Uint8Array([1, 0, 0, 0]));
		expect(decodePasteInput(paste.value)).toEqual({ ok: true, value: pasteValue });
		const unknownFlag = paste.value.slice();
		unknownFlag[24] = 2;
		expect(decodePasteInput(unknownFlag)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unknownRequiredFlag, ok: false }),
		);
		expect(
			encodeInputRequest(MessageKind.textInput, {
				attachmentId,
				capabilityRevision,
				text: '\ud800',
			}),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }));
		expect(
			encodeInputRequest(MessageKind.keyInput, {
				action: KeyAction.press,
				attachmentId,
				capabilityRevision,
				composing: false,
				consumedModifiers: noModifiers,
				key: 'KeyA',
				modifiers: noModifiers,
				optionAsAlt: false,
				text: '\ud800',
				unshiftedCodepoint: 0,
			}),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }));
	});

	it('round-trips an exact leading U+FEFF through text and paste payloads', () => {
		const value = { attachmentId, capabilityRevision, text: '\ufeff' };
		const pasteValue = { ...value, bracketed: false };
		const text = encodeInputRequest(MessageKind.textInput, value);
		const paste = encodeInputRequest(MessageKind.pasteInput, pasteValue);
		if (!text.ok) throw new Error(text.detail);
		if (!paste.ok) throw new Error(paste.detail);

		expect(text.value.subarray(-3)).toEqual(new Uint8Array([0xef, 0xbb, 0xbf]));
		expect(paste.value.subarray(-3)).toEqual(new Uint8Array([0xef, 0xbb, 0xbf]));
		expect(decodeTextInput(text.value)).toEqual({ ok: true, value });
		expect(decodePasteInput(paste.value)).toEqual({ ok: true, value: pasteValue });
	});

	it('round-trips opaque paste bytes, including control and invalid UTF-8 bytes', () => {
		const content = new Uint8Array([
			0x02, 0x80, 0xff, 0x1b, 0x0d, 0x0a, 0x72, 0x61, 0x77, 0x2d, 0x70, 0x61, 0x73, 0x74, 0x65,
		]);
		const value = { attachmentId, capabilityRevision, content };
		const encoded = encodeInputRequest(MessageKind.pasteAutoInput, value);
		if (!encoded.ok) throw new Error(encoded.detail);
		expect(encoded.value).toHaveLength(32 + content.byteLength);
		expect(encoded.value.subarray(32)).toEqual(content);
		expect(decodePasteAutoInput(encoded.value)).toEqual({ ok: true, value });

		const dispatched = encodeInputRequest(MessageKind.pasteAutoInput, value);
		if (!dispatched.ok) throw new Error(dispatched.detail);
		expect(dispatched.value).toEqual(encoded.value);
	});

	it('rejects paste-auto payloads that violate flags, length, or size bounds', () => {
		const value = {
			attachmentId,
			capabilityRevision,
			content: new Uint8Array([0x61]),
		};
		const encoded = encodeInputRequest(MessageKind.pasteAutoInput, value);
		if (!encoded.ok) throw new Error(encoded.detail);

		expect(decodePasteAutoInput(new Uint8Array(32 - 1))).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.truncated, ok: false }),
		);

		const flagged = encoded.value.slice();
		flagged[24] = 1;
		expect(decodePasteAutoInput(flagged)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unknownRequiredFlag, ok: false }),
		);
		const reserved = encoded.value.slice();
		reserved[27] = 1;
		expect(decodePasteAutoInput(reserved)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unknownRequiredFlag, ok: false }),
		);

		const zeroLength = encoded.value.slice();
		new DataView(zeroLength.buffer).setUint32(28, 0, true);
		expect(decodePasteAutoInput(zeroLength)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.lengthMismatch, ok: false }),
		);

		expect(
			encodeInputRequest(MessageKind.pasteAutoInput, { ...value, content: new Uint8Array(0) }),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }));
		expect(
			encodeInputRequest(MessageKind.pasteAutoInput, {
				...value,
				content: new Uint8Array(PROTOCOL_LIMITS.maxPasteAutoInputBytes + 1),
			}),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }));
		expect(
			decodePasteAutoInput(new Uint8Array(32 + PROTOCOL_LIMITS.maxPasteAutoInputBytes + 1)),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.limitExceeded, ok: false }));
	});

	it('round-trips finite terminal-space mouse coordinates and pressed-button state', () => {
		const input = {
			action: MouseAction.motion,
			attachmentId,
			capabilityRevision,
			button: MouseButton.left,
			modifiers: { ...noModifiers, alt: true },
			pressedButtons: 1,
			x: 10.5,
			y: -2.25,
		};
		const encoded = encodeInputRequest(MessageKind.mouseInput, input);
		if (!encoded.ok) throw new Error(encoded.detail);
		expect(decodeMouseInput(encoded.value)).toEqual({ ok: true, value: input });
	});

	it('round-trips the exact finite float32 coordinate boundary', () => {
		const maximumFloat32 = 3.4028234663852886e38;
		const input = {
			action: MouseAction.release,
			attachmentId,
			capabilityRevision,
			button: MouseButton.left,
			modifiers: noModifiers,
			pressedButtons: 0,
			x: maximumFloat32,
			y: -maximumFloat32,
		};
		const encoded = encodeInputRequest(MessageKind.mouseInput, input);
		if (!encoded.ok) throw new Error(encoded.detail);
		expect(decodeMouseInput(encoded.value)).toEqual({ ok: true, value: input });
	});

	it('rejects mouse coordinates that cannot survive float32 encoding', () => {
		const encoded = encodeInputRequest(MessageKind.mouseInput, {
			action: MouseAction.motion,
			attachmentId,
			capabilityRevision,
			button: null,
			modifiers: noModifiers,
			pressedButtons: 0,
			x: Number.MAX_VALUE,
			y: 0,
		});
		expect(encoded.ok).toBe(false);
	});

	it('requires a concrete button for mouse press and release', () => {
		const press = encodeInputRequest(MessageKind.mouseInput, {
			action: MouseAction.press,
			attachmentId,
			capabilityRevision,
			button: null,
			modifiers: noModifiers,
			pressedButtons: 0,
			x: 0,
			y: 0,
		});
		expect(press).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);

		const motion = encodeInputRequest(MessageKind.mouseInput, {
			action: MouseAction.motion,
			attachmentId,
			capabilityRevision,
			button: null,
			modifiers: noModifiers,
			pressedButtons: 0,
			x: 0,
			y: 0,
		});
		if (!motion.ok) throw new Error(motion.detail);
		motion.value[24] = 0;
		expect(decodeMouseInput(motion.value)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});

	it('matches mouse motion button identity while keeping wheel presses stateless', () => {
		for (const input of [
			{
				action: MouseAction.motion,
				attachmentId,
				capabilityRevision,
				button: null,
				modifiers: noModifiers,
				pressedButtons: 1,
				x: 0,
				y: 0,
			},
			{
				action: MouseAction.motion,
				attachmentId,
				capabilityRevision,
				button: MouseButton.left,
				modifiers: noModifiers,
				pressedButtons: 0,
				x: 0,
				y: 0,
			},
			{
				action: MouseAction.motion,
				attachmentId,
				capabilityRevision,
				button: MouseButton.right,
				modifiers: noModifiers,
				pressedButtons: 1,
				x: 0,
				y: 0,
			},
		]) {
			expect(encodeInputRequest(MessageKind.mouseInput, input).ok).toBe(false);
		}

		const wheel = encodeInputRequest(MessageKind.mouseInput, {
			action: MouseAction.press,
			attachmentId,
			capabilityRevision,
			button: MouseButton.four,
			modifiers: noModifiers,
			pressedButtons: 0,
			x: 0,
			y: 0,
		});
		expect(wheel.ok).toBe(true);
	});

	it('round-trips focus and rejects reserved focus flags', () => {
		const encoded = encodeInputRequest(MessageKind.focusInput, {
			attachmentId,
			capabilityRevision,
			focused: true,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		expect(decodeFocusInput(encoded.value)).toEqual({
			ok: true,
			value: { attachmentId, capabilityRevision, focused: true },
		});
		const malformed = encoded.value.slice();
		malformed[25] = 1;
		expect(decodeFocusInput(malformed)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unknownRequiredFlag, ok: false }),
		);
		expect(
			FocusInputSchema.safeParse({ attachmentId, capabilityRevision, focused: true }).success,
		).toBe(true);
	});
});

describe('semantic-input rejection parity with the native mirror', () => {
	const fPlusOneKeyPayload = new Uint8Array([
		0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
		0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00,
		0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46, 0x2b, 0x31,
	]);

	it('rejects the non-contract F+1 key code as invalid-field', () => {
		expect(decodeKeyInput(fPlusOneKeyPayload)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});

	it('reports length-mismatch for a zero-declared text length with a body', () => {
		const encoded = encodeInputRequest(MessageKind.textInput, {
			attachmentId,
			capabilityRevision,
			text: 'abc',
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const zeroLength = encoded.value.slice();
		zeroLength[24] = 0;
		zeroLength[25] = 0;
		zeroLength[26] = 0;
		zeroLength[27] = 0;
		expect(decodeTextInput(zeroLength)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.lengthMismatch, ok: false }),
		);
		const encodedPaste = encodeInputRequest(MessageKind.pasteInput, {
			attachmentId,
			bracketed: false,
			capabilityRevision,
			text: 'abc',
		});
		if (!encodedPaste.ok) throw new Error(encodedPaste.detail);
		const zeroLengthPaste = encodedPaste.value.slice();
		zeroLengthPaste[28] = 0;
		zeroLengthPaste[29] = 0;
		zeroLengthPaste[30] = 0;
		zeroLengthPaste[31] = 0;
		expect(decodePasteInput(zeroLengthPaste)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.lengthMismatch, ok: false }),
		);
	});

	it('rejects a header-only text payload as invalid-field', () => {
		const headerOnly = new Uint8Array(28);
		headerOnly.set(new Uint8Array(Array.from({ length: 16 }, () => 0x22)));
		new DataView(headerOnly.buffer).setBigUint64(16, 1n, true);
		expect(decodeTextInput(headerOnly)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});

	it('classifies pressed-buttons overflow as invalid-field', () => {
		const encoded = encodeInputRequest(MessageKind.mouseInput, {
			action: MouseAction.motion,
			attachmentId,
			capabilityRevision,
			button: MouseButton.left,
			modifiers: { ...noModifiers, alt: true },
			pressedButtons: 1,
			x: 10.5,
			y: -2.25,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const bit11 = encoded.value.slice();
		bit11[37] = (bit11[37] ?? 0) | 0x08; // pressed-buttons bit 11 (0x0800)
		expect(decodeMouseInput(bit11)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});

	it('retains unknown-required-flag for reserved and unknown modifier bits', () => {
		const encoded = encodeInputRequest(MessageKind.mouseInput, {
			action: MouseAction.motion,
			attachmentId,
			capabilityRevision,
			button: MouseButton.left,
			modifiers: { ...noModifiers, alt: true },
			pressedButtons: 1,
			x: 10.5,
			y: -2.25,
		});
		if (!encoded.ok) throw new Error(encoded.detail);
		const reserved = encoded.value.slice();
		reserved[38] = 1;
		expect(decodeMouseInput(reserved)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unknownRequiredFlag, ok: false }),
		);
		const unknownModifier = encoded.value.slice();
		unknownModifier[26] = 0x00;
		unknownModifier[27] = 0x10; // modifier bit 12
		expect(decodeMouseInput(unknownModifier)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.unknownRequiredFlag, ok: false }),
		);
	});
});
