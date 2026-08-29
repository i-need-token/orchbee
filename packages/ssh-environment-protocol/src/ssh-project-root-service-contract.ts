import { z } from 'zod';

export const SSH_PROJECT_ROOT_INITIAL_CREDIT_BYTES = 65_536;
export const SSH_PROJECT_ROOT_MAX_MESSAGE_BYTES = 16 * 1024;
export const SSH_PROJECT_ROOT_MAX_PATH_BYTES = 4_096;

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

export type SshProjectRootPath = z.infer<typeof SshProjectRootPathSchema>;
export type SshProjectRootFailureReason = z.infer<typeof SshProjectRootFailureReasonSchema>;
export type SshProjectRootInspectionRequest = z.infer<typeof SshProjectRootInspectionRequestSchema>;
export type SshProjectRootInspectionResult = z.infer<typeof SshProjectRootInspectionResultSchema>;

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
