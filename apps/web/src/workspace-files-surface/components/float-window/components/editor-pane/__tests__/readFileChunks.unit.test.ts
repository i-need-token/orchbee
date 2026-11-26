import type {
	WorkspaceFilesOperation,
	WorkspaceFilesOutcome,
} from '@orchbee/workbench-client-protocol';
import { describe, expect, it } from 'vitest';

import type { WorkbenchWorkspaceFilesSource } from '../../../../../../workbench';
import { readFileChunks } from '../readFileChunks';

/** A file the fake serves in chunks of `size` characters, at the modification time `modified`. */
interface Served {
	text: string;
	modified: string;
}

function serving(file: Served, size: number, onRead?: (offset: number) => void) {
	const offsets: number[] = [];
	const source: Pick<WorkbenchWorkspaceFilesSource, 'read'> = {
		read(_workspaceId: string, operation: WorkspaceFilesOperation) {
			if (operation.kind !== 'read-file') throw new Error('not a file read');
			offsets.push(operation.offset);
			onRead?.(operation.offset);
			const end = Math.min(operation.offset + size, file.text.length);
			const part = file.text.slice(operation.offset, end);
			const outcome: WorkspaceFilesOutcome = {
				byteLength: file.text.length,
				content:
					operation.encoding === 'text'
						? {
								kind: 'text',
								nextOffset: end < file.text.length ? end : null,
								offset: operation.offset,
								text: part,
							}
						: {
								base64: btoa(part),
								kind: 'bytes',
								nextOffset: end < file.text.length ? end : null,
								offset: operation.offset,
							},
				kind: 'file',
				version: { modified: file.modified, sha256: null },
			};
			return Promise.resolve(outcome);
		},
	};
	return { offsets, source };
}

describe('readFileChunks', () => {
	it('joins every chunk of one version, as text or as bytes', async () => {
		const file = { modified: '1', text: 'abcdefghij' };
		const { offsets, source } = serving(file, 4);
		await expect(readFileChunks(source, 'w', 'a.txt', 'worktree', 'text')).resolves.toMatchObject({
			byteLength: 10,
			kind: 'text',
			text: 'abcdefghij',
		});
		expect(offsets).toEqual([0, 4, 8]);
		const bytes = await readFileChunks(source, 'w', 'a.png', 'worktree', 'bytes');
		expect(bytes.kind === 'bytes' ? [...bytes.bytes] : null).toEqual(
			[...file.text].map((character) => character.charCodeAt(0)),
		);
	});

	it('starts over when the file changes between chunks, so parts of two versions never mix', async () => {
		const file = { modified: '1', text: 'aaaaaaaa' };
		const { offsets, source } = serving(file, 4, (offset) => {
			if (offset === 4 && file.modified === '1')
				Object.assign(file, { modified: '2', text: 'bbbbbbbb' });
		});
		await expect(readFileChunks(source, 'w', 'a.txt', 'worktree', 'text')).resolves.toMatchObject({
			text: 'bbbbbbbb',
		});
		expect(offsets).toEqual([0, 4, 0, 4]);
	});

	it('gives up as unavailable on a file that keeps changing', async () => {
		const file = { modified: '0', text: 'aaaaaaaa' };
		const { source } = serving(file, 4, (offset) => {
			if (offset === 4) file.modified = String(Number(file.modified) + 1);
		});
		await expect(readFileChunks(source, 'w', 'a.txt', 'worktree', 'text')).resolves.toEqual({
			kind: 'stopped',
			outcome: { kind: 'failed', reason: 'unavailable' },
		});
	});
});
