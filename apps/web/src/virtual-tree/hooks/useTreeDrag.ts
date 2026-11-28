import * as React from 'react';

import {
	registerPointerDropTarget,
	startPointerDrag,
	type PointerDrag,
	type PointerDragCargo,
	type PointerDragModifiers,
	type PointerDragPoint,
	type PointerDropTarget,
} from '../../pointer-drag';
import { dragScrollStep, HOVER_EXPAND_DELAY, rowsToDrag } from '../interaction/tree-drag';
import type { TreeDropTarget, TreeViewport } from '../layout/TreeViewport';
import type { VirtualTreeRow, VirtualTreeSegment } from '../model/virtual-tree-types';
import type { VirtualTreeDropReaction } from '../types/virtual-tree-drag';
import { hitTreeRow, TREE_ROW_CONTROLS, treePointerTarget } from '../utils/tree-row-hit';

/** The view's drag and drop parts as the tree calls them; null where the view has none. */
interface TreeDragParts<K, T> {
	readonly drag: ((items: readonly VirtualTreeSegment<K, T>[]) => PointerDragCargo | null) | null;
	readonly over:
		| ((
				drag: PointerDrag,
				target: VirtualTreeSegment<K, T> | null,
		  ) => VirtualTreeDropReaction | null)
		| null;
	readonly drop: (drag: PointerDrag, target: VirtualTreeSegment<K, T> | null) => void;
}

/** The view's last answer, kept while the item under the pointer and the keys held stay the same. */
interface DropAnswer<K, T> {
	readonly drag: PointerDrag;
	readonly target: VirtualTreeSegment<K, T> | null;
	readonly modifiers: PointerDragModifiers;
	readonly reaction: VirtualTreeDropReaction | null;
}

/** The row under the pointer and the item of it a drop lands on; null for the tree itself. */
interface DropPlace<K, T> {
	readonly row: VirtualTreeRow<K, T>;
	readonly target: VirtualTreeSegment<K, T>;
}

function sameModifiers(one: PointerDragModifiers, other: PointerDragModifiers): boolean {
	return (
		one.altKey === other.altKey &&
		one.ctrlKey === other.ctrlKey &&
		one.metaKey === other.metaKey &&
		one.shiftKey === other.shiftKey
	);
}

/** Where a drop the view accepted shows, at `place` or on the tree for null. */
function dropTargetOf<K, T>(
	place: DropPlace<K, T> | null,
	reaction: VirtualTreeDropReaction,
): TreeDropTarget<K> {
	if (place === null) return 'tree';
	const { row, target } = place;
	// An item a compact row joins lights up alone, as VS Code's Explorer shows a drop on its name.
	if (target !== row) return { key: target.key };
	if (reaction.bubble !== 'up') return { key: row.key };
	return row.parent === null ? 'tree' : { key: row.parent.key };
}

/**
 * Drags rows and takes drops as VS Code's tree does, through the view's parts. A drag from a selected
 * row carries the selection in tree order, from any other row that row alone, and neither changes
 * the selection. While a drag is over the tree, the rows it would drop into light up, a collapsed row
 * the view lets expand opens once the pointer rests on it for 500 ms, and the tree scrolls while the
 * pointer is within 35 px of its top or bottom edge, whether or not the view accepts the drag there.
 * A sticky row stands for its row as a place to drop.
 *
 * In a compact row, as in VS Code's compact folders, a drag that carries that row alone carries the
 * item whose name was pressed, and a drag over an item's name targets that item: an item the row
 * joins lights up alone, and the row still opens when the view lets it.
 */
export function useTreeDrag<K, T>(
	frameReference: React.RefObject<HTMLElement | null>,
	scrollbarReference: React.RefObject<HTMLElement | null>,
	viewport: TreeViewport<K, T>,
	{ drag, drop, over }: TreeDragParts<K, T>,
): void {
	React.useEffect(() => {
		const frame = frameReference.current;
		if (frame === null || drag === null) return undefined;
		const onPointerDown = (event: PointerEvent) => {
			const target = treePointerTarget(
				frame,
				scrollbarReference.current,
				event.target,
				TREE_ROW_CONTROLS,
			);
			const hit = target === null ? null : hitTreeRow(frame, viewport, target);
			// A sticky row takes drops for its row but starts no drag, as in VS Code.
			if (hit === null || hit.sticky) return;
			const { row, segment } = hit;
			startPointerDrag(event, frame, () => {
				const rows = rowsToDrag(viewport.model, row);
				return drag(rows.length === 1 && segment !== null ? [segment] : rows);
			});
		};
		frame.addEventListener('pointerdown', onPointerDown);
		return () => {
			frame.removeEventListener('pointerdown', onPointerDown);
		};
	}, [drag, frameReference, scrollbarReference, viewport]);

	React.useEffect(() => {
		const frame = frameReference.current;
		const view = frame?.ownerDocument.defaultView;
		if (frame === null || !view || over === null) return undefined;
		const { model } = viewport;
		let answer: DropAnswer<K, T> | null = null;
		/** The collapsed row the pointer rests on, and since when. */
		let resting: { readonly row: VirtualTreeRow<K, T>; readonly since: number } | null = null;
		const placeAt = (point: PointerDragPoint): DropPlace<K, T> | null => {
			if (scrollbarReference.current?.contains(point.element) === true) return null;
			const hit = hitTreeRow(frame, viewport, point.element);
			return hit === null ? null : { row: hit.row, target: hit.segment ?? hit.row };
		};
		const reactionTo = (active: PointerDrag, target: VirtualTreeSegment<K, T> | null) => {
			if (
				answer !== null &&
				answer.drag === active &&
				answer.target === target &&
				sameModifiers(answer.modifiers, active.modifiers)
			) {
				return answer.reaction;
			}
			const reaction = over(active, target);
			answer = { drag: active, modifiers: active.modifiers, reaction, target };
			return reaction;
		};
		const clear = () => {
			answer = null;
			resting = null;
			viewport.setDropTarget(null);
		};
		const target: PointerDropTarget = {
			drop(active, point) {
				const place = placeAt(point);
				clear();
				drop(active, place?.target ?? null);
			},
			leave: clear,
			over(active, point) {
				const step = dragScrollStep(
					point.clientY - frame.getBoundingClientRect().top,
					viewport.viewportHeight,
				);
				if (step !== 0) viewport.scrollTo(viewport.scrollTop + step);
				const place = placeAt(point);
				const reaction = reactionTo(active, place?.target ?? null);
				if (reaction === null) {
					resting = null;
					viewport.setDropTarget(null);
					return false;
				}
				viewport.setDropTarget(dropTargetOf(place, reaction));
				const row = place?.row ?? null;
				if (reaction.autoExpand !== true || row === null || !row.collapsible || !row.collapsed) {
					resting = null;
				} else if (resting?.row !== row) {
					resting = { row, since: view.performance.now() };
				} else if (view.performance.now() - resting.since >= HOVER_EXPAND_DELAY) {
					resting = null;
					model.expand(row.key).catch(() => undefined);
				}
				return true;
			},
		};
		const unregister = registerPointerDropTarget(frame, target);
		return () => {
			unregister();
			viewport.setDropTarget(null);
		};
	}, [drop, frameReference, over, scrollbarReference, viewport]);
}
