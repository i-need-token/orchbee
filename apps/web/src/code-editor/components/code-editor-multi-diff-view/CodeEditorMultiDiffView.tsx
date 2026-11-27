import { useLayoutEffect, useRef, useState, useSyncExternalStore, type ReactNode } from 'react';

import type { editor } from '../../monaco-entry';
import styles from './CodeEditorMultiDiffView.module.css';
import { MultiDiffItem } from './components/multi-diff-item/MultiDiffItem';
import { MultiDiffController, type MultiDiffEntry } from './MultiDiffController';
import { useMultiDiffScrolling } from './useMultiDiffScrolling';

interface CodeEditorMultiDiffViewProps {
	/** The files in display order. Pass the same array while the list stays the same. */
	readonly entries: readonly MultiDiffEntry[];
	/** Monaco, from what `loadCodeEditor` resolves to. */
	readonly runtime: { readonly editor: Pick<typeof editor, 'createDiffEditor'> };
}

/**
 * A multi-diff tab's content, as VS Code's multi-diff editor shows it: every file's diff in one
 * scrolling list, each under a header that collapses it. Only the files in sight have editors,
 * lent from a pool as they scroll in, and a file keeps its computed diff while it is listed.
 */
export function CodeEditorMultiDiffView({
	entries,
	runtime,
}: CodeEditorMultiDiffViewProps): ReactNode {
	const scrollerReference = useRef<HTMLDivElement>(null);
	const [controller] = useState(() => new MultiDiffController());
	// It runs before the items' cleanups, so the editors let go of their diffs before they go.
	useLayoutEffect(() => () => controller.dispose(), [controller]);
	useLayoutEffect(() => {
		controller.setEntries(entries);
	}, [controller, entries]);
	useMultiDiffScrolling(scrollerReference, controller);
	const slots = useSyncExternalStore(controller.subscribe, () => controller.slots);
	const contentHeight = useSyncExternalStore(controller.subscribe, () => controller.contentHeight);
	useLayoutEffect(() => {
		controller.flushScroll();
	});

	// A slot keeps its place, and its editor, as the files it shows change.
	const items: ReactNode[] = [];
	for (let slot = 0; slot < slots.length; slot += 1) {
		items.push(<MultiDiffItem controller={controller} key={slot} runtime={runtime} slot={slot} />);
	}
	return (
		<div className={styles['root']} data-slot='code-editor-multi-diff-view' ref={scrollerReference}>
			<div
				className={styles['content']}
				style={{ '--ob-comp-code-editor-multi-diff-content-height': `${contentHeight}px` }}
			>
				{items}
			</div>
		</div>
	);
}
