import type {
	WorkspaceFilesOutcome,
	WorkspaceFileVersion,
} from '@orchbee/workbench-client-protocol';
import type * as MonacoApi from 'monaco-editor/editor/editor.api';
import { describe, expect, it } from 'vitest';

import type { WorkbenchWorkspaceFilesSource } from '../../workbench';
import { type DocumentModel, EditorDocuments } from '../EditorDocuments';

// The holder decides which disk reads may replace text a person is editing and what a save sends.
// No browser runs here to exercise the editor, so these rules are kept as evidence.

const WORKSPACE = 'w';
const PATH = 'a.txt';

function version(sha: string | null, modified = '1'): WorkspaceFileVersion {
	return { modified, sha256: sha === null ? null : sha.padEnd(64, '0') };
}

function splitsPair(text: string, offset: number): boolean {
	return (
		/[\uD800-\uDBFF]/u.test(text.charAt(offset - 1)) && /[\uDC00-\uDFFF]/u.test(text.charAt(offset))
	);
}

/** A text model reduced to what the holder uses; `type` is the person editing. */
class FakeModel implements DocumentModel {
	#value: string;
	#version = 1;
	readonly #listeners = new Set<() => void>();
	disposed = false;

	constructor(text: string) {
		this.#value = text;
	}

	getValue(): string {
		return this.#value;
	}

	getAlternativeVersionId(): number {
		return this.#version;
	}

	getPositionAt(offset: number): MonacoApi.IPosition {
		const before = this.#value.slice(0, offset).split('\n');
		return { column: (before.at(-1)?.length ?? 0) + 1, lineNumber: before.length };
	}

	onDidChangeContent(listener: () => void): MonacoApi.IDisposable {
		this.#listeners.add(listener);
		return { dispose: () => this.#listeners.delete(listener) };
	}

	pushEditOperations(
		_before: MonacoApi.Selection[] | null,
		operations: MonacoApi.editor.IIdentifiedSingleEditOperation[],
	): null {
		for (const { range, text } of operations) {
			// Monaco widens a range that splits a surrogate pair to the whole pair.
			let start = this.#offsetAt(range.startLineNumber, range.startColumn);
			let end = this.#offsetAt(range.endLineNumber, range.endColumn);
			if (splitsPair(this.#value, start)) start -= 1;
			if (splitsPair(this.#value, end)) end += 1;
			this.#set(this.#value.slice(0, start) + (text ?? '') + this.#value.slice(end));
		}
		return null;
	}

	pushStackElement(): void {}

	dispose(): void {
		this.disposed = true;
	}

	type(text: string): void {
		this.#set(text);
	}

	#offsetAt(lineNumber: number, column: number): number {
		const lines = this.#value.split('\n').slice(0, lineNumber - 1);
		return lines.reduce((total, line) => total + line.length + 1, 0) + column - 1;
	}

	#set(text: string): void {
		this.#value = text;
		this.#version += 1;
		for (const listener of this.#listeners) listener();
	}
}

interface Sent {
	readonly text: string;
	readonly expected: string | null;
	readonly answer: (outcome: WorkspaceFilesOutcome | null) => void;
}

/** A save source whose answers the test gives, one save at a time. */
function saving(): {
	readonly source: Pick<WorkbenchWorkspaceFilesSource, 'save'>;
	readonly sent: Sent[];
} {
	const sent: Sent[] = [];
	return {
		sent,
		source: {
			save: (_workspaceId, _path, text, expected) =>
				new Promise((answer) => {
					sent.push({ answer, expected, text });
				}),
		},
	};
}

function opened(text = 'one'): {
	readonly documents: EditorDocuments<FakeModel>;
	readonly model: FakeModel;
} {
	const documents = new EditorDocuments<FakeModel>();
	const model = documents.adopt(WORKSPACE, PATH, text, version('a'), documents.now(), (initial) => {
		return new FakeModel(initial);
	});
	return { documents, model };
}

describe('EditorDocuments', () => {
	it('lets a newer read replace clean text and keeps edits as a conflict', () => {
		const { documents, model } = opened();
		documents.adopt(WORKSPACE, PATH, 'two', version('b'), documents.now(), () => model);
		expect(model.getValue()).toBe('two');
		expect(documents.state(WORKSPACE, PATH)).toMatchObject({ dirty: false, notice: null });

		model.type('mine');
		expect(documents.dirtyPaths(WORKSPACE)).toEqual(new Set([PATH]));
		documents.adopt(WORKSPACE, PATH, 'two', version('b'), documents.now(), () => model);
		expect(documents.state(WORKSPACE, PATH)?.notice).toBeNull();
		documents.adopt(WORKSPACE, PATH, 'three', version('c'), documents.now(), () => model);
		expect(model.getValue()).toBe('mine');
		expect(documents.state(WORKSPACE, PATH)?.notice).toEqual({
			kind: 'conflict',
			version: version('c'),
		});
	});

	it('takes a newer read without splitting a character the old and new text share half of', () => {
		const { documents, model } = opened('a😀b');
		documents.adopt(WORKSPACE, PATH, 'a😁b', version('b'), documents.now(), () => model);
		expect(model.getValue()).toBe('a😁b');
	});

	it('never lets a read older than the one taken replace it, as another tab’s may be', () => {
		const { documents, model } = opened();
		const older = documents.now();
		documents.adopt(WORKSPACE, PATH, 'two', version('b'), documents.now(), () => model);
		documents.adopt(WORKSPACE, PATH, 'one', version('a'), older, () => model);
		expect(model.getValue()).toBe('two');
		model.type('mine');
		documents.adopt(WORKSPACE, PATH, 'one', version('a'), older, () => model);
		expect(documents.state(WORKSPACE, PATH)?.notice).toBeNull();
	});

	it('saves over the version read, and edits typed meanwhile stay unsaved', async () => {
		const { documents, model } = opened();
		const { sent, source } = saving();
		model.type('mine');
		const saved = documents.save(source, WORKSPACE, PATH);
		expect(sent).toMatchObject([{ expected: version('a').sha256, text: 'mine' }]);
		expect(documents.state(WORKSPACE, PATH)?.saving).toBe(true);
		model.type('mine, more');
		sent[0]?.answer({ kind: 'written', version: version('m', '2') });
		await expect(saved).resolves.toBe('written');
		expect(documents.state(WORKSPACE, PATH)).toMatchObject({ dirty: true, saving: false });

		const again = documents.save(source, WORKSPACE, PATH);
		expect(sent[1]?.expected).toBe(version('m').sha256);
		sent[1]?.answer({ kind: 'written', version: version('n', '3') });
		await expect(again).resolves.toBe('written');
		expect(documents.dirtyPaths(WORKSPACE).size).toBe(0);
	});

	it('drops a read that started before a save', async () => {
		const { documents, model } = opened();
		const { sent, source } = saving();
		model.type('mine');
		const readAt = documents.now();
		const saved = documents.save(source, WORKSPACE, PATH);
		sent[0]?.answer({ kind: 'written', version: version('m', '2') });
		await saved;
		documents.adopt(WORKSPACE, PATH, 'one', version('a'), readAt, () => model);
		expect(model.getValue()).toBe('mine');
		expect(documents.state(WORKSPACE, PATH)?.notice).toBeNull();
	});

	it('reports a conflict, and overwrites only the version the person chose', async () => {
		const { documents, model } = opened();
		const { sent, source } = saving();
		model.type('mine');
		const saved = documents.save(source, WORKSPACE, PATH);
		sent[0]?.answer({ kind: 'conflict', version: version('d', '2') });
		await expect(saved).resolves.toBe('conflict');
		expect(documents.state(WORKSPACE, PATH)?.notice).toEqual({
			kind: 'conflict',
			version: version('d', '2'),
		});

		const overwritten = documents.save(source, WORKSPACE, PATH, {
			expected: version('d').sha256,
		});
		expect(sent[1]?.expected).toBe(version('d').sha256);
		sent[1]?.answer({ kind: 'written', version: version('m', '3') });
		await overwritten;
		expect(documents.state(WORKSPACE, PATH)).toMatchObject({ dirty: false, notice: null });
	});

	it('treats an unanswered save as unconfirmed until a read shows the edits on disk', async () => {
		const { documents, model } = opened();
		const { sent, source } = saving();
		model.type('mine');
		const saved = documents.save(source, WORKSPACE, PATH);
		sent[0]?.answer(null);
		await expect(saved).resolves.toBe('failed');
		expect(documents.state(WORKSPACE, PATH)?.notice).toEqual({
			kind: 'failed',
			reason: 'unconfirmed',
		});
		documents.adopt(WORKSPACE, PATH, 'mine', version('m', '2'), documents.now(), () => model);
		expect(documents.state(WORKSPACE, PATH)).toMatchObject({ dirty: false, notice: null });
	});

	it('keeps edits of a file gone from disk, and releases what no tab shows', async () => {
		const { documents, model } = opened();
		const { sent, source } = saving();
		model.type('mine');
		expect(documents.gone(WORKSPACE, PATH, documents.now())).toBe(model);
		expect(documents.state(WORKSPACE, PATH)?.notice).toEqual({ kind: 'conflict', version: null });

		const saved = documents.save(source, WORKSPACE, PATH);
		documents.retain(new Map());
		expect(model.disposed).toBe(true);
		sent[0]?.answer({ kind: 'written', version: version('m') });
		await expect(saved).resolves.toBe('skipped');
		expect(documents.state(WORKSPACE, PATH)).toBeNull();
	});

	it('refuses text larger than a save carries without sending it', async () => {
		const { documents, model } = opened();
		const { sent, source } = saving();
		model.type('x'.repeat(10 * 1024 * 1024 + 1));
		await expect(documents.save(source, WORKSPACE, PATH)).resolves.toBe('failed');
		expect(sent).toHaveLength(0);
		expect(documents.state(WORKSPACE, PATH)?.notice).toEqual({ kind: 'too-large' });
	});
});
