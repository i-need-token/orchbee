import { useLayoutEffect, useRef, useState, type RefObject } from 'react';

import type { CodeEditorViewStates } from '../../CodeEditorViewStates';
import type { editor } from '../../monaco-entry';

/**
 * The text editor over Monaco's defaults: read only, and without Monaco's context menu, whose
 * command palette and actions bypass Orchbee's keyboard shortcuts. It starts without a model, so
 * Monaco creates no empty one of its own.
 */
const TEXT_EDITOR_OPTIONS: editor.IStandaloneEditorConstructionOptions = {
	automaticLayout: true,
	contextmenu: false,
	model: null,
	readOnly: true,
};

interface TextEditorOptions {
	readonly host: RefObject<HTMLDivElement | null>;
	/** The model to show, or null while the view shows a placeholder instead. */
	readonly model: editor.ITextModel | null;
	/** The editor's accessible name. */
	readonly name: string;
	readonly runtime: { readonly editor: Pick<typeof editor, 'create'> };
	/** The key the shown file's view state is kept under. */
	readonly viewStateKey: string;
	readonly viewStates: CodeEditorViewStates<editor.ICodeEditorViewState>;
}

/**
 * One Monaco editor in `host` for as long as the view is mounted. It moves between models with
 * `setModel`, saving the outgoing file's view state and restoring the incoming one's, and keeps no
 * model of its own: models belong to their documents, which outlive it. Layout effects attach the
 * incoming model in the frame that shows its tab.
 */
export function useTextEditor({
	host,
	model,
	name,
	runtime,
	viewStateKey,
	viewStates,
}: TextEditorOptions): void {
	const [instance, setInstance] = useState<editor.IStandaloneCodeEditor | null>(null);
	const saveShownReference = useRef<() => void>(() => undefined);

	useLayoutEffect(() => {
		const element = host.current;
		if (element === null) return undefined;
		const created = runtime.editor.create(element, TEXT_EDITOR_OPTIONS);
		setInstance(created);
		return () => {
			// The shown file's view state outlives the editor.
			saveShownReference.current();
			created.dispose();
		};
	}, [host, runtime]);

	useLayoutEffect(() => {
		if (instance === null || model === null) return undefined;
		const saveShown = (): void => {
			// An editor without a model, as a disposed one is, has no view state to save.
			const shown = instance.saveViewState();
			if (shown !== null) viewStates.set(viewStateKey, shown);
		};
		// Monaco detaches a model it sees disposed of; a listener added before that saves first.
		const disposal = model.onWillDispose(saveShown);
		instance.setModel(model);
		const saved = viewStates.get(viewStateKey);
		if (saved !== undefined) instance.restoreViewState(saved);
		saveShownReference.current = saveShown;
		return () => {
			saveShownReference.current = () => undefined;
			disposal.dispose();
			saveShown();
		};
	}, [instance, model, viewStateKey, viewStates]);

	useLayoutEffect(() => {
		instance?.updateOptions({ ariaLabel: name });
	}, [instance, name]);
}
