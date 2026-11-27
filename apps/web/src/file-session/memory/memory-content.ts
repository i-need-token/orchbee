import type { FileContent, FileReadOptions } from '../file-content-port';
import type { FileResult } from '../file-error';

/** Stored content and the facts a read reports about it. */
export interface MemoryContent {
	readonly bom: boolean;
	/** Text, or bytes, which a text read treats as binary when a zero byte comes early. */
	readonly content: string | Uint8Array;
	readonly encoding: string | null;
	readonly mtimeMs: number;
}

interface MemoryVersion {
	readonly mtimeMs: number;
	readonly size: number;
}

/** VS Code's binary probe: a zero byte among the first 512 bytes without a BOM. */
const BINARY_PROBE_BYTES = 512;
const UTF8_BOM_BYTES = 3;
const LINE_FEED = 10;
const CARRIAGE_RETURN = 13;
const decoder = new TextDecoder('utf-8');
const encoder = new TextEncoder();

/** Content's size as the backend measures it, with text counted as UTF-8. */
export function memoryContentSize(content: MemoryContent): number {
	if (typeof content.content !== 'string') return content.content.byteLength;
	return encoder.encode(content.content).byteLength + (content.bom ? UTF8_BOM_BYTES : 0);
}

export function memoryVersion(content: MemoryContent): MemoryVersion {
	return { mtimeMs: content.mtimeMs, size: memoryContentSize(content) };
}

/**
 * VS Code's line ending rule for a new text buffer: CRLF when carriage returns exceed half of all
 * line breaks, counting a lone CR, a lone LF and a CRLF pair as one break each.
 */
function lineEndingOf(text: string): 'crlf' | 'lf' | 'none' {
	let breaks = 0;
	let carriageReturns = 0;
	for (let index = 0; index < text.length; index += 1) {
		const code = text.charCodeAt(index);
		if (code === CARRIAGE_RETURN) {
			breaks += 1;
			carriageReturns += 1;
			if (text.charCodeAt(index + 1) === LINE_FEED) index += 1;
		} else if (code === LINE_FEED) {
			breaks += 1;
		}
	}
	if (breaks === 0) return 'none';
	return carriageReturns > breaks / 2 ? 'crlf' : 'lf';
}

/**
 * Reads stored content as `fs.read` would. `issued` is the version when the request was sent, so
 * content that changed before the answer reports `changed`.
 */
export function readMemoryContent(
	content: MemoryContent,
	options: FileReadOptions,
	issued: MemoryVersion,
): FileResult<FileContent> {
	const version = memoryVersion(content);
	if (version.size > options.limit) return { error: { code: 'too-large' }, kind: 'error' };
	const changed = version.mtimeMs !== issued.mtimeMs || version.size !== issued.size;
	const stored = content.content;
	if (options.mode === 'bytes') {
		const bytes = typeof stored === 'string' ? encoder.encode(stored) : stored;
		return { kind: 'ok', value: { bytes, changed, kind: 'bytes', version } };
	}
	if (
		typeof stored !== 'string' &&
		!content.bom &&
		options.forceText !== true &&
		stored.subarray(0, BINARY_PROBE_BYTES).includes(0)
	) {
		return { kind: 'ok', value: { kind: 'binary', version } };
	}
	const text = typeof stored === 'string' ? stored : decoder.decode(stored);
	// A BOM names the encoding; otherwise the request's encoding applies, as in VS Code.
	const encoding = content.bom
		? (content.encoding ?? 'utf8')
		: (options.encoding ?? content.encoding ?? 'utf8');
	return {
		kind: 'ok',
		value: {
			bom: content.bom,
			changed,
			encoding,
			kind: 'text',
			lineEnding: lineEndingOf(text),
			text,
			version,
		},
	};
}
