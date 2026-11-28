import { describe, expect, it, vi } from 'vitest';

import type { FileContent, FileResult } from '../../file-session';
import { CodeEditorTextDocument } from '../CodeEditorTextDocument';

class FakeModel {
	public disposals = 0;
	public text: string;

	public constructor(content: { readonly text: string }) {
		this.text = content.text;
	}

	public dispose(): void {
		this.disposals += 1;
	}
}

const VERSION = { mtimeMs: 1, size: 5 };
function textResult(text: string): FileResult<FileContent> {
	return {
		kind: 'ok',
		value: {
			bom: false,
			changed: false,
			encoding: 'utf8',
			kind: 'text',
			lineEnding: 'lf',
			text,
			version: VERSION,
		},
	};
}

const TEXT = textResult('hello');
const BINARY: FileResult<FileContent> = { kind: 'ok', value: { kind: 'binary', version: VERSION } };
const TOO_LARGE: FileResult<FileContent> = { error: { code: 'too-large' }, kind: 'error' };

interface PendingRead {
	readonly options: { readonly forceText: boolean; readonly limit: number };
	readonly resolve: (result: FileResult<FileContent>) => void;
	readonly signal: AbortSignal;
}

/** A document over a read whose answers the test gives, one pending read at a time. */
function harness() {
	const reads: PendingRead[] = [];
	const createModel = vi.fn<(content: { readonly text: string }) => FakeModel>(
		(content) => new FakeModel(content),
	);
	const updateModel = vi.fn<(model: FakeModel, content: { readonly text: string }) => void>(
		(model, content) => {
			model.text = content.text;
		},
	);
	const document = new CodeEditorTextDocument<FakeModel>({
		createModel,
		limit: 10,
		read: (options, signal) =>
			new Promise((resolve) => {
				reads.push({ options, resolve, signal });
			}),
		updateModel,
	});
	return {
		async answer(result: FileResult<FileContent>): Promise<PendingRead> {
			const read = reads.at(-1);
			if (read === undefined) throw new Error('No read is pending.');
			read.resolve(result);
			// The document settles in the microtask after the answer.
			await Promise.resolve();
			return read;
		},
		createModel,
		document,
		reads,
		updateModel,
	};
}

describe('CodeEditorTextDocument', () => {
	it('abandons a read on disposal and creates no model from its late answer', async () => {
		const { answer, createModel, document, reads } = harness();
		document.dispose();
		expect(reads[0]?.signal.aborted).toBe(true);

		await answer(TEXT);
		expect(createModel).not.toHaveBeenCalled();
	});

	it('disposes of the model it created when it is disposed', async () => {
		const { answer, document } = harness();
		await answer(TEXT);
		const state = document.getState();
		if (state.kind !== 'text') throw new Error(`Expected text, got ${state.kind}.`);

		document.dispose();
		document.dispose();
		expect(state.model.disposals).toBe(1);
	});

	it('lifts a refusal only through Open Anyway, and the ceiling only once', async () => {
		const { answer, document, reads } = harness();
		await answer(BINARY);
		expect(document.getState()).toEqual({ kind: 'binary' });

		document.openAnyway();
		expect(reads.at(-1)?.options).toEqual({ forceText: true, limit: 10 });
		await answer(TOO_LARGE);
		expect(document.getState()).toEqual({ kind: 'too-large', lifted: false });

		document.openAnyway();
		expect(reads.at(-1)?.options.limit).toBeGreaterThan(10);
		await answer(TOO_LARGE);
		expect(document.getState()).toEqual({ kind: 'too-large', lifted: true });

		document.openAnyway();
		expect(reads).toHaveLength(3);
	});

	it('takes a reload into the shown model past every refusal, and keeps the text when it fails', async () => {
		const { answer, createModel, document, reads, updateModel } = harness();
		await answer(TEXT);
		const shown = document.getState();
		if (shown.kind !== 'text') throw new Error(`Expected text, got ${shown.kind}.`);

		document.reload();
		expect(document.getState()).toBe(shown);
		expect(reads.at(-1)?.options.forceText).toBe(true);
		expect(reads.at(-1)?.options.limit).toBeGreaterThan(10);
		await answer(textResult('world'));
		const reloaded = document.getState();
		expect(reloaded.kind === 'text' ? reloaded.model : null).toBe(shown.model);
		expect(updateModel).toHaveBeenCalledOnce();
		expect(shown.model.text).toBe('world');
		expect(createModel).toHaveBeenCalledOnce();

		document.reload();
		await answer({ error: { code: 'not-found' }, kind: 'error' });
		expect(document.getState()).toBe(reloaded);
	});
});
