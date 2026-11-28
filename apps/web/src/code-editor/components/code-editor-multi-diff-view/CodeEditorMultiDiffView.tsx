import { useLayoutEffect, useRef, useState, useSyncExternalStore, type ReactNode } from 'react';

import type { CodeEditorMultiDiffHandle } from '../../code-editor-multi-diff-handle';
import type { CodeEditorViewStates } from '../../CodeEditorViewStates';
import type { editor } from '../../monaco-entry';
import styles from './CodeEditorMultiDiffView.module.css';
import { MultiDiffItem } from './components/multi-diff-item/MultiDiffItem';
import {
	MultiDiffController,
	type CodeEditorMultiDiffViewState,
	type MultiDiffEntry,
} from './MultiDiffController';
import { useMultiDiffScrolling } from './useMultiDiffScrolling';

interface CodeEditorMultiDiffViewProps {
	/** The files in display order. Pass the same array while the list stays the same. */
	readonly entries: readonly MultiDiffEntry[];
	/** Receives the view's handle once it is mounted, and null as it goes. */
	readonly onHandle?: ((handle: CodeEditorMultiDiffHandle | null) => void) | undefined;
	/** Monaco, from what `loadCodeEditor` resolves to. */
	readonly runtime: { readonly editor: Pick<typeof editor, 'createDiffEditor'> };
	/** The key the list's view state is kept under, such as its tab's. */
	readonly viewStateKey: string;
	readonly viewStates: CodeEditorViewStates<CodeEditorMultiDiffViewState>;
}

/**
 * A multi-diff tab's content, as VS Code's multi-diff editor shows it: every file's diff in one
 * scrolling list, each under a header that collapses it, all scrolled sideways together. Only the
 * files in sight have editors, lent from a pool as they scroll in, and a file keeps its computed
 * diff while it is listed. The list returns to where it was left under the same key.
 */
export function CodeEditorMultiDiffView({
	entries,
	onHandle,
	runtime,
	viewStateKey,
	viewStates,
}: CodeEditorMultiDiffViewProps): ReactNode {
	const rootReference = useRef<HTMLDivElement>(null);
	const scrollerReference = useRef<HTMLDivElement>(null);
	const horizontalReference = useRef<HTMLDivElement>(null);
	const [controller] = useState(() => new MultiDiffController());
	useLayoutEffect(() => {
		controller.setEntries(entries);
	}, [controller, entries]);
	useLayoutEffect(() => {
		controller.restore(viewStates.get(viewStateKey));
		return () => viewStates.set(viewStateKey, controller.viewState());
	}, [controller, viewStateKey, viewStates]);
	useLayoutEffect(() => {
		if (onHandle === undefined) return undefined;
		onHandle({
			allCollapsed: () => controller.allCollapsed,
			collapseAll: () => controller.collapseAll(),
			expandAll: () => controller.expandAll(),
			subscribe: controller.subscribe,
		});
		return () => onHandle(null);
	}, [controller, onHandle]);
	// After the view state is kept, which reads the editors' selections, and before the items'
	// cleanups, so the editors let go of their diffs before they go.
	useLayoutEffect(() => () => controller.dispose(), [controller]);
	useMultiDiffScrolling({
		controller,
		horizontal: horizontalReference,
		root: rootReference,
		scroller: scrollerReference,
	});
	const slots = useSyncExternalStore(controller.subscribe, () => controller.slots);
	const contentHeight = useSyncExternalStore(controller.subscribe, () => controller.contentHeight);
	const maxScrollLeft = useSyncExternalStore(controller.subscribe, () => controller.maxScrollLeft);
	const viewportWidth = useSyncExternalStore(controller.subscribe, () => controller.viewportWidth);
	useLayoutEffect(() => {
		controller.flushScroll({ contentHeight, scrollWidth: viewportWidth + maxScrollLeft });
	});

	// A slot keeps its place, and its editor, as the files it shows change.
	const items: ReactNode[] = [];
	for (let slot = 0; slot < slots.length; slot += 1) {
		items.push(<MultiDiffItem controller={controller} key={slot} runtime={runtime} slot={slot} />);
	}
	return (
		<div className={styles['root']} data-slot='code-editor-multi-diff-view' ref={rootReference}>
			<div className={styles['scroller']} ref={scrollerReference}>
				<div
					className={styles['content']}
					style={{ '--ob-comp-code-editor-multi-diff-content-height': `${contentHeight}px` }}
				>
					{items}
				</div>
			</div>
			<div
				className={styles['horizontal']}
				hidden={maxScrollLeft <= 0}
				ref={horizontalReference}
				style={{ '--ob-comp-code-editor-multi-diff-view-width': `${viewportWidth}px` }}
			>
				<div
					className={styles['extent']}
					style={{
						'--ob-comp-code-editor-multi-diff-scroll-width': `${viewportWidth + maxScrollLeft}px`,
					}}
				/>
			</div>
		</div>
	);
}
