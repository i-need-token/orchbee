import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import * as React from 'react';

import {
	focusedSegment,
	selectRange,
	toggleCollapsed,
	toggleRow,
	type OpenTreeRow,
} from '../interaction/tree-commands';
import type { TreeViewport } from '../layout/TreeViewport';
import type { VirtualTreeRow } from '../model/virtual-tree-types';
import type { VirtualTreeContextMenuEvent } from '../types/virtual-tree-events';
import {
	hitTreeRow,
	TREE_ROW_CONTROLS,
	treePointerTarget,
	type TreeRowHit,
} from '../utils/tree-row-hit';

/** Text fields inside a row keep their own context menu. */
const TEXT_FIELDS = 'input, select, textarea, [contenteditable]';

/**
 * Handles the pointer as VS Code's tree does. A click focuses, anchors and selects its row and opens
 * it as a preview; on a row that expands it also toggles the row, with Alt every row below it, and
 * a click on the twistie only toggles. Shift-click selects a range from the anchor, Ctrl-click, or
 * Cmd-click on macOS, adds or removes one row, and a click on the empty space clears focus,
 * selection and anchor. A middle click opens the row for good, and a double click selects the
 * focused row and opens it for good, moving focus to it. A click on a sticky row scrolls its row
 * below its ancestors' sticky rows first and toggles the row only from the twistie. A right click,
 * or the keyboard's request, focuses its row before the view's menu opens.
 *
 * In a compact row, as in VS Code's compact folders, a press on an item's name focuses that item and
 * acts on the row; a click elsewhere on the row keeps the focused item of a focused row and focuses
 * the row's own item otherwise, and a right click elsewhere focuses the row's own item.
 */
export function useTreePointer<K, T>(
	frameReference: React.RefObject<HTMLElement | null>,
	scrollbarReference: React.RefObject<HTMLElement | null>,
	viewport: TreeViewport<K, T>,
	platform: KeyboardShortcutPlatform,
	open: OpenTreeRow<K, T>,
	showContextMenu: (event: VirtualTreeContextMenuEvent<K, T>) => void,
): void {
	React.useEffect(() => {
		const frame = frameReference.current;
		const view = frame?.ownerDocument.defaultView;
		if (frame === null || !view) return undefined;
		const { model } = viewport;
		/** The element a pointer event targets, unless it belongs to the scrollbar or a row's control. */
		const targetOf = (event: MouseEvent): Element | null =>
			treePointerTarget(frame, scrollbarReference.current, event.target, TREE_ROW_CONTROLS);
		const hitRow = (target: Element) => hitTreeRow(frame, viewport, target);
		const addsOne = (event: MouseEvent) => (platform === 'apple' ? event.metaKey : event.ctrlKey);
		/** The item a click focuses: the one pressed, else the focused one of a focused row. */
		const focusOf = (hit: TreeRowHit<K, T>): K => {
			if (hit.segment !== null) return hit.segment.key;
			const focus = model.focus;
			return focus !== null && viewport.focusIndex === hit.index ? focus : hit.row.key;
		};
		const selectOnly = (row: VirtualTreeRow<K, T>, focus: K) => {
			model.setFocus(focus);
			model.setAnchor(row.key);
			model.setSelection([row.key]);
		};
		const onPointer = (event: MouseEvent) => {
			const target = targetOf(event);
			if (target === null) return;
			const hit = hitRow(target);
			if (hit === null) {
				model.setFocus(null);
				model.setSelection([]);
				model.setAnchor(null);
				return;
			}
			const focus = focusOf(hit);
			if (event.shiftKey) {
				selectRange(viewport, hit.index);
				model.setFocus(focus);
				return;
			}
			if (addsOne(event)) {
				toggleRow(model, hit.row.key, focus);
				return;
			}
			const { row } = hit;
			if (hit.sticky) {
				viewport.scrollBelowStickyRows(row.key);
				selectOnly(row, focus);
				if (hit.twistie && row.collapsible) toggleCollapsed(model, row, event.altKey);
				return;
			}
			if (row.collapsible) {
				model.setFocus(focus);
				toggleCollapsed(model, row, event.altKey);
				if (hit.twistie) return;
			}
			selectOnly(row, focus);
			// The second click of a double click leaves opening to the double click.
			if (event.detail === 2) return;
			open({
				pinned: event.button === 1,
				preserveFocus: true,
				row,
				sideBySide: event.ctrlKey || event.metaKey || event.altKey,
			});
		};
		const onClick = (event: MouseEvent) => {
			if (event.button === 0) onPointer(event);
		};
		const onAuxClick = (event: MouseEvent) => {
			if (event.button === 1) onPointer(event);
		};
		const onDoubleClick = (event: MouseEvent) => {
			const target = targetOf(event);
			const hit = target === null ? null : hitRow(target);
			if (hit === null || hit.twistie) return;
			if (!event.shiftKey && !addsOne(event)) {
				const focus = model.focus;
				model.setSelection(focus === null ? [] : [focus]);
			}
			open({
				pinned: true,
				preserveFocus: false,
				row: hit.row,
				sideBySide: event.ctrlKey || event.metaKey || event.altKey,
			});
		};
		const onContextMenu = (event: MouseEvent) => {
			const target = event.target;
			if (!(target instanceof view.Element)) return;
			const field = target.closest(TEXT_FIELDS);
			if (field !== null && frame.contains(field)) return;
			const hit = hitRow(target);
			if (hit !== null) {
				// The keyboard's request lands on the focused item's name, so it keeps that item.
				const segment = hit.segment ?? hit.row;
				model.setFocus(segment.key);
				showContextMenu({ browserEvent: event, row: hit.row, segment });
				return;
			}
			// A request without a pointer, such as assistive technology's, has no button and lands on
			// the tree itself: it is for the focused row.
			if (event.button < 0) {
				const focus = viewport.focusIndex;
				const row = focus < 0 ? null : model.rowAt(focus);
				showContextMenu({ browserEvent: event, row, segment: focusedSegment(viewport) });
				return;
			}
			model.setFocus(null);
			showContextMenu({ browserEvent: event, row: null, segment: null });
		};
		frame.addEventListener('click', onClick);
		frame.addEventListener('auxclick', onAuxClick);
		frame.addEventListener('dblclick', onDoubleClick);
		frame.addEventListener('contextmenu', onContextMenu);
		return () => {
			frame.removeEventListener('click', onClick);
			frame.removeEventListener('auxclick', onAuxClick);
			frame.removeEventListener('dblclick', onDoubleClick);
			frame.removeEventListener('contextmenu', onContextMenu);
		};
	}, [frameReference, open, platform, scrollbarReference, showContextMenu, viewport]);
}
