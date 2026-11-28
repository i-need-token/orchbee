// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { act, createElement, type ComponentProps } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

import { CodeEditorTextDocument } from '../../../CodeEditorTextDocument';
import { CodeEditorViewStates } from '../../../CodeEditorViewStates';
import type { editor } from '../../../monaco-entry';
import { CodeEditorDiffView } from '../CodeEditorDiffView';

beforeAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: true,
	});
});

afterAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: false,
	});
});

class FakeModel {
	/** A Set keeps the order listeners were added in, as Monaco's emitters do. */
	private readonly disposeListeners = new Set<() => void>();

	public dispose(): void {
		for (const listener of this.disposeListeners) listener();
	}

	public onWillDispose(listener: () => void): { dispose(): void } {
		const entry = (): void => listener();
		this.disposeListeners.add(entry);
		return {
			dispose: () => {
				this.disposeListeners.delete(entry);
			},
		};
	}
}

interface FakePair {
	readonly modified: FakeModel;
	readonly original: FakeModel;
}

/** Monaco's diff editor reduced to what a view state is, the line scrolled to, and its bug report. */
class FakeDiffEditor {
	/** How often a side was disposed of while the editor still compared it. */
	public bugs = 0;
	public disposed = false;
	public line = 1;
	public model: FakePair | null = null;
	private watches: { dispose(): void }[] = [];

	public dispose(): void {
		this.setModel(null);
		this.disposed = true;
	}

	public restoreViewState(state: { readonly line: number }): void {
		this.line = state.line;
	}

	public saveViewState(): { readonly line: number } | null {
		return this.model === null ? null : { line: this.line };
	}

	public setModel(pair: FakePair | null): void {
		for (const watch of this.watches) watch.dispose();
		this.model = pair;
		this.line = 1;
		// Monaco reports a side disposed of before the editor's model was reset as a bug.
		this.watches =
			pair === null
				? []
				: [pair.original, pair.modified].map((side) =>
						side.onWillDispose(() => {
							this.bugs += 1;
						}),
					);
	}
}

function textDocument(answers: boolean): CodeEditorTextDocument<FakeModel> {
	return new CodeEditorTextDocument<FakeModel>({
		createModel: () => new FakeModel(),
		limit: 10,
		updateModel: () => undefined,
		read: () =>
			answers
				? Promise.resolve({
						kind: 'ok',
						value: {
							bom: false,
							changed: false,
							encoding: 'utf8',
							kind: 'text',
							lineEnding: 'lf',
							text: '',
							version: { mtimeMs: 0, size: 0 },
						},
					})
				: new Promise(() => undefined),
	});
}

function shownModel(document: CodeEditorTextDocument<FakeModel>): FakeModel {
	const state = document.getState();
	if (state.kind !== 'text') throw new Error(`Expected text, got ${state.kind}.`);
	return state.model;
}

describe('CodeEditorDiffView', () => {
	it('moves one diff editor between pairs and lets go of a pair before either side goes', async () => {
		const editors: FakeDiffEditor[] = [];
		const runtime = {
			editor: {
				createDiffEditor: () => {
					const created = new FakeDiffEditor();
					editors.push(created);
					return created;
				},
			},
		} as unknown as ComponentProps<typeof CodeEditorDiffView>['runtime'];
		const viewStates = new CodeEditorViewStates<editor.IDiffEditorViewState>();
		const head = textDocument(true);
		const working = textDocument(true);
		const staged = textDocument(true);
		const index = textDocument(true);
		const loading = textDocument(false);
		await Promise.resolve();
		const root = createRoot(document.createElement('div'));
		const show = async (
			original: CodeEditorTextDocument<FakeModel>,
			modified: CodeEditorTextDocument<FakeModel>,
			key: string,
		): Promise<void> => {
			await act(async () => {
				root.render(
					createElement(
						OrchbeeI18nProvider,
						{ locale: 'en-US' },
						createElement(CodeEditorDiffView, {
							modified: modified as unknown as CodeEditorTextDocument<editor.ITextModel>,
							name: key,
							original: original as unknown as CodeEditorTextDocument<editor.ITextModel>,
							runtime,
							viewStateKey: key,
							viewStates,
						}),
					),
				);
			});
		};

		await show(head, working, 'working');
		const [only] = editors;
		if (only === undefined) throw new Error('The view created no diff editor.');
		only.line = 30;

		await show(staged, index, 'index');
		expect(only.model).toEqual({ modified: shownModel(index), original: shownModel(staged) });
		expect(only.line).toBe(1);
		only.line = 5;

		await show(head, working, 'working');
		expect(only.line).toBe(30);
		only.line = 12;

		// Behind a placeholder the editor still holds its pair, so the owner's disposal must not
		// reach it while it does.
		await show(head, loading, 'loading');
		expect(viewStates.get('working')).toEqual({ line: 12 });
		working.dispose();
		expect(only.model).toBeNull();
		head.dispose();

		await act(async () => root.unmount());
		expect(editors).toHaveLength(1);
		expect(only.disposed).toBe(true);
		expect(only.bugs).toBe(0);
		expect(viewStates.get('index')).toEqual({ line: 5 });
		expect(viewStates.get('working')).toEqual({ line: 12 });
	});
});
