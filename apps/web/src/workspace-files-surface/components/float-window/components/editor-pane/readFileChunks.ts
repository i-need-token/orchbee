import type {
	WorkspaceFileEncoding,
	WorkspaceFileSource,
	WorkspaceFilesOutcome,
	WorkspaceFileVersion,
} from '@orchbee/workbench-client-protocol';

import type { WorkbenchWorkspaceFilesSource } from '../../../../../workbench';
import { readWorkspaceFiles } from '../../../../readWorkspaceFiles';

/** A whole file joined from its chunks, or the outcome that stopped the read. */
export type WholeFile =
	| {
			readonly kind: 'text';
			readonly text: string;
			readonly byteLength: number;
			readonly version: WorkspaceFileVersion | null;
	  }
	| {
			readonly kind: 'bytes';
			readonly bytes: Uint8Array;
			readonly byteLength: number;
			readonly version: WorkspaceFileVersion | null;
	  }
	| { readonly kind: 'stopped'; readonly outcome: WorkspaceFilesOutcome };

/** How often a read starts over when the file changes under it before it gives up. */
const ATTEMPTS = 3;

const CHANGED: WholeFile = { kind: 'stopped', outcome: { kind: 'failed', reason: 'unavailable' } };

function decodeBase64(encoded: string): Uint8Array {
	const binary = atob(encoded);
	const bytes = new Uint8Array(binary.length);
	for (let index = 0; index < binary.length; index += 1) bytes[index] = binary.charCodeAt(index);
	return bytes;
}

function join(parts: readonly Uint8Array[], length: number): Uint8Array {
	const bytes = new Uint8Array(length);
	let at = 0;
	for (const part of parts) {
		bytes.set(part, at);
		at += part.length;
	}
	return bytes;
}

/**
 * Reads one try of a whole file, following each chunk's next offset. Null means the file changed
 * between chunks: its size, or a working-tree file's modification time, is not what the first
 * chunk said.
 */
async function readOnce(
	source: Pick<WorkbenchWorkspaceFilesSource, 'read'>,
	workspaceId: string,
	path: string,
	fileSource: WorkspaceFileSource,
	encoding: WorkspaceFileEncoding,
): Promise<WholeFile | null> {
	const texts: string[] = [];
	const parts: Uint8Array[] = [];
	let first: { readonly byteLength: number; readonly version: WorkspaceFileVersion | null } | null =
		null;
	let offset: number | null = 0;
	while (offset !== null) {
		const outcome = await readWorkspaceFiles(source, workspaceId, {
			encoding,
			kind: 'read-file',
			offset,
			path,
			source: fileSource,
		});
		if (outcome.kind !== 'file') return { kind: 'stopped', outcome };
		if (first === null) first = { byteLength: outcome.byteLength, version: outcome.version };
		else if (
			outcome.byteLength !== first.byteLength ||
			outcome.version?.modified !== first.version?.modified
		) {
			return null;
		}
		const { content } = outcome;
		if (content.kind === 'text' && content.offset === offset) {
			texts.push(content.text);
			offset = content.nextOffset;
		} else if (content.kind === 'bytes' && content.offset === offset) {
			parts.push(decodeBase64(content.base64));
			offset = content.nextOffset;
		} else if (offset === 0) {
			// Binary, too large or absent: the first chunk already says what the file is.
			return { kind: 'stopped', outcome };
		} else {
			// A file that turns binary or too large partway through changed under the read.
			return null;
		}
	}
	if (first === null) return CHANGED;
	return encoding === 'text'
		? { byteLength: first.byteLength, kind: 'text', text: texts.join(''), version: first.version }
		: {
				byteLength: first.byteLength,
				bytes: join(parts, first.byteLength),
				kind: 'bytes',
				version: first.version,
			};
}

/**
 * Reads a whole file chunk by chunk, starting over when it changes between chunks, so the parts
 * always belong to one version of it.
 */
export async function readFileChunks(
	source: Pick<WorkbenchWorkspaceFilesSource, 'read'>,
	workspaceId: string,
	path: string,
	fileSource: WorkspaceFileSource,
	encoding: WorkspaceFileEncoding,
): Promise<WholeFile> {
	for (let attempt = 0; attempt < ATTEMPTS; attempt += 1) {
		const whole = await readOnce(source, workspaceId, path, fileSource, encoding);
		if (whole !== null) return whole;
	}
	return CHANGED;
}
