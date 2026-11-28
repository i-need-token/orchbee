import { useMemoizedFn } from '@orchbee/foundation/react';
import { useLayoutEffect, useRef, useState, type RefObject } from 'react';

import type { CodeEditorTextHandle } from '../../code-editor-text-handle';
import type { CodeEditorViewStates } from '../../CodeEditorViewStates';
import type { editor } from '../../monaco-entry';

/**
 * The text editor over Monaco's defaults: read only, without word-based suggestions, and without
 * Monaco's context menu, whose command palette and actions bypass Orchbee's keyboard shortcuts; the
 * view's owner shows its own. It starts without a model, so Monaco creates no empty one of its own.
 */
const TEXT_EDITOR_OPTIONS: editor.IStandaloneEditorConstructionOptions = {
	automaticLayout: true,
	contextmenu: false,
	model: null,
	readOnly: true,
	wordBasedSuggestions: 'off',
};

/** What the hook needs of Monaco, from what `loadCodeEditor` resolves to. */
export interface TextEditorRuntime {
	readonly editor: Pick<typeof editor, 'MouseTargetType' | 'create'>;
}

interface TextEditorOptions {
	readonly host: RefObject<HTMLDivElement | null>;
	/** The model to show, or null while the view shows a placeholder instead. */
	readonly model: editor.ITextModel | null;
	/** The editor's accessible name. */
	readonly name: string;
	/** Receives the editor's handle once it exists, and null as it goes. */
	readonly onHandle: ((handle: CodeEditorTextHandle | null) => void) | undefined;
	readonly runtime: TextEditorRuntime;
	/** The key the shown file's view state is kept under. */
	readonly viewStateKey: string;
	readonly viewStates: CodeEditorViewStates<editor.ICodeEditorViewState>;
}

/**
 * One Monaco editor in `host` for as long as the view is mounted. It moves between models with
 * `setModel`, saving the outgoing file's view state and restoring the incoming one's, and keeps no
 * model of its own: models belong to their documents, which outlive it. Layout effects attach the
 * incoming model in the frame that shows its tab.
 *
 * A right click reaches the view's owner, as a `contextmenu` event, only where VS Code shows its
 * editor menu: on the text, or from the keys. The editor takes the keys first and the cursor moves
 * to the click unless it lands in a selection. A field in a widget such as Find keeps the native
 * menu, and nothing shows elsewhere, as on the scrollbar or in the gutter.
 */
export function useTextEditor({
	host,
	model,
	name,
	onHandle,
	runtime,
	viewStateKey,
	viewStates,
}: TextEditorOptions): void {
	const [instance, setInstance] = useState<editor.IStandaloneCodeEditor | null>(null);
	const saveShownReference = useRef<() => void>(() => undefined);
	const announce = useMemoizedFn((handle: CodeEditorTextHandle | null): void => onHandle?.(handle));

	useLayoutEffect(() => {
		const element = host.current;
		if (element === null) return undefined;
		const created = runtime.editor.create(element, TEXT_EDITOR_OPTIONS);
		const { MouseTargetType } = runtime.editor;
		const contextMenu = created.onContextMenu(({ event, target }) => {
			if (target.type === MouseTargetType.OVERLAY_WIDGET) {
				event.stopPropagation();
				return;
			}
			if (
				target.type !== MouseTargetType.CONTENT_TEXT &&
				target.type !== MouseTargetType.CONTENT_EMPTY &&
				target.type !== MouseTargetType.TEXTAREA
			) {
				event.preventDefault();
				event.stopPropagation();
				return;
			}
			created.focus();
			const selection = created.getSelection();
			if (target.position !== null && selection?.containsPosition(target.position) === false) {
				created.setPosition(target.position);
			}
		});
		setInstance(created);
		announce(textHandle(created));
		return () => {
			announce(null);
			// The shown file's view state outlives the editor.
			saveShownReference.current();
			contextMenu.dispose();
			created.dispose();
		};
	}, [announce, host, runtime]);

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

/** The editor as its view's owner may use it. */
function textHandle(instance: editor.IStandaloneCodeEditor): CodeEditorTextHandle {
	return {
		contains: (node) => instance.getContainerDomNode().contains(node),
		focus: () => instance.focus(),
		run: (id) => instance.trigger('orchbee', id, null),
		supportedActions: () =>
			instance.getSupportedActions().map(({ alias, id, label }) => ({ alias, id, label })),
		supports: (id) => instance.getAction(id)?.isSupported() === true,
		writable: () => instance.getRawOptions().readOnly !== true,
	};
}
