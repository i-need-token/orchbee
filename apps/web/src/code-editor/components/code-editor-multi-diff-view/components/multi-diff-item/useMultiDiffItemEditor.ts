import { useLayoutEffect, useState, type RefObject } from 'react';

import type { editor } from '../../../../monaco-entry';
import type { MultiDiffController } from '../../MultiDiffController';

/**
 * A file's diff editor as VS Code's multi-diff configures it, over the diff view's settings:
 * unchanged regions folded, nothing to scroll past the last line, and no scrollbars or overview
 * ruler, because the list scrolls and the editor follows. Widgets such as the hover escape the
 * file's frame.
 */
const ITEM_DIFF_EDITOR_OPTIONS: editor.IStandaloneDiffEditorConstructionOptions = {
	contextmenu: false,
	fixedOverflowWidgets: true,
	hideUnchangedRegions: { enabled: true },
	maxComputationTime: 5000,
	overviewRulerBorder: false,
	readOnly: true,
	renderOverviewRuler: false,
	renderSideBySideInlineBreakpoint: 900,
	scrollBeyondLastLine: false,
	scrollbar: {
		handleMouseWheel: false,
		horizontal: 'hidden',
		useShadows: false,
		vertical: 'hidden',
	},
};

interface MultiDiffItemEditorOptions {
	readonly controller: MultiDiffController;
	/** The editor's height, what the file's frame leaves below its header. */
	readonly editorHeight: number;
	/** The shown file's key, or null while the slot shows no file. */
	readonly fileKey: string | null;
	readonly host: RefObject<HTMLDivElement | null>;
	readonly runtime: { readonly editor: Pick<typeof editor, 'createDiffEditor'> };
	/** The sides to compare, or null while the file is collapsed or shows a placeholder. */
	readonly sides: {
		readonly modified: editor.ITextModel;
		readonly original: editor.ITextModel;
	} | null;
	readonly slot: number;
	readonly width: number;
}

/**
 * The slot's diff editor, created when the slot first shows a diff and kept while the view is
 * mounted, as the pool's share. It shows each file's diff from the controller's view models,
 * measures the diff once it is computed, so a file counts its estimated height until then, and
 * scrolls as the controller says while the list scrolls through the file.
 */
export function useMultiDiffItemEditor({
	controller,
	editorHeight,
	fileKey,
	host,
	runtime,
	sides,
	slot,
	width,
}: MultiDiffItemEditorOptions): void {
	const [instance, setInstance] = useState<editor.IStandaloneDiffEditor | null>(null);
	const wanted = sides !== null;

	useLayoutEffect(() => {
		const element = host.current;
		if (!wanted || instance !== null || element === null) return;
		const overflowWidgetsDomNode = controller.overflowWidgets;
		setInstance(
			runtime.editor.createDiffEditor(
				element,
				overflowWidgetsDomNode === null
					? ITEM_DIFF_EDITOR_OPTIONS
					: { ...ITEM_DIFF_EDITOR_OPTIONS, overflowWidgetsDomNode },
			),
		);
	}, [controller, host, instance, runtime, wanted]);

	useLayoutEffect(() => {
		if (instance === null) return undefined;
		return () => {
			controller.models.release(instance);
			instance.dispose();
		};
	}, [controller, instance]);

	useLayoutEffect(() => {
		if (instance !== null) instance.layout({ height: Math.max(0, editorHeight), width });
	}, [editorHeight, instance, width]);

	const original = sides?.original ?? null;
	const modified = sides?.modified ?? null;
	useLayoutEffect(() => {
		if (instance === null || fileKey === null || original === null || modified === null) {
			return undefined;
		}
		const shownEditor = instance.getModifiedEditor();
		let current = true;
		// Until the diff is computed, the editor shows every line, and scrolls only as it is told.
		let computed = false;
		let applying = false;
		const measure = (): void => {
			if (computed) controller.setContentHeight(fileKey, shownEditor.getContentHeight());
		};
		const scrollTo = (scrollTop: number): void => {
			applying = true;
			instance.getOriginalEditor().setScrollTop(scrollTop);
			applying = false;
		};
		const listeners = [
			shownEditor.onDidContentSizeChange(measure),
			shownEditor.onDidScrollChange((event) => {
				if (computed && !applying && event.scrollTopChanged) {
					controller.followInnerScroll(slot, event.scrollTop);
				}
			}),
		];
		void controller.models.show(fileKey, { modified, original }, instance).then(() => {
			if (!current) return;
			computed = true;
			measure();
		});
		controller.setInnerScroller(slot, scrollTo);
		scrollTo(controller.innerScrollTop(slot));
		return () => {
			current = false;
			for (const listener of listeners) listener.dispose();
			controller.setInnerScroller(slot, null);
			controller.models.release(instance);
		};
	}, [controller, fileKey, instance, modified, original, slot]);
}
