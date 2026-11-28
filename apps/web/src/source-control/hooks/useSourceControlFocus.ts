import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { SourceControlFocus } from '../types/source-control-focus';

interface UseSourceControlFocusOptions {
	readonly changesTree: React.RefObject<HTMLDivElement | null>;
	/** Moves focus into a commit input when the Changes section would take it; false when it did not. */
	readonly focusInput: () => boolean;
	readonly reference: React.Ref<SourceControlFocus> | undefined;
	readonly repositoriesTree: React.RefObject<HTMLDivElement | null>;
	readonly rootReference: React.RefObject<HTMLDivElement | null>;
	/** Whether a section's tree shows. */
	readonly treeShown: boolean;
}

interface SourceControlFocusTracking {
	/** Records that focus entered the Changes section. */
	readonly onChangesFocus: () => void;
	/** Records that focus entered the Repositories section. */
	readonly onRepositoriesFocus: () => void;
}

/**
 * Source Control's side of a focus request: the tree of the section focused last takes focus, else
 * the first shown of Changes and Repositories, where a commit input may take it first. While no tree
 * shows, the view itself takes focus and hands it to a tree once one shows, unless focus has moved.
 */
export function useSourceControlFocus({
	changesTree,
	focusInput,
	reference,
	repositoriesTree,
	rootReference,
	treeShown,
}: UseSourceControlFocusOptions): SourceControlFocusTracking {
	const lastReference = React.useRef<'changes' | 'repositories'>('changes');
	const pendingReference = React.useRef(false);
	const firstTree = useMemoizedFn((): HTMLDivElement | null => {
		const [first, second] =
			lastReference.current === 'repositories'
				? [repositoriesTree, changesTree]
				: [changesTree, repositoriesTree];
		return first.current ?? second.current;
	});
	React.useImperativeHandle(
		reference,
		() => ({
			focus: () => {
				if (lastReference.current === 'changes' && focusInput()) return;
				const tree = firstTree();
				pendingReference.current = tree === null;
				(tree ?? rootReference.current)?.focus({ preventScroll: true });
			},
		}),
		[firstTree, focusInput, rootReference],
	);
	React.useLayoutEffect(() => {
		const root = rootReference.current;
		const tree = firstTree();
		if (!treeShown || !pendingReference.current || root === null || tree === null) return;
		pendingReference.current = false;
		if (root.ownerDocument.activeElement === root) tree.focus({ preventScroll: true });
	}, [firstTree, rootReference, treeShown]);
	return React.useMemo(
		() => ({
			onChangesFocus: () => {
				lastReference.current = 'changes';
			},
			onRepositoriesFocus: () => {
				lastReference.current = 'repositories';
			},
		}),
		[],
	);
}
