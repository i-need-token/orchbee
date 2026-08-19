import { z } from 'zod';

import { ProtocolErrorCode, protocolFailure, protocolSuccess } from './protocol-result';
import type { ProtocolResult } from './protocol-result';

export const IDENTIFIER_BYTE_LENGTH = 16;
export const IDENTIFIER_TEXT_LENGTH = IDENTIFIER_BYTE_LENGTH * 2;
const ZERO_IDENTIFIER_TEXT = '00000000000000000000000000000000';

const identifierPattern = /^[0-9a-f]{32}$/;
const canonicalIdentifierSchema = z
	.string()
	.regex(identifierPattern, 'Identifier must contain exactly 32 lowercase hexadecimal characters')
	.refine((value) => value !== ZERO_IDENTIFIER_TEXT, 'Identifier must not be the zero sentinel');

export const AttachmentIdSchema = canonicalIdentifierSchema.brand<'AttachmentId'>();
export const ClientIdSchema = canonicalIdentifierSchema.brand<'ClientId'>();
export const CorrelationIdSchema = canonicalIdentifierSchema.brand<'CorrelationId'>();
export const CreationIdSchema = canonicalIdentifierSchema.brand<'CreationId'>();
export const HostEpochSchema = canonicalIdentifierSchema.brand<'HostEpoch'>();
export const TerminalIdSchema = canonicalIdentifierSchema.brand<'TerminalId'>();
export const TerminalThemeOperationIdSchema =
	canonicalIdentifierSchema.brand<'TerminalThemeOperationId'>();

export type AttachmentId = z.infer<typeof AttachmentIdSchema>;
export type ClientId = z.infer<typeof ClientIdSchema>;
export type CorrelationId = z.infer<typeof CorrelationIdSchema>;
export type CreationId = z.infer<typeof CreationIdSchema>;
export type HostEpoch = z.infer<typeof HostEpochSchema>;
export type TerminalId = z.infer<typeof TerminalIdSchema>;
export type TerminalThemeOperationId = z.infer<typeof TerminalThemeOperationIdSchema>;

export function identifierBytes(value: string): Uint8Array {
	const bytes = new Uint8Array(IDENTIFIER_BYTE_LENGTH);
	for (let index = 0; index < IDENTIFIER_BYTE_LENGTH; index += 1) {
		bytes[index] = Number.parseInt(value.slice(index * 2, index * 2 + 2), 16);
	}
	return bytes;
}

export function identifierText(bytes: Uint8Array): ProtocolResult<string> {
	if (bytes.byteLength !== IDENTIFIER_BYTE_LENGTH) {
		return protocolFailure(
			ProtocolErrorCode.invalidIdentifier,
			`Identifier requires ${IDENTIFIER_BYTE_LENGTH} bytes`,
		);
	}

	let value = '';
	for (const byte of bytes) value += byte.toString(16).padStart(2, '0');
	if (value === ZERO_IDENTIFIER_TEXT) {
		return protocolFailure(
			ProtocolErrorCode.invalidIdentifier,
			'Identifier must not use the zero sentinel',
		);
	}
	return protocolSuccess(value);
}

function parseIdentifier<Schema extends z.ZodType>(
	schema: Schema,
	value: unknown,
	name: string,
): ProtocolResult<z.output<Schema>> {
	const parsed = schema.safeParse(value);
	if (!parsed.success) {
		return protocolFailure(
			ProtocolErrorCode.invalidIdentifier,
			`${name} is not a canonical non-zero 128-bit identifier`,
		);
	}
	return protocolSuccess(parsed.data);
}

export function parseAttachmentId(value: unknown): ProtocolResult<AttachmentId> {
	return parseIdentifier(AttachmentIdSchema, value, 'AttachmentId');
}

export function parseClientId(value: unknown): ProtocolResult<ClientId> {
	return parseIdentifier(ClientIdSchema, value, 'ClientId');
}

export function parseCorrelationId(value: unknown): ProtocolResult<CorrelationId> {
	return parseIdentifier(CorrelationIdSchema, value, 'CorrelationId');
}

export function parseCreationId(value: unknown): ProtocolResult<CreationId> {
	return parseIdentifier(CreationIdSchema, value, 'CreationId');
}

export function parseHostEpoch(value: unknown): ProtocolResult<HostEpoch> {
	return parseIdentifier(HostEpochSchema, value, 'HostEpoch');
}

export function parseTerminalId(value: unknown): ProtocolResult<TerminalId> {
	return parseIdentifier(TerminalIdSchema, value, 'TerminalId');
}

export function parseTerminalThemeOperationId(
	value: unknown,
): ProtocolResult<TerminalThemeOperationId> {
	return parseIdentifier(TerminalThemeOperationIdSchema, value, 'TerminalThemeOperationId');
}
