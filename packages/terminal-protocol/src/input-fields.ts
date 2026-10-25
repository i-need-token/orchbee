import { z } from 'zod';
import type { BinaryReader } from './binary';
import {
	AttachmentIdSchema,
	IDENTIFIER_BYTE_LENGTH,
	identifierText,
	parseAttachmentId,
	type AttachmentId,
} from './identifiers';
import { CanonicalUint64TextSchema, type CanonicalUint64Text } from './control';
import { PROTOCOL_LIMITS } from './limits';
import {
	ProtocolErrorCode,
	protocolFailure,
	protocolSuccess,
	type ProtocolResult,
} from './protocol-result';

const INPUT_AUTHORITY_HEADER_BYTES = IDENTIFIER_BYTE_LENGTH + 8;

export const WRITE_INPUT_HEADER_BYTES = INPUT_AUTHORITY_HEADER_BYTES;

export const PASTE_AUTO_INPUT_HEADER_BYTES = 32;

export const MOUSE_INPUT_BYTES = 40;

export const FOCUS_INPUT_BYTES = 28;

const MODIFIER_FLAGS_MASK = 0x03ff;

export const capabilityRevisionSchema = CanonicalUint64TextSchema.refine(
	(value) => value !== '0',
	'Capability revision must be greater than zero',
);

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

export const PasteAutoInputSchema = z.strictObject({
	attachmentId: AttachmentIdSchema,
	capabilityRevision: capabilityRevisionSchema,
	content: z
		.instanceof(Uint8Array)
		.refine(
			(value) =>
				value.byteLength >= 1 && value.byteLength <= PROTOCOL_LIMITS.maxPasteAutoInputBytes,
			`Paste content must contain 1 to ${PROTOCOL_LIMITS.maxPasteAutoInputBytes} bytes`,
		),
});

export type TerminalModifiers = z.infer<typeof TerminalModifiersSchema>;

export function validationFailure(
	name: string,
	issues: readonly { readonly message: string }[],
): ProtocolResult<never> {
	return protocolFailure(
		ProtocolErrorCode.invalidField,
		`${name} validation failed: ${issues[0]?.message ?? 'unknown field error'}`,
	);
}

export function decodeAttachmentId(bytes: Uint8Array): ProtocolResult<AttachmentId> {
	const identifier = identifierText(bytes);
	if (!identifier.ok) return identifier;
	return parseAttachmentId(identifier.value);
}

export function decodeCapabilityRevision(
	reader: BinaryReader,
): ProtocolResult<CanonicalUint64Text> {
	const value = reader.readUint64();
	if (reader.failure) return reader.failure;
	const parsed = capabilityRevisionSchema.safeParse(value.toString());
	if (!parsed.success) return validationFailure('Capability revision', parsed.error.issues);
	return protocolSuccess(parsed.data);
}

export function decodeModifiers(flags: number): ProtocolResult<TerminalModifiers> {
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
