// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { act, createElement, type ComponentProps } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it } from 'vitest';

import { CodeEditorTextDocument } from '../../../CodeEditorTextDocument';
import { CodeEditorViewStates } from '../../../CodeEditorViewStates';
import type { editor } from '../../../monaco-entry';
import { CodeEditorTextView } from '../CodeEditorTextView';

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
	public disposals = 0;
	/** A Set keeps the order listeners were added in, as Monaco's emitters do. */
	private readonly disposeListeners = new Set<() => void>();

	public dispose(): void {
		for (const listener of this.disposeListeners) listener();
		this.disposals += 1;
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

/** Monaco's editor reduced to what a view state is: here, the line scrolled to. */
class FakeEditor {
	public disposed = false;
	public line = 1;
	public model: FakeModel | null = null;
	private detach: { dispose(): void } | null = null;

	public dispose(): void {
		this.detach?.dispose();
		this.disposed = true;
		this.model = null;
	}

	public onContextMenu(): { dispose(): void } {
		return { dispose: () => undefined };
	}

	public restoreViewState(state: { readonly line: number }): void {
		this.line = state.line;
	}

	public saveViewState(): { readonly line: number } | null {
		return this.model === null ? null : { line: this.line };
	}

	public setModel(model: FakeModel): void {
		this.detach?.dispose();
		this.model = model;
		this.line = 1;
		// Monaco detaches a model it sees disposed of, and listens only while it is attached.
		this.detach = model.onWillDispose(() => {
			this.model = null;
		});
	}

	public updateOptions(): void {}
}

async function textDocument(): Promise<CodeEditorTextDocument<FakeModel>> {
	const document = new CodeEditorTextDocument<FakeModel>({
		createModel: () => new FakeModel(),
		limit: 10,
		updateModel: () => undefined,
		read: () =>
			Promise.resolve({
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
			}),
	});
	await Promise.resolve();
	return document;
}

function shownModel(document: CodeEditorTextDocument<FakeModel>): FakeModel {
	const state = document.getState();
	if (state.kind !== 'text') throw new Error(`Expected text, got ${state.kind}.`);
	return state.model;
}

describe('CodeEditorTextView', () => {
	it('moves one editor between files and returns each to where it was left', async () => {
		const editors: FakeEditor[] = [];
		const runtime = {
			editor: {
				MouseTargetType: {},
				create: () => {
					const created = new FakeEditor();
					editors.push(created);
					return created;
				},
			},
		} as unknown as ComponentProps<typeof CodeEditorTextView>['runtime'];
		const viewStates = new CodeEditorViewStates<editor.ICodeEditorViewState>();
		const first = await textDocument();
		const second = await textDocument();
		const root = createRoot(document.createElement('div'));
		const show = async (shown: CodeEditorTextDocument<FakeModel>, key: string): Promise<void> => {
			await act(async () => {
				root.render(
					createElement(
						OrchbeeI18nProvider,
						{ locale: 'en-US' },
						createElement(CodeEditorTextView, {
							document: shown as unknown as CodeEditorTextDocument<editor.ITextModel>,
							name: key,
							runtime,
							viewStateKey: key,
							viewStates,
						}),
					),
				);
			});
		};

		await show(first, 'first');
		const [only] = editors;
		if (only === undefined) throw new Error('The view created no editor.');
		only.line = 40;

		await show(second, 'second');
		expect(only.model).toBe(shownModel(second));
		expect(only.line).toBe(1);
		only.line = 7;

		await show(first, 'first');
		expect(only.model).toBe(shownModel(first));
		expect(only.line).toBe(40);
		only.line = 12;

		// The tab's owner may dispose of the document before the view unmounts.
		const firstModel = shownModel(first);
		first.dispose();
		expect(only.model).toBeNull();
		expect(viewStates.get('first')).toEqual({ line: 12 });

		await act(async () => root.unmount());
		expect(editors).toHaveLength(1);
		expect(only.disposed).toBe(true);
		expect(viewStates.get('first')).toEqual({ line: 12 });
		expect(viewStates.get('second')).toEqual({ line: 7 });
		expect(firstModel.disposals).toBe(1);
		expect(shownModel(second).disposals).toBe(0);
	});
});
