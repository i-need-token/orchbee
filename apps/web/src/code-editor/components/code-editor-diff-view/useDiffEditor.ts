import { useLayoutEffect, useRef, useState, type RefObject } from 'react';

import type { CodeEditorViewStates } from '../../CodeEditorViewStates';
import type { editor } from '../../monaco-entry';

/**
 * The diff editor over Monaco's defaults: read only, without Monaco's context menu, and with
 * VS Code's inline breakpoint and computation timeout pinned. Narrower than 900 pixels it shows the
 * inline view, and a diff that takes longer than 5 seconds to compute shows none.
 */
const DIFF_EDITOR_OPTIONS: editor.IStandaloneDiffEditorConstructionOptions = {
	automaticLayout: true,
	contextmenu: false,
	maxComputationTime: 5000,
	readOnly: true,
	renderSideBySideInlineBreakpoint: 900,
};

/** The two sides a diff compares. */
interface DiffModels {
	readonly modified: editor.ITextModel;
	readonly original: editor.ITextModel;
}

interface DiffEditorOptions {
	readonly host: RefObject<HTMLDivElement | null>;
	/** The sides to compare, or null while the view shows a placeholder instead. */
	readonly models: DiffModels | null;
	readonly runtime: { readonly editor: Pick<typeof editor, 'createDiffEditor'> };
	/** The key the shown diff's view state is kept under. */
	readonly viewStateKey: string;
	readonly viewStates: CodeEditorViewStates<editor.IDiffEditorViewState>;
}

/**
 * One Monaco diff editor in `host` for as long as the view is mounted, moving between pairs of
 * models with `setModel` and keeping each pair's view state, as `useTextEditor` does for one model.
 * Monaco reports a side disposed of while it still compares it as a bug, so the editor watches the
 * sides it holds, even behind a placeholder, and lets go of both before either goes.
 */
export function useDiffEditor({
	host,
	models,
	runtime,
	viewStateKey,
	viewStates,
}: DiffEditorOptions): void {
	const [instance, setInstance] = useState<editor.IStandaloneDiffEditor | null>(null);
	const saveShownReference = useRef<() => void>(() => undefined);
	/** Stops watching the sides the editor holds. */
	const releaseHeldReference = useRef<() => void>(() => undefined);

	useLayoutEffect(() => {
		const element = host.current;
		if (element === null) return undefined;
		const created = runtime.editor.createDiffEditor(element, DIFF_EDITOR_OPTIONS);
		setInstance(created);
		return () => {
			// The shown diff's view state outlives the editor, which lets go of its sides.
			saveShownReference.current();
			releaseHeldReference.current();
			created.dispose();
		};
	}, [host, runtime]);

	const original = models?.original ?? null;
	const modified = models?.modified ?? null;
	useLayoutEffect(() => {
		if (instance === null || original === null || modified === null) return undefined;
		let held = true;
		const saveShown = (): void => {
			const shown = instance.saveViewState();
			if (shown !== null) viewStates.set(viewStateKey, shown);
		};
		// Added before Monaco's own listeners, these run first.
		const letGo = (): void => {
			if (!held) return;
			releaseHeldReference.current();
			saveShown();
			instance.setModel(null);
		};
		releaseHeldReference.current();
		const watches = [original.onWillDispose(letGo), modified.onWillDispose(letGo)];
		releaseHeldReference.current = () => {
			held = false;
			for (const watch of watches) watch.dispose();
			releaseHeldReference.current = () => undefined;
		};
		instance.setModel({ modified, original });
		const saved = viewStates.get(viewStateKey);
		if (saved !== undefined) instance.restoreViewState(saved);
		saveShownReference.current = saveShown;
		return () => {
			saveShownReference.current = () => undefined;
			// The sides stay held, and watched, until other sides or the editor's disposal replace them.
			if (held) saveShown();
		};
	}, [instance, modified, original, viewStateKey, viewStates]);
}
