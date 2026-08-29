import { SEMANTIC_COMMAND_PROTOCOL_VERSION } from '../identity-contract';
import {
	SEMANTIC_COMMAND_LOCAL_HTTP_MAX_HEADER_BYTES,
	SEMANTIC_COMMAND_LOCAL_HTTP_MAX_REQUEST_BODY_BYTES,
	SEMANTIC_COMMAND_LOCAL_HTTP_MAX_RESPONSE_BODY_BYTES,
	type SemanticCommandLocalHttpError,
	type SemanticCommandLocalHttpErrorCode,
} from './local-http-contract';

const headerTerminator = new Uint8Array([13, 10, 13, 10]);
const headerNamePattern = /^[!#$%&'*+.^_`|~0-9A-Za-z-]+$/u;
const requestTargetPattern = /^\/[!-~]*$/u;
const canonicalDecimalPattern = /^(?:0|[1-9][0-9]*)$/u;

export interface SemanticCommandLocalHttpRequestFrame {
	readonly body: unknown;
	readonly method: string;
	readonly path: string;
}

export interface SemanticCommandLocalHttpResponseFrame {
	readonly body: unknown;
	readonly status: number;
}

export type SemanticCommandLocalHttpRequestInspection =
	| { readonly kind: 'incomplete' }
	| { readonly code: SemanticCommandLocalHttpErrorCode; readonly kind: 'rejected' }
	| { readonly frame: SemanticCommandLocalHttpRequestFrame; readonly kind: 'complete' };

interface ParsedHead {
	readonly contentLength: number;
	readonly contentType: string | undefined;
	readonly headBytes: number;
	readonly startLine: string;
}

interface ParsedResponseHead extends ParsedHead {
	readonly status: number;
}

export function inspectSemanticCommandLocalHttpRequest(
	bytes: Uint8Array,
): SemanticCommandLocalHttpRequestInspection {
	const headEnd = findSequence(bytes, headerTerminator);
	if (headEnd < 0) {
		return bytes.byteLength > SEMANTIC_COMMAND_LOCAL_HTTP_MAX_HEADER_BYTES
			? { kind: 'rejected', code: 'request-capacity-exceeded' }
			: { kind: 'incomplete' };
	}
	if (headEnd + headerTerminator.byteLength > SEMANTIC_COMMAND_LOCAL_HTTP_MAX_HEADER_BYTES) {
		return { kind: 'rejected', code: 'request-capacity-exceeded' };
	}
	const parsed = parseHead(bytes, headEnd);
	if (parsed === null) return { kind: 'rejected', code: 'malformed-request' };
	if (parsed.contentLength > SEMANTIC_COMMAND_LOCAL_HTTP_MAX_REQUEST_BODY_BYTES) {
		return { kind: 'rejected', code: 'request-capacity-exceeded' };
	}
	const expectedBytes = parsed.headBytes + parsed.contentLength;
	if (bytes.byteLength < expectedBytes) return { kind: 'incomplete' };
	if (bytes.byteLength !== expectedBytes) {
		return { kind: 'rejected', code: 'malformed-request' };
	}
	const requestLine = /^([A-Z]+) ([^ ]+) HTTP\/1\.1$/u.exec(parsed.startLine);
	if (requestLine === null) return { kind: 'rejected', code: 'malformed-request' };
	const method = requestLine[1];
	const path = requestLine[2];
	if (method === undefined || path === undefined || !requestTargetPattern.test(path)) {
		return { kind: 'rejected', code: 'malformed-request' };
	}
	if (method !== 'GET' && method !== 'POST') {
		return { kind: 'rejected', code: 'method-not-allowed' };
	}
	if (method === 'GET' && parsed.contentLength !== 0) {
		return { kind: 'rejected', code: 'malformed-request' };
	}
	if (method === 'POST' && parsed.contentType !== 'application/json') {
		return { kind: 'rejected', code: 'content-type-required' };
	}
	let body: unknown = null;
	if (parsed.contentLength > 0) {
		try {
			body = JSON.parse(decodeUtf8(bytes.subarray(parsed.headBytes)));
		} catch {
			return { kind: 'rejected', code: 'malformed-request' };
		}
	}
	return { kind: 'complete', frame: { body, method, path } };
}

export function encodeSemanticCommandLocalHttpRequest(
	method: 'GET' | 'POST',
	path: string,
	body?: unknown,
): Uint8Array {
	if (!requestTargetPattern.test(path) || path.includes('#')) {
		throw new Error('Local HTTP request path is invalid.');
	}
	const bodyBytes = method === 'POST' ? encodeJson(body ?? {}) : new Uint8Array();
	if (bodyBytes.byteLength > SEMANTIC_COMMAND_LOCAL_HTTP_MAX_REQUEST_BODY_BYTES) {
		throw new Error('Local HTTP request body exceeds its capacity.');
	}
	const head = [
		`${method} ${path} HTTP/1.1`,
		'Host: localhost',
		'Connection: close',
		...(method === 'POST' ? ['Content-Type: application/json'] : []),
		`Content-Length: ${bodyBytes.byteLength}`,
		'',
		'',
	].join('\r\n');
	return concatenate(encodeUtf8(head), bodyBytes);
}

export function encodeSemanticCommandLocalHttpResponse(status: number, body: unknown): Uint8Array {
	let resolvedStatus = status;
	let bodyBytes = encodeJson(body);
	if (bodyBytes.byteLength > SEMANTIC_COMMAND_LOCAL_HTTP_MAX_RESPONSE_BODY_BYTES) {
		resolvedStatus = 500;
		bodyBytes = encodeJson(localHttpError('response-capacity-exceeded'));
	}
	const reason = responseReason(resolvedStatus);
	const head = [
		`HTTP/1.1 ${resolvedStatus} ${reason}`,
		'Content-Type: application/json',
		'Connection: close',
		`Content-Length: ${bodyBytes.byteLength}`,
		'',
		'',
	].join('\r\n');
	return concatenate(encodeUtf8(head), bodyBytes);
}

export function decodeSemanticCommandLocalHttpResponse(
	bytes: Uint8Array,
): SemanticCommandLocalHttpResponseFrame {
	const headEnd = findSequence(bytes, headerTerminator);
	if (
		headEnd < 0 ||
		headEnd + headerTerminator.byteLength > SEMANTIC_COMMAND_LOCAL_HTTP_MAX_HEADER_BYTES
	) {
		throw new Error('Local HTTP response framing is invalid.');
	}
	const parsed = parseResponseHead(bytes, headEnd);
	if (
		parsed === null ||
		parsed.contentType !== 'application/json' ||
		parsed.contentLength > SEMANTIC_COMMAND_LOCAL_HTTP_MAX_RESPONSE_BODY_BYTES ||
		bytes.byteLength !== parsed.headBytes + parsed.contentLength
	) {
		throw new Error('Local HTTP response framing is invalid.');
	}
	let body: unknown;
	try {
		body = JSON.parse(decodeUtf8(bytes.subarray(parsed.headBytes)));
	} catch {
		throw new Error('Local HTTP response JSON is invalid.');
	}
	return { body, status: parsed.status };
}

export function localHttpError(
	code: SemanticCommandLocalHttpErrorCode,
): SemanticCommandLocalHttpError {
	return {
		protocolVersion: SEMANTIC_COMMAND_PROTOCOL_VERSION,
		kind: 'local-http-error',
		code,
	};
}

function parseResponseHead(bytes: Uint8Array, headEnd: number): ParsedResponseHead | null {
	const parsed = parseHead(bytes, headEnd);
	if (parsed === null) return null;
	const match = /^HTTP\/1\.1 ([1-5][0-9]{2}) [\x20-\x7e]+$/u.exec(parsed.startLine);
	if (match?.[1] === undefined) return null;
	return { ...parsed, status: Number(match[1]) };
}

function parseHead(bytes: Uint8Array, headEnd: number): ParsedHead | null {
	let decoded: string;
	try {
		decoded = decodeUtf8(bytes.subarray(0, headEnd));
	} catch {
		return null;
	}
	const lines = decoded.split('\r\n');
	const startLine = lines.shift();
	if (startLine === undefined || startLine.length === 0 || lines.length > 32) return null;
	const headers = new Map<string, string>();
	for (const line of lines) {
		const separator = line.indexOf(':');
		if (separator <= 0) return null;
		const name = line.slice(0, separator);
		const value = line.slice(separator + 1).trim();
		const normalizedName = name.toLowerCase();
		if (
			!headerNamePattern.test(name) ||
			value.length > 1024 ||
			/[^\x20-\x7e]/u.test(value) ||
			headers.has(normalizedName)
		) {
			return null;
		}
		headers.set(normalizedName, value);
	}
	if (headers.get('connection')?.toLowerCase() !== 'close') return null;
	if (headers.has('transfer-encoding')) return null;
	const contentLengthText = headers.get('content-length');
	if (contentLengthText === undefined || !canonicalDecimalPattern.test(contentLengthText))
		return null;
	const contentLength = Number(contentLengthText);
	if (!Number.isSafeInteger(contentLength)) return null;
	return {
		contentLength,
		contentType: headers.get('content-type')?.toLowerCase(),
		headBytes: headEnd + headerTerminator.byteLength,
		startLine,
	};
}

function findSequence(bytes: Uint8Array, sequence: Uint8Array): number {
	outer: for (let index = 0; index <= bytes.byteLength - sequence.byteLength; index += 1) {
		for (let offset = 0; offset < sequence.byteLength; offset += 1) {
			if (bytes[index + offset] !== sequence[offset]) continue outer;
		}
		return index;
	}
	return -1;
}

function concatenate(left: Uint8Array, right: Uint8Array): Uint8Array {
	const result = new Uint8Array(left.byteLength + right.byteLength);
	result.set(left, 0);
	result.set(right, left.byteLength);
	return result;
}

function encodeJson(value: unknown): Uint8Array {
	const encoded = JSON.stringify(value);
	if (encoded === undefined) throw new Error('Local HTTP body must be JSON serializable.');
	return encodeUtf8(encoded);
}

function responseReason(status: number): string {
	switch (status) {
		case 200:
			return 'OK';
		case 400:
			return 'Bad Request';
		case 404:
			return 'Not Found';
		case 405:
			return 'Method Not Allowed';
		case 408:
			return 'Request Timeout';
		case 413:
			return 'Content Too Large';
		case 500:
			return 'Internal Server Error';
		case 503:
			return 'Service Unavailable';
		default:
			throw new Error('Local HTTP response status is unsupported.');
	}
}

function encodeUtf8(value: string): Uint8Array {
	const bytes: number[] = [];
	for (let index = 0; index < value.length; index += 1) {
		const first = value.charCodeAt(index);
		let codePoint = first;
		if (first >= 0xd800 && first <= 0xdbff) {
			const second = value.charCodeAt(index + 1);
			if (second < 0xdc00 || second > 0xdfff) throw new Error('UTF-8 input is invalid.');
			codePoint = 0x1_0000 + ((first - 0xd800) << 10) + (second - 0xdc00);
			index += 1;
		} else if (first >= 0xdc00 && first <= 0xdfff) {
			throw new Error('UTF-8 input is invalid.');
		}
		if (codePoint <= 0x7f) {
			bytes.push(codePoint);
		} else if (codePoint <= 0x7ff) {
			bytes.push(0xc0 | (codePoint >> 6), 0x80 | (codePoint & 0x3f));
		} else if (codePoint <= 0xffff) {
			bytes.push(
				0xe0 | (codePoint >> 12),
				0x80 | ((codePoint >> 6) & 0x3f),
				0x80 | (codePoint & 0x3f),
			);
		} else {
			bytes.push(
				0xf0 | (codePoint >> 18),
				0x80 | ((codePoint >> 12) & 0x3f),
				0x80 | ((codePoint >> 6) & 0x3f),
				0x80 | (codePoint & 0x3f),
			);
		}
	}
	return Uint8Array.from(bytes);
}

function decodeUtf8(bytes: Uint8Array): string {
	const characters: string[] = [];
	for (let index = 0; index < bytes.byteLength;) {
		const first = requireByte(bytes, index);
		let codePoint: number;
		let continuationCount: number;
		let minimum: number;
		if (first <= 0x7f) {
			codePoint = first;
			continuationCount = 0;
			minimum = 0;
		} else if (first >= 0xc2 && first <= 0xdf) {
			codePoint = first & 0x1f;
			continuationCount = 1;
			minimum = 0x80;
		} else if (first >= 0xe0 && first <= 0xef) {
			codePoint = first & 0x0f;
			continuationCount = 2;
			minimum = 0x800;
		} else if (first >= 0xf0 && first <= 0xf4) {
			codePoint = first & 0x07;
			continuationCount = 3;
			minimum = 0x1_0000;
		} else {
			throw new Error('UTF-8 input is invalid.');
		}
		for (let offset = 1; offset <= continuationCount; offset += 1) {
			const continuation = requireByte(bytes, index + offset);
			if ((continuation & 0xc0) !== 0x80) throw new Error('UTF-8 input is invalid.');
			codePoint = (codePoint << 6) | (continuation & 0x3f);
		}
		if (
			codePoint < minimum ||
			codePoint > 0x10_ffff ||
			(codePoint >= 0xd800 && codePoint <= 0xdfff)
		) {
			throw new Error('UTF-8 input is invalid.');
		}
		characters.push(String.fromCodePoint(codePoint));
		index += continuationCount + 1;
	}
	return characters.join('');
}

function requireByte(bytes: Uint8Array, index: number): number {
	const value = bytes[index];
	if (value === undefined) throw new Error('UTF-8 input is invalid.');
	return value;
}
