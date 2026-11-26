import type {
	WorkspaceFileSource,
	WorkspaceFilesOutcome,
} from '@orchbee/workbench-client-protocol';

import type { WorkbenchWorkspaceFilesSource } from '../../../../../workbench';
import type { FloatTarget } from '../../../../float-tabs';
import { readFileChunks, type WholeFile } from './readFileChunks';

/** The largest file the Server opens as text, as `workbench.workspace-files` sets it. */
const MAXIMUM_TEXT_BYTES = 50 * 1024 * 1024;
/** The largest image, and the largest index or `HEAD` side of a diff, the Server reads. */
const MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES = 10 * 1024 * 1024;

/** Images the pane shows as pictures, by extension, with the type they load as. */
const IMAGE_TYPES: Readonly<Record<string, string>> = {
	gif: 'image/gif',
	jpeg: 'image/jpeg',
	jpg: 'image/jpeg',
	png: 'image/png',
	svg: 'image/svg+xml',
	webp: 'image/webp',
};

/** Why a tab shows a note instead of its content, with the size the note names. */
type EditorNote =
	| { readonly note: 'binary'; readonly byteLength: number; readonly type: string | null }
	| { readonly note: 'too-large'; readonly byteLength: number; readonly limit: number }
	| { readonly note: 'absent' | 'failed' | 'not-repository' };

/** What one tab holds once read: a file's text, an image, both sides of a diff, or a note. */
export type EditorContent =
	| ({ readonly kind: 'note' } & EditorNote)
	| { readonly kind: 'file'; readonly text: string; readonly byteLength: number }
	| {
			readonly kind: 'image';
			readonly url: string;
			readonly type: string;
			readonly byteLength: number;
	  }
	| {
			readonly kind: 'diff';
			readonly before: string;
			readonly after: string;
			readonly byteLength: number;
	  };

const NO_TEXT: WholeFile = { byteLength: 0, kind: 'text', text: '', version: null };
const NO_NOTE: EditorContent = { kind: 'note', note: 'failed' };

function extensionOf(path: string): string | null {
	const name = path.slice(path.lastIndexOf('/') + 1);
	const dot = name.lastIndexOf('.');
	return dot > 0 ? name.slice(dot + 1).toLowerCase() : null;
}

function noteFor(
	outcome: WorkspaceFilesOutcome,
	path: string,
	limit: number,
): ({ readonly kind: 'note' } & EditorNote) | null {
	if (outcome.kind === 'failed') {
		return {
			kind: 'note',
			note: outcome.reason === 'not-repository' ? 'not-repository' : 'failed',
		};
	}
	if (outcome.kind !== 'file') return { kind: 'note', note: 'failed' };
	switch (outcome.content.kind) {
		case 'binary':
			return {
				byteLength: outcome.byteLength,
				kind: 'note',
				note: 'binary',
				type: extensionOf(path)?.toUpperCase() ?? null,
			};
		case 'too-large':
			return { byteLength: outcome.byteLength, kind: 'note', limit, note: 'too-large' };
		case 'absent':
			return { kind: 'note', note: 'absent' };
		case 'text':
		case 'bytes':
			return null;
	}
}

/** Standard base64 of any length, in pieces small enough to pass as arguments. */
function encodeBase64(bytes: Uint8Array): string {
	const pieces: string[] = [];
	for (let at = 0; at < bytes.length; at += 0x8000) {
		pieces.push(String.fromCharCode(...bytes.subarray(at, at + 0x8000)));
	}
	return btoa(pieces.join(''));
}

/** A diff side's text, empty for a side that does not exist, or the note that stops the diff. */
function sideOf(
	whole: WholeFile,
	path: string,
	limit: number,
): { readonly kind: 'side'; readonly text: string; readonly byteLength: number } | EditorContent {
	if (whole.kind === 'text')
		return { byteLength: whole.byteLength, kind: 'side', text: whole.text };
	if (whole.kind === 'bytes') return NO_NOTE;
	if (whole.outcome.kind === 'file' && whole.outcome.content.kind === 'absent') {
		return { byteLength: 0, kind: 'side', text: '' };
	}
	return noteFor(whole.outcome, path, limit) ?? NO_NOTE;
}

/** The largest file a side of a diff reads: Git holds its sides up to the editable size. */
function limitOf(side: WorkspaceFileSource): number {
	return side === 'worktree' ? MAXIMUM_TEXT_BYTES : MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES;
}

/**
 * Reads what a tab shows, chunk by chunk. A file is its working-tree text, or the picture of an
 * image. A diff reads both sides at once; a side that is absent, as a new or deleted file's is,
 * reads as empty, while a side that cannot show as text makes the whole diff a note.
 */
export async function readEditorContent(
	source: WorkbenchWorkspaceFilesSource,
	workspaceId: string,
	target: FloatTarget,
): Promise<EditorContent> {
	if (target.kind === 'file') {
		const { path } = target;
		const imageType = IMAGE_TYPES[extensionOf(path) ?? ''];
		if (imageType !== undefined) {
			const whole = await readFileChunks(source, workspaceId, path, 'worktree', 'bytes');
			if (whole.kind === 'bytes') {
				return {
					byteLength: whole.byteLength,
					kind: 'image',
					type: imageType,
					url: `data:${imageType};base64,${encodeBase64(whole.bytes)}`,
				};
			}
			if (whole.kind === 'stopped') {
				return noteFor(whole.outcome, path, MAXIMUM_WORKSPACE_FILE_EDITABLE_BYTES) ?? NO_NOTE;
			}
			return NO_NOTE;
		}
		const whole = await readFileChunks(source, workspaceId, path, 'worktree', 'text');
		if (whole.kind === 'text') {
			return { byteLength: whole.byteLength, kind: 'file', text: whole.text };
		}
		if (whole.kind === 'stopped')
			return noteFor(whole.outcome, path, MAXIMUM_TEXT_BYTES) ?? NO_NOTE;
		return NO_NOTE;
	}
	const { item } = target;
	const [before, after] = await Promise.all([
		item.before === null
			? Promise.resolve(NO_TEXT)
			: readFileChunks(source, workspaceId, item.beforePath, item.before, 'text'),
		readFileChunks(source, workspaceId, item.path, item.after, 'text'),
	]);
	const afterSide = sideOf(after, item.path, limitOf(item.after));
	if (afterSide.kind !== 'side') return afterSide;
	const beforeSide = sideOf(before, item.beforePath, limitOf(item.before ?? 'worktree'));
	if (beforeSide.kind !== 'side') return beforeSide;
	return {
		after: afterSide.text,
		before: beforeSide.text,
		byteLength: Math.max(beforeSide.byteLength, afterSide.byteLength),
		kind: 'diff',
	};
}
