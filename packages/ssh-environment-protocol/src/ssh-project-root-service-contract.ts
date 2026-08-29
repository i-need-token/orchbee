import { z } from 'zod';

export const SSH_PROJECT_ROOT_INITIAL_CREDIT_BYTES = 128 * 1024;
export const SSH_PROJECT_ROOT_MAX_MESSAGE_BYTES = 128 * 1024;
export const SSH_PROJECT_ROOT_MAX_PATH_BYTES = 4_096;
export const SSH_PROJECT_ROOT_MAX_DIRECTORY_NAME_BYTES = 255;
export const SSH_PROJECT_ROOT_MAX_DIRECTORY_ENTRIES = 256;

const textDecoder = new TextDecoder('utf-8', { fatal: true });
const textEncoder = new TextEncoder();

export const SshProjectRootPathSchema = z
	.string()
	.refine((path) => path.startsWith('/'), 'path must be absolute POSIX')
	.refine((path) => !containsControlCharacter(path), 'path must not contain control characters')
	.refine((path) => isUnicodeScalarSequence(path), 'path must contain valid Unicode scalar values')
	.refine(
		(path) => textEncoder.encode(path).byteLength <= SSH_PROJECT_ROOT_MAX_PATH_BYTES,
		`path must not exceed ${SSH_PROJECT_ROOT_MAX_PATH_BYTES} UTF-8 bytes`,
	)
	.brand<'SshProjectRootPath'>();

export const SshProjectRootFailureReasonSchema = z.enum([
	'invalid-path',
	'not-found',
	'not-directory',
	'permission-denied',
	'timed-out',
	'busy',
	'unavailable',
]);

export const SshProjectRootInspectionRequestSchema = z.strictObject({
	type: z.literal('inspect-project-root'),
	path: SshProjectRootPathSchema,
});

export const SshProjectRootDirectoryNameSchema = z
	.string()
	.min(1)
	.refine((name) => name !== '.' && name !== '..', 'name must identify a child directory')
	.refine((name) => !name.includes('/'), 'name must not contain a POSIX path separator')
	.refine((name) => !containsControlCharacter(name), 'name must not contain control characters')
	.refine((name) => isUnicodeScalarSequence(name), 'name must contain valid Unicode scalar values')
	.refine(
		(name) => textEncoder.encode(name).byteLength <= SSH_PROJECT_ROOT_MAX_DIRECTORY_NAME_BYTES,
		`name must not exceed ${SSH_PROJECT_ROOT_MAX_DIRECTORY_NAME_BYTES} UTF-8 bytes`,
	)
	.brand<'SshProjectRootDirectoryName'>();

export const SshProjectRootDirectoryListingRequestSchema = z.strictObject({
	type: z.literal('list-project-directory'),
	path: SshProjectRootPathSchema.nullable(),
});

export const SshProjectRootDirectoryEntrySchema = z.strictObject({
	name: SshProjectRootDirectoryNameSchema,
});

export const SshProjectRootInspectionResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('resolved'),
		canonicalPath: SshProjectRootPathSchema,
	}),
	z.strictObject({
		kind: z.literal('failed'),
		reason: SshProjectRootFailureReasonSchema,
	}),
]);

export const SshProjectRootDirectoryListingResultSchema = z.discriminatedUnion('kind', [
	z.strictObject({
		kind: z.literal('listed'),
		canonicalPath: SshProjectRootPathSchema,
		parentPath: SshProjectRootPathSchema.nullable(),
		directories: z
			.array(SshProjectRootDirectoryEntrySchema)
			.max(SSH_PROJECT_ROOT_MAX_DIRECTORY_ENTRIES),
		truncated: z.boolean(),
	}),
	z.strictObject({
		kind: z.literal('failed'),
		reason: SshProjectRootFailureReasonSchema,
	}),
]);

export type SshProjectRootPath = z.infer<typeof SshProjectRootPathSchema>;
export type SshProjectRootDirectoryName = z.infer<typeof SshProjectRootDirectoryNameSchema>;
export type SshProjectRootFailureReason = z.infer<typeof SshProjectRootFailureReasonSchema>;
export type SshProjectRootInspectionRequest = z.infer<typeof SshProjectRootInspectionRequestSchema>;
export type SshProjectRootInspectionResult = z.infer<typeof SshProjectRootInspectionResultSchema>;
export type SshProjectRootDirectoryEntry = z.infer<typeof SshProjectRootDirectoryEntrySchema>;
export type SshProjectRootDirectoryListingRequest = z.infer<
	typeof SshProjectRootDirectoryListingRequestSchema
>;
export type SshProjectRootDirectoryListingResult = z.infer<
	typeof SshProjectRootDirectoryListingResultSchema
>;

export interface SshProjectRootMessageDecodeError {
	readonly code: 'message-empty' | 'message-invalid' | 'message-too-large';
}

export type SshProjectRootMessageDecodeResult<Message> =
	| { readonly ok: true; readonly message: Message }
	| { readonly ok: false; readonly error: SshProjectRootMessageDecodeError };

export function encodeSshProjectRootInspectionRequest(
	request: SshProjectRootInspectionRequest,
): Uint8Array {
	return encodeMessage(SshProjectRootInspectionRequestSchema, request);
}

export function decodeSshProjectRootInspectionRequest(
	bytes: Uint8Array,
): SshProjectRootMessageDecodeResult<SshProjectRootInspectionRequest> {
	return decodeMessage(SshProjectRootInspectionRequestSchema, bytes);
}

export function encodeSshProjectRootInspectionResult(
	result: SshProjectRootInspectionResult,
): Uint8Array {
	return encodeMessage(SshProjectRootInspectionResultSchema, result);
}

export function decodeSshProjectRootInspectionResult(
	bytes: Uint8Array,
): SshProjectRootMessageDecodeResult<SshProjectRootInspectionResult> {
	return decodeMessage(SshProjectRootInspectionResultSchema, bytes);
}

export function encodeSshProjectRootDirectoryListingRequest(
	request: SshProjectRootDirectoryListingRequest,
): Uint8Array {
	return encodeMessage(SshProjectRootDirectoryListingRequestSchema, request);
}

export function decodeSshProjectRootDirectoryListingRequest(
	bytes: Uint8Array,
): SshProjectRootMessageDecodeResult<SshProjectRootDirectoryListingRequest> {
	return decodeMessage(SshProjectRootDirectoryListingRequestSchema, bytes);
}

export function encodeSshProjectRootDirectoryListingResult(
	result: SshProjectRootDirectoryListingResult,
): Uint8Array {
	return encodeMessage(SshProjectRootDirectoryListingResultSchema, result);
}

export function decodeSshProjectRootDirectoryListingResult(
	bytes: Uint8Array,
): SshProjectRootMessageDecodeResult<SshProjectRootDirectoryListingResult> {
	return decodeMessage(SshProjectRootDirectoryListingResultSchema, bytes);
}

function encodeMessage<Message>(schema: z.ZodType<Message>, message: Message): Uint8Array {
	const encoded = textEncoder.encode(JSON.stringify(schema.parse(message)));
	if (encoded.byteLength > SSH_PROJECT_ROOT_MAX_MESSAGE_BYTES) {
		throw new RangeError(
			`project-root service message exceeds ${SSH_PROJECT_ROOT_MAX_MESSAGE_BYTES} bytes`,
		);
	}
	return encoded;
}

function decodeMessage<Message>(
	schema: z.ZodType<Message>,
	bytes: Uint8Array,
): SshProjectRootMessageDecodeResult<Message> {
	if (bytes.byteLength === 0) return { ok: false, error: { code: 'message-empty' } };
	if (bytes.byteLength > SSH_PROJECT_ROOT_MAX_MESSAGE_BYTES) {
		return { ok: false, error: { code: 'message-too-large' } };
	}
	try {
		const candidate: unknown = JSON.parse(textDecoder.decode(bytes));
		const parsed = schema.safeParse(candidate);
		return parsed.success
			? { ok: true, message: parsed.data }
			: { ok: false, error: { code: 'message-invalid' } };
	} catch {
		return { ok: false, error: { code: 'message-invalid' } };
	}
}

function containsControlCharacter(value: string): boolean {
	for (const character of value) {
		const codePoint = character.codePointAt(0);
		if (
			codePoint !== undefined &&
			(codePoint <= 0x1f || (codePoint >= 0x7f && codePoint <= 0x9f))
		) {
			return true;
		}
	}
	return false;
}

function isUnicodeScalarSequence(value: string): boolean {
	for (let index = 0; index < value.length; index += 1) {
		const codeUnit = value.charCodeAt(index);
		if (codeUnit < 0xd800 || codeUnit > 0xdfff) continue;
		if (codeUnit > 0xdbff) return false;
		if (index + 1 >= value.length) return false;
		const trailing = value.charCodeAt(index + 1);
		if (trailing < 0xdc00 || trailing > 0xdfff) return false;
		index += 1;
	}
	return true;
}
