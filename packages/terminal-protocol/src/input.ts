import { z } from 'zod';

import { BinaryReader, BinaryWriter, checkedAdd } from './binary';
import {
	AttachmentIdSchema,
	IDENTIFIER_BYTE_LENGTH,
	identifierBytes,
	identifierText,
	parseAttachmentId,
} from './identifiers';
import type { AttachmentId } from './identifiers';
import { CanonicalUint64TextSchema } from './control';
import type { CanonicalUint64Text } from './control';
import { isInputRequestKind, MessageKind } from './envelope';
import type { MessageKind as MessageKindValue } from './envelope';
import { PROTOCOL_LIMITS } from './limits';
import { ProtocolErrorCode, protocolFailure, protocolSuccess } from './protocol-result';
import type { ProtocolResult } from './protocol-result';
import { containsUnpairedSurrogate } from './containsUnpairedSurrogate';

const INPUT_AUTHORITY_HEADER_BYTES = IDENTIFIER_BYTE_LENGTH + 8;
export const WRITE_INPUT_HEADER_BYTES = INPUT_AUTHORITY_HEADER_BYTES;
export const KEY_INPUT_HEADER_BYTES = 40;
export const TEXT_INPUT_HEADER_BYTES = 28;
export const PASTE_INPUT_HEADER_BYTES = 32;
export const MOUSE_INPUT_BYTES = 40;
export const FOCUS_INPUT_BYTES = 28;

const MAX_UNICODE_CODEPOINT = 0x10_ffff;
const MAX_FLOAT32 = 3.4028234663852886e38;
const KEY_FLAGS_MASK = 0b11;
const PASTE_FLAGS_MASK = 0b1;
const MODIFIER_FLAGS_MASK = 0x03ff;
const MOUSE_BUTTON_FLAGS_MASK = 0x07ff;
const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
const capabilityRevisionSchema = CanonicalUint64TextSchema.refine(
	(value) => value !== '0',
	'Capability revision must be greater than zero',
);

export const TerminalKeyCode = [
	'Unidentified',
	'Backquote',
	'Backslash',
	'BracketLeft',
	'BracketRight',
	'Comma',
	'Digit0',
	'Digit1',
	'Digit2',
	'Digit3',
	'Digit4',
	'Digit5',
	'Digit6',
	'Digit7',
	'Digit8',
	'Digit9',
	'Equal',
	'IntlBackslash',
	'IntlRo',
	'IntlYen',
	'KeyA',
	'KeyB',
	'KeyC',
	'KeyD',
	'KeyE',
	'KeyF',
	'KeyG',
	'KeyH',
	'KeyI',
	'KeyJ',
	'KeyK',
	'KeyL',
	'KeyM',
	'KeyN',
	'KeyO',
	'KeyP',
	'KeyQ',
	'KeyR',
	'KeyS',
	'KeyT',
	'KeyU',
	'KeyV',
	'KeyW',
	'KeyX',
	'KeyY',
	'KeyZ',
	'Minus',
	'Period',
	'Quote',
	'Semicolon',
	'Slash',
	'AltLeft',
	'AltRight',
	'Backspace',
	'CapsLock',
	'ContextMenu',
	'ControlLeft',
	'ControlRight',
	'Enter',
	'MetaLeft',
	'MetaRight',
	'ShiftLeft',
	'ShiftRight',
	'Space',
	'Tab',
	'Convert',
	'KanaMode',
	'NonConvert',
	'Delete',
	'End',
	'Help',
	'Home',
	'Insert',
	'PageDown',
	'PageUp',
	'ArrowDown',
	'ArrowLeft',
	'ArrowRight',
	'ArrowUp',
	'NumLock',
	'Numpad0',
	'Numpad1',
	'Numpad2',
	'Numpad3',
	'Numpad4',
	'Numpad5',
	'Numpad6',
	'Numpad7',
	'Numpad8',
	'Numpad9',
	'NumpadAdd',
	'NumpadBackspace',
	'NumpadClear',
	'NumpadClearEntry',
	'NumpadComma',
	'NumpadDecimal',
	'NumpadDivide',
	'NumpadEnter',
	'NumpadEqual',
	'NumpadMemoryAdd',
	'NumpadMemoryClear',
	'NumpadMemoryRecall',
	'NumpadMemoryStore',
	'NumpadMemorySubtract',
	'NumpadMultiply',
	'NumpadParenLeft',
	'NumpadParenRight',
	'NumpadSubtract',
	'NumpadSeparator',
	'NumpadUp',
	'NumpadDown',
	'NumpadRight',
	'NumpadLeft',
	'NumpadBegin',
	'NumpadHome',
	'NumpadEnd',
	'NumpadInsert',
	'NumpadDelete',
	'NumpadPageUp',
	'NumpadPageDown',
	'Escape',
	'F1',
	'F2',
	'F3',
	'F4',
	'F5',
	'F6',
	'F7',
	'F8',
	'F9',
	'F10',
	'F11',
	'F12',
	'F13',
	'F14',
	'F15',
	'F16',
	'F17',
	'F18',
	'F19',
	'F20',
	'F21',
	'F22',
	'F23',
	'F24',
	'F25',
	'Fn',
	'FnLock',
	'PrintScreen',
	'ScrollLock',
	'Pause',
	'BrowserBack',
	'BrowserFavorites',
	'BrowserForward',
	'BrowserHome',
	'BrowserRefresh',
	'BrowserSearch',
	'BrowserStop',
	'Eject',
	'LaunchApp1',
	'LaunchApp2',
	'LaunchMail',
	'MediaPlayPause',
	'MediaSelect',
	'MediaStop',
	'MediaTrackNext',
	'MediaTrackPrevious',
	'Power',
	'Sleep',
	'AudioVolumeDown',
	'AudioVolumeMute',
	'AudioVolumeUp',
	'WakeUp',
	'Copy',
	'Cut',
	'Paste',
] as const;

export const KeyAction = {
	press: 'press',
	release: 'release',
	repeat: 'repeat',
} as const;

export const MouseAction = {
	motion: 'motion',
	press: 'press',
	release: 'release',
} as const;

export const MouseButton = {
	eight: 8,
	eleven: 11,
	five: 5,
	four: 4,
	left: 1,
	middle: 3,
	nine: 9,
	right: 2,
	seven: 7,
	six: 6,
	ten: 10,
} as const;

const keyCodeSchema = z.enum(TerminalKeyCode);
const keyActionSchema = z.enum(KeyAction);
const mouseActionSchema = z.enum(MouseAction);
const mouseButtonSchema = z.union([
	z.literal(MouseButton.left),
	z.literal(MouseButton.right),
	z.literal(MouseButton.middle),
	z.literal(MouseButton.four),
	z.literal(MouseButton.five),
	z.literal(MouseButton.six),
	z.literal(MouseButton.seven),
	z.literal(MouseButton.eight),
	z.literal(MouseButton.nine),
	z.literal(MouseButton.ten),
	z.literal(MouseButton.eleven),
]);

const modifierNames = [
	'alt',
	'altRight',
	'capsLock',
	'control',
	'controlRight',
	'meta',
	'metaRight',
	'numLock',
	'shift',
	'shiftRight',
] as const;

export const TerminalModifiersSchema = z
	.strictObject({
		alt: z.boolean(),
		altRight: z.boolean(),
		capsLock: z.boolean(),
		control: z.boolean(),
		controlRight: z.boolean(),
		meta: z.boolean(),
		metaRight: z.boolean(),
		numLock: z.boolean(),
		shift: z.boolean(),
		shiftRight: z.boolean(),
	})
	.superRefine((modifiers, context) => {
		for (const [right, base] of [
			['altRight', 'alt'],
			['controlRight', 'control'],
			['metaRight', 'meta'],
			['shiftRight', 'shift'],
		] as const) {
			if (modifiers[right] && !modifiers[base]) {
				context.addIssue({
					code: 'custom',
					message: `${right} requires ${base}`,
					path: [right],
				});
			}
		}
	});

const unicodeCodepointSchema = z
	.number()
	.int()
	.min(0)
	.max(MAX_UNICODE_CODEPOINT)
	.refine((value) => value < 0xd800 || value > 0xdfff, 'Codepoint cannot be a UTF-16 surrogate');

const boundedInputTextSchema = z
	.string()
	.min(1)
	.superRefine((value, context) => {
		if (containsUnpairedSurrogate(value)) {
			context.addIssue({ code: 'custom', message: 'Input text contains an unpaired surrogate' });
			return;
		}
		if (textEncoder.encode(value).byteLength > PROTOCOL_LIMITS.maxInputBytes) {
			context.addIssue({
				code: 'custom',
				message: `Input text exceeds ${PROTOCOL_LIMITS.maxInputBytes} UTF-8 bytes`,
			});
		}
	});

const boundedKeyTextSchema = z.string().superRefine((value, context) => {
	if (containsUnpairedSurrogate(value)) {
		context.addIssue({ code: 'custom', message: 'Key text contains an unpaired surrogate' });
		return;
	}
	if (textEncoder.encode(value).byteLength > PROTOCOL_LIMITS.maxKeyTextBytes) {
		context.addIssue({
			code: 'custom',
			message: `Key text exceeds ${PROTOCOL_LIMITS.maxKeyTextBytes} UTF-8 bytes`,
		});
	}
});

export const WriteInputSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	bytes: z
		.instanceof(Uint8Array)
		.refine(
			(value) => value.byteLength >= 1 && value.byteLength <= PROTOCOL_LIMITS.maxInputBytes,
			`Input must contain 1 to ${PROTOCOL_LIMITS.maxInputBytes} bytes`,
		),
	capabilityRevision: capabilityRevisionSchema,
});

export const KeyInputSchema = z
	.strictObject({
		action: keyActionSchema,
		attachmentId: AttachmentIdSchema,
		capabilityRevision: capabilityRevisionSchema,
		composing: z.boolean(),
		consumedModifiers: TerminalModifiersSchema,
		key: keyCodeSchema,
		modifiers: TerminalModifiersSchema,
		optionAsAlt: z.boolean(),
		text: boundedKeyTextSchema,
		unshiftedCodepoint: unicodeCodepointSchema,
	})
	.superRefine((input, context) => {
		for (const modifier of modifierNames) {
			if (input.consumedModifiers[modifier] && !input.modifiers[modifier]) {
				context.addIssue({
					code: 'custom',
					message: 'Consumed modifiers must be a subset of active modifiers',
					path: ['consumedModifiers', modifier],
				});
			}
		}
	});

const attachmentTextShape = {
	attachmentId: AttachmentIdSchema,
	capabilityRevision: capabilityRevisionSchema,
	text: boundedInputTextSchema,
} as const;

export const TextInputSchema = z.strictObject(attachmentTextShape);

export const PasteInputSchema = z.strictObject({
	...attachmentTextShape,
	bracketed: z.boolean(),
});

export const MouseInputSchema = z
	.strictObject({
		action: mouseActionSchema,
		attachmentId: AttachmentIdSchema,
		capabilityRevision: capabilityRevisionSchema,
		button: mouseButtonSchema.nullable(),
		modifiers: TerminalModifiersSchema,
		pressedButtons: z.number().int().min(0).max(MOUSE_BUTTON_FLAGS_MASK),
		x: z.number().finite().min(-MAX_FLOAT32).max(MAX_FLOAT32),
		y: z.number().finite().min(-MAX_FLOAT32).max(MAX_FLOAT32),
	})
	.superRefine((input, context) => {
		if (input.action !== MouseAction.motion && input.button === null) {
			context.addIssue({
				code: 'custom',
				message: 'Mouse press and release require a button',
				path: ['button'],
			});
		}
		if (input.action === MouseAction.motion) {
			if (input.button === null && input.pressedButtons !== 0) {
				context.addIssue({
					code: 'custom',
					message: 'Mouse motion with pressed buttons requires an active button',
					path: ['button'],
				});
			} else if (
				input.button !== null &&
				(input.pressedButtons & (1 << (input.button - 1))) === 0
			) {
				context.addIssue({
					code: 'custom',
					message: 'Mouse motion button must be present in pressedButtons',
					path: ['pressedButtons'],
				});
			}
		}
	});

export const FocusInputSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	capabilityRevision: capabilityRevisionSchema,
	focused: z.boolean(),
});

export type FocusInput = z.infer<typeof FocusInputSchema>;
export type KeyAction = (typeof KeyAction)[keyof typeof KeyAction];
export type KeyInput = z.infer<typeof KeyInputSchema>;
export type MouseAction = (typeof MouseAction)[keyof typeof MouseAction];
export type MouseButton = (typeof MouseButton)[keyof typeof MouseButton];
export type MouseInput = z.infer<typeof MouseInputSchema>;
export type PasteInput = z.infer<typeof PasteInputSchema>;
export type TerminalKeyCode = (typeof TerminalKeyCode)[number];
export type TerminalModifiers = z.infer<typeof TerminalModifiersSchema>;
export type TextInput = z.infer<typeof TextInputSchema>;
export type WriteInput = z.infer<typeof WriteInputSchema>;

export interface DecodedWriteInput {
	readonly attachmentId: AttachmentId;
	readonly bytes: Uint8Array;
	readonly capabilityRevision: CanonicalUint64Text;
}

function validationFailure(
	name: string,
	issues: readonly { readonly message: string }[],
): ProtocolResult<never> {
	return protocolFailure(
		ProtocolErrorCode.invalidField,
		`${name} validation failed: ${issues[0]?.message ?? 'unknown field error'}`,
	);
}

function decodeAttachmentId(bytes: Uint8Array): ProtocolResult<AttachmentId> {
	const identifier = identifierText(bytes);
	if (!identifier.ok) return identifier;
	return parseAttachmentId(identifier.value);
}

function decodeCapabilityRevision(reader: BinaryReader): ProtocolResult<CanonicalUint64Text> {
	const value = reader.readUint64();
	if (reader.failure) return reader.failure;
	const parsed = capabilityRevisionSchema.safeParse(value.toString());
	if (!parsed.success) return validationFailure('Capability revision', parsed.error.issues);
	return protocolSuccess(parsed.data);
}

function keyActionCode(action: KeyAction): number {
	switch (action) {
		case KeyAction.release:
			return 0;
		case KeyAction.press:
			return 1;
		case KeyAction.repeat:
			return 2;
	}
}

function keyActionFromCode(code: number): ProtocolResult<KeyAction> {
	switch (code) {
		case 0:
			return protocolSuccess(KeyAction.release);
		case 1:
			return protocolSuccess(KeyAction.press);
		case 2:
			return protocolSuccess(KeyAction.repeat);
		default:
			return protocolFailure(ProtocolErrorCode.invalidField, `Unknown key action ${code}`);
	}
}

function mouseActionCode(action: MouseAction): number {
	switch (action) {
		case MouseAction.press:
			return 0;
		case MouseAction.release:
			return 1;
		case MouseAction.motion:
			return 2;
	}
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

function encodeModifiers(modifiers: TerminalModifiers): number {
	return (
		(modifiers.shift ? 1 : 0) |
		(modifiers.control ? 1 << 1 : 0) |
		(modifiers.alt ? 1 << 2 : 0) |
		(modifiers.meta ? 1 << 3 : 0) |
		(modifiers.capsLock ? 1 << 4 : 0) |
		(modifiers.numLock ? 1 << 5 : 0) |
		(modifiers.shiftRight ? 1 << 6 : 0) |
		(modifiers.controlRight ? 1 << 7 : 0) |
		(modifiers.altRight ? 1 << 8 : 0) |
		(modifiers.metaRight ? 1 << 9 : 0)
	);
}

function decodeModifiers(flags: number): ProtocolResult<TerminalModifiers> {
	if ((flags & ~MODIFIER_FLAGS_MASK) !== 0) {
		return protocolFailure(
			ProtocolErrorCode.unknownRequiredFlag,
			'Terminal modifiers contain unknown required bits',
		);
	}
	const parsed = TerminalModifiersSchema.safeParse({
		alt: (flags & (1 << 2)) !== 0,
		altRight: (flags & (1 << 8)) !== 0,
		capsLock: (flags & (1 << 4)) !== 0,
		control: (flags & (1 << 1)) !== 0,
		controlRight: (flags & (1 << 7)) !== 0,
		meta: (flags & (1 << 3)) !== 0,
		metaRight: (flags & (1 << 9)) !== 0,
		numLock: (flags & (1 << 5)) !== 0,
		shift: (flags & 1) !== 0,
		shiftRight: (flags & (1 << 6)) !== 0,
	});
	if (!parsed.success) return validationFailure('Terminal modifiers', parsed.error.issues);
	return protocolSuccess(parsed.data);
}

function decodeUtf8(bytes: Uint8Array, field: string): ProtocolResult<string> {
	try {
		return protocolSuccess(textDecoder.decode(bytes));
	} catch {
		return protocolFailure(ProtocolErrorCode.invalidUtf8, `${field} is not valid UTF-8`);
	}
}

export function encodeWriteInput(value: unknown): ProtocolResult<Uint8Array> {
	const parsed = WriteInputSchema.safeParse(value);
	if (!parsed.success) return validationFailure('WriteInput', parsed.error.issues);

	const writer = new BinaryWriter(WRITE_INPUT_HEADER_BYTES + parsed.data.bytes.byteLength);
	writer.writeBytes(identifierBytes(parsed.data.attachmentId));
	writer.writeUint64(BigInt(parsed.data.capabilityRevision));
	writer.writeBytes(parsed.data.bytes);
	return protocolSuccess(writer.bytes());
}

/**
 * Encodes only client-to-Host input requests. The message kind chooses the
 * binary codec, so producers do not maintain a parallel kind-to-format map.
 */
export function encodeInputRequest(
	kind: MessageKindValue,
	value: unknown,
): ProtocolResult<Uint8Array> {
	if (!isInputRequestKind(kind)) {
		return protocolFailure(
			ProtocolErrorCode.invalidField,
			`Message kind ${kind} is not an input request`,
		);
	}
	switch (kind) {
		case MessageKind.writeInput:
			return encodeWriteInput(value);
		case MessageKind.keyInput:
			return encodeKeyInput(value);
		case MessageKind.textInput:
			return encodeTextInput(value);
		case MessageKind.mouseInput:
			return encodeMouseInput(value);
		case MessageKind.focusInput:
			return encodeFocusInput(value);
		case MessageKind.pasteInput:
			return encodePasteInput(value);
	}
	return missingInputRequestCodec(kind);
}

function missingInputRequestCodec(kind: never): ProtocolResult<Uint8Array> {
	return protocolFailure(
		ProtocolErrorCode.invalidField,
		`Input request codec is missing for message kind ${String(kind)}`,
	);
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

export function encodeKeyInput(value: unknown): ProtocolResult<Uint8Array> {
	const parsed = KeyInputSchema.safeParse(value);
	if (!parsed.success) return validationFailure('KeyInput', parsed.error.issues);
	const keyBytes = textEncoder.encode(parsed.data.key);
	const textBytes = textEncoder.encode(parsed.data.text);
	const totalLength = checkedAdd(KEY_INPUT_HEADER_BYTES, keyBytes.byteLength, textBytes.byteLength);
	if (!totalLength.ok) return totalLength;
	if (totalLength.value > PROTOCOL_LIMITS.maxSemanticInputPayloadBytes) {
		return protocolFailure(ProtocolErrorCode.limitExceeded, 'KeyInput payload exceeds its limit');
	}

	const writer = new BinaryWriter(totalLength.value);
	writer.writeBytes(identifierBytes(parsed.data.attachmentId));
	writer.writeUint64(BigInt(parsed.data.capabilityRevision));
	writer.writeUint8(keyActionCode(parsed.data.action));
	writer.writeUint8((parsed.data.composing ? 1 : 0) | (parsed.data.optionAsAlt ? 1 << 1 : 0));
	writer.writeUint16(encodeModifiers(parsed.data.modifiers));
	writer.writeUint16(encodeModifiers(parsed.data.consumedModifiers));
	writer.writeUint16(keyBytes.byteLength);
	writer.writeUint32(parsed.data.unshiftedCodepoint);
	writer.writeUint32(textBytes.byteLength);
	writer.writeBytes(keyBytes);
	writer.writeBytes(textBytes);
	return protocolSuccess(writer.bytes());
}

export function decodeKeyInput(payload: Uint8Array): ProtocolResult<KeyInput> {
	if (payload.byteLength < KEY_INPUT_HEADER_BYTES) {
		return protocolFailure(ProtocolErrorCode.truncated, 'KeyInput header is truncated');
	}
	if (payload.byteLength > PROTOCOL_LIMITS.maxSemanticInputPayloadBytes) {
		return protocolFailure(ProtocolErrorCode.limitExceeded, 'KeyInput payload exceeds its limit');
	}
	const reader = new BinaryReader(payload);
	const attachmentId = decodeAttachmentId(reader.readBytes(IDENTIFIER_BYTE_LENGTH));
	if (!attachmentId.ok) return attachmentId;
	const capabilityRevision = decodeCapabilityRevision(reader);
	if (!capabilityRevision.ok) return capabilityRevision;
	const action = keyActionFromCode(reader.readUint8());
	if (!action.ok) return action;
	const flags = reader.readUint8();
	const modifiers = decodeModifiers(reader.readUint16());
	if (!modifiers.ok) return modifiers;
	const consumedModifiers = decodeModifiers(reader.readUint16());
	if (!consumedModifiers.ok) return consumedModifiers;
	const keyLength = reader.readUint16();
	const unshiftedCodepoint = reader.readUint32();
	const textLength = reader.readUint32();
	if (reader.failure) return reader.failure;
	if ((flags & ~KEY_FLAGS_MASK) !== 0) {
		return protocolFailure(ProtocolErrorCode.unknownRequiredFlag, 'KeyInput has unknown flags');
	}
	if (textLength > PROTOCOL_LIMITS.maxKeyTextBytes) {
		return protocolFailure(ProtocolErrorCode.limitExceeded, 'KeyInput text exceeds its limit');
	}
	if (keyLength + textLength !== payload.byteLength - KEY_INPUT_HEADER_BYTES) {
		return protocolFailure(
			ProtocolErrorCode.lengthMismatch,
			'KeyInput lengths do not match payload',
		);
	}
	const keyText = decodeUtf8(reader.readBytes(keyLength), 'KeyInput key');
	if (!keyText.ok) return keyText;
	const key = keyCodeSchema.safeParse(keyText.value);
	if (!key.success) return validationFailure('KeyInput key', key.error.issues);
	const text = decodeUtf8(reader.readBytes(textLength), 'KeyInput text');
	if (!text.ok) return text;
	const parsed = KeyInputSchema.safeParse({
		action: action.value,
		attachmentId: attachmentId.value,
		capabilityRevision: capabilityRevision.value,
		composing: (flags & 1) !== 0,
		consumedModifiers: consumedModifiers.value,
		key: key.data,
		modifiers: modifiers.value,
		optionAsAlt: (flags & (1 << 1)) !== 0,
		text: text.value,
		unshiftedCodepoint,
	});
	if (!parsed.success) return validationFailure('KeyInput', parsed.error.issues);
	return protocolSuccess(parsed.data);
}

function encodeTextPayload(
	value: unknown,
	schema: typeof TextInputSchema,
	name: string,
): ProtocolResult<Uint8Array> {
	const parsed = schema.safeParse(value);
	if (!parsed.success) return validationFailure(name, parsed.error.issues);
	const textBytes = textEncoder.encode(parsed.data.text);
	const writer = new BinaryWriter(TEXT_INPUT_HEADER_BYTES + textBytes.byteLength);
	writer.writeBytes(identifierBytes(parsed.data.attachmentId));
	writer.writeUint64(BigInt(parsed.data.capabilityRevision));
	writer.writeUint32(textBytes.byteLength);
	writer.writeBytes(textBytes);
	return protocolSuccess(writer.bytes());
}

function decodeTextPayload(
	payload: Uint8Array,
	name: string,
	schema: typeof TextInputSchema,
): ProtocolResult<TextInput> {
	if (payload.byteLength < TEXT_INPUT_HEADER_BYTES) {
		return protocolFailure(ProtocolErrorCode.truncated, `${name} header is truncated`);
	}
	if (payload.byteLength > TEXT_INPUT_HEADER_BYTES + PROTOCOL_LIMITS.maxInputBytes) {
		return protocolFailure(ProtocolErrorCode.limitExceeded, `${name} payload exceeds its limit`);
	}
	const reader = new BinaryReader(payload);
	const attachmentId = decodeAttachmentId(reader.readBytes(IDENTIFIER_BYTE_LENGTH));
	if (!attachmentId.ok) return attachmentId;
	const capabilityRevision = decodeCapabilityRevision(reader);
	if (!capabilityRevision.ok) return capabilityRevision;
	const textLength = reader.readUint32();
	if (reader.failure) return reader.failure;
	if (textLength !== payload.byteLength - TEXT_INPUT_HEADER_BYTES) {
		return protocolFailure(
			ProtocolErrorCode.lengthMismatch,
			`${name} length does not match payload`,
		);
	}
	const text = decodeUtf8(reader.readBytes(textLength), `${name} text`);
	if (!text.ok) return text;
	const parsed = schema.safeParse({
		attachmentId: attachmentId.value,
		capabilityRevision: capabilityRevision.value,
		text: text.value,
	});
	if (!parsed.success) return validationFailure(name, parsed.error.issues);
	return protocolSuccess(parsed.data);
}

export function encodeTextInput(value: unknown): ProtocolResult<Uint8Array> {
	return encodeTextPayload(value, TextInputSchema, 'TextInput');
}

export function decodeTextInput(payload: Uint8Array): ProtocolResult<TextInput> {
	return decodeTextPayload(payload, 'TextInput', TextInputSchema);
}

export function encodePasteInput(value: unknown): ProtocolResult<Uint8Array> {
	const parsed = PasteInputSchema.safeParse(value);
	if (!parsed.success) return validationFailure('PasteInput', parsed.error.issues);
	const textBytes = textEncoder.encode(parsed.data.text);
	const writer = new BinaryWriter(PASTE_INPUT_HEADER_BYTES + textBytes.byteLength);
	writer.writeBytes(identifierBytes(parsed.data.attachmentId));
	writer.writeUint64(BigInt(parsed.data.capabilityRevision));
	writer.writeUint8(parsed.data.bracketed ? 1 : 0);
	writer.writeUint8(0);
	writer.writeUint16(0);
	writer.writeUint32(textBytes.byteLength);
	writer.writeBytes(textBytes);
	return protocolSuccess(writer.bytes());
}

export function decodePasteInput(payload: Uint8Array): ProtocolResult<PasteInput> {
	if (payload.byteLength < PASTE_INPUT_HEADER_BYTES) {
		return protocolFailure(ProtocolErrorCode.truncated, 'PasteInput header is truncated');
	}
	if (payload.byteLength > PASTE_INPUT_HEADER_BYTES + PROTOCOL_LIMITS.maxInputBytes) {
		return protocolFailure(ProtocolErrorCode.limitExceeded, 'PasteInput payload exceeds its limit');
	}
	const reader = new BinaryReader(payload);
	const attachmentId = decodeAttachmentId(reader.readBytes(IDENTIFIER_BYTE_LENGTH));
	if (!attachmentId.ok) return attachmentId;
	const capabilityRevision = decodeCapabilityRevision(reader);
	if (!capabilityRevision.ok) return capabilityRevision;
	const flags = reader.readUint8();
	const reservedByte = reader.readUint8();
	const reservedWord = reader.readUint16();
	const textLength = reader.readUint32();
	if (reader.failure) return reader.failure;
	if ((flags & ~PASTE_FLAGS_MASK) !== 0 || reservedByte !== 0 || reservedWord !== 0) {
		return protocolFailure(ProtocolErrorCode.unknownRequiredFlag, 'PasteInput has unknown flags');
	}
	if (textLength !== payload.byteLength - PASTE_INPUT_HEADER_BYTES) {
		return protocolFailure(
			ProtocolErrorCode.lengthMismatch,
			'PasteInput length does not match payload',
		);
	}
	const text = decodeUtf8(reader.readBytes(textLength), 'PasteInput text');
	if (!text.ok) return text;
	const parsed = PasteInputSchema.safeParse({
		attachmentId: attachmentId.value,
		bracketed: (flags & 1) !== 0,
		capabilityRevision: capabilityRevision.value,
		text: text.value,
	});
	if (!parsed.success) return validationFailure('PasteInput', parsed.error.issues);
	return protocolSuccess(parsed.data);
}

export function encodeMouseInput(value: unknown): ProtocolResult<Uint8Array> {
	const parsed = MouseInputSchema.safeParse(value);
	if (!parsed.success) return validationFailure('MouseInput', parsed.error.issues);
	const writer = new BinaryWriter(MOUSE_INPUT_BYTES);
	writer.writeBytes(identifierBytes(parsed.data.attachmentId));
	writer.writeUint64(BigInt(parsed.data.capabilityRevision));
	writer.writeUint8(mouseActionCode(parsed.data.action));
	writer.writeUint8(parsed.data.button ?? 0);
	writer.writeUint16(encodeModifiers(parsed.data.modifiers));
	writer.writeFloat32(parsed.data.x);
	writer.writeFloat32(parsed.data.y);
	writer.writeUint16(parsed.data.pressedButtons);
	writer.writeUint16(0);
	return protocolSuccess(writer.bytes());
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

export function encodeFocusInput(value: unknown): ProtocolResult<Uint8Array> {
	const parsed = FocusInputSchema.safeParse(value);
	if (!parsed.success) return validationFailure('FocusInput', parsed.error.issues);
	const writer = new BinaryWriter(FOCUS_INPUT_BYTES);
	writer.writeBytes(identifierBytes(parsed.data.attachmentId));
	writer.writeUint64(BigInt(parsed.data.capabilityRevision));
	writer.writeUint8(parsed.data.focused ? 1 : 0);
	writer.writeBytes(new Uint8Array(3));
	return protocolSuccess(writer.bytes());
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
