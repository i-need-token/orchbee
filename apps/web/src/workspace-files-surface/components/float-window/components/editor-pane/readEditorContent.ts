import type { WorkspaceFilesOutcome } from '@orchbee/workbench-client-protocol';

import type { WorkbenchWorkspaceFilesSource } from '../../../../../workbench';
import type { FloatTarget } from '../../../../float-tabs';
import { readWorkspaceFiles } from '../../../../readWorkspaceFiles';

/** Why a tab shows a note instead of text. */
type EditorNote = 'binary' | 'too-large' | 'absent' | 'failed' | 'not-repository';

/** What one tab's sides hold once read: text for each side, or the note to show. */
export type EditorContent =
	| { readonly kind: 'note'; readonly note: EditorNote }
	| { readonly kind: 'file'; readonly text: string; readonly byteLength: number }
	| {
			readonly kind: 'diff';
			readonly before: string;
			readonly after: string;
			readonly byteLength: number;
	  };

const NO_TEXT: WorkspaceFilesOutcome = { byteLength: 0, content: { kind: 'absent' }, kind: 'file' };

function noteFor(outcome: WorkspaceFilesOutcome): EditorNote | null {
	if (outcome.kind === 'failed') {
		return outcome.reason === 'not-repository' ? 'not-repository' : 'failed';
	}
	if (outcome.kind !== 'file') return 'failed';
	if (outcome.content.kind === 'binary') return 'binary';
	if (outcome.content.kind === 'too-large') return 'too-large';
	return null;
}

function textOf(outcome: WorkspaceFilesOutcome): string {
	return outcome.kind === 'file' && outcome.content.kind === 'text' ? outcome.content.text : '';
}

function bytesOf(outcome: WorkspaceFilesOutcome): number {
	return outcome.kind === 'file' ? outcome.byteLength : 0;
}

/**
 * Reads what a tab shows. A file is its working-tree text. A diff reads both sides at once; a side
 * that is absent, as a new or deleted file's is, reads as empty, while a side that cannot show as
 * text makes the whole diff a note.
 */
export async function readEditorContent(
	source: WorkbenchWorkspaceFilesSource,
	workspaceId: string,
	target: FloatTarget,
): Promise<EditorContent> {
	if (target.kind === 'file') {
		const outcome = await readWorkspaceFiles(source, workspaceId, {
			kind: 'read-file',
			path: target.path,
			source: 'worktree',
		});
		const note = noteFor(outcome);
		if (note !== null) return { kind: 'note', note };
		if (outcome.kind === 'file' && outcome.content.kind === 'absent') {
			return { kind: 'note', note: 'absent' };
		}
		return { byteLength: bytesOf(outcome), kind: 'file', text: textOf(outcome) };
	}
	const { item } = target;
	const [before, after] = await Promise.all([
		item.before === null
			? Promise.resolve(NO_TEXT)
			: readWorkspaceFiles(source, workspaceId, {
					kind: 'read-file',
					path: item.beforePath,
					source: item.before,
				}),
		readWorkspaceFiles(source, workspaceId, {
			kind: 'read-file',
			path: item.path,
			source: item.after,
		}),
	]);
	const note = noteFor(after) ?? noteFor(before);
	if (note !== null) return { kind: 'note', note };
	return {
		after: textOf(after),
		before: textOf(before),
		byteLength: Math.max(bytesOf(before), bytesOf(after)),
		kind: 'diff',
	};
}
