import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import * as React from 'react';

import {
	selectRange,
	toggleCollapsed,
	toggleRow,
	type OpenTreeRow,
} from '../interaction/tree-commands';
import type { TreeViewport } from '../layout/TreeViewport';
import type { VirtualTreeRow } from '../model/virtual-tree-types';
import type { VirtualTreeContextMenuEvent } from '../types/virtual-tree-events';

/** Controls inside a row's content handle their own clicks, as VS Code's lists leave them. */
const ROW_CONTROLS = 'a[href], button, input, select, textarea, [contenteditable]';

/** Text fields inside a row keep their own context menu. */
const TEXT_FIELDS = 'input, select, textarea, [contenteditable]';

/** The row a pointer event landed on. */
interface RowHit<K, T> {
	readonly row: VirtualTreeRow<K, T>;
	readonly index: number;
	readonly sticky: boolean;
	readonly twistie: boolean;
}

/**
 * Handles the pointer as VS Code's tree does. A click focuses, anchors and selects its row and opens
 * it as a preview; on a row that expands it also toggles the row, with Alt every row below it, and
 * a click on the twistie only toggles. Shift-click selects a range from the anchor, Ctrl-click, or
 * Cmd-click on macOS, adds or removes one row, and a click on the empty space clears focus,
 * selection and anchor. A middle click opens the row for good, and a double click selects the
 * focused row and opens it for good, moving focus to it. A click on a sticky row scrolls its row
 * below its ancestors' sticky rows first and toggles the row only from the twistie. A right click,
 * or the keyboard's request, focuses its row before the view's menu opens.
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
		const targetOf = (event: MouseEvent, controls: string): Element | null => {
			const target = event.target;
			if (!(target instanceof view.Element)) return null;
			if (scrollbarReference.current?.contains(target) === true) return null;
			const control = target.closest(controls);
			return control !== null && frame.contains(control) ? null : target;
		};
		const hitRow = (target: Element): RowHit<K, T> | null => {
			const element = target.closest('[data-row-index], [data-sticky-index]');
			if (element === null || !frame.contains(element)) return null;
			const twistie = target.closest('[data-twistie]');
			const onTwistie = twistie !== null && element.contains(twistie);
			const depth = element.getAttribute('data-sticky-index');
			if (depth !== null) {
				const sticky = viewport.stickyRow(Number(depth));
				if (sticky === null) return null;
				const index = model.indexOf(sticky.row.key);
				return index < 0 ? null : { index, row: sticky.row, sticky: true, twistie: onTwistie };
			}
			const index = Number(element.getAttribute('data-row-index'));
			if (!Number.isInteger(index) || index < 0 || index >= model.rowCount) return null;
			return { index, row: model.rowAt(index), sticky: false, twistie: onTwistie };
		};
		const addsOne = (event: MouseEvent) => (platform === 'apple' ? event.metaKey : event.ctrlKey);
		const selectOnly = (row: VirtualTreeRow<K, T>) => {
			model.setFocus(row.key);
			model.setAnchor(row.key);
			model.setSelection([row.key]);
		};
		const onPointer = (event: MouseEvent) => {
			const target = targetOf(event, ROW_CONTROLS);
			if (target === null) return;
			const hit = hitRow(target);
			if (hit === null) {
				model.setFocus(null);
				model.setSelection([]);
				model.setAnchor(null);
				return;
			}
			if (event.shiftKey) {
				selectRange(viewport, hit.index);
				return;
			}
			if (addsOne(event)) {
				toggleRow(model, hit.row.key);
				return;
			}
			const { row } = hit;
			if (hit.sticky) {
				viewport.scrollBelowStickyRows(row.key);
				selectOnly(row);
				if (hit.twistie && row.collapsible) toggleCollapsed(model, row, event.altKey);
				return;
			}
			if (row.collapsible) {
				model.setFocus(row.key);
				toggleCollapsed(model, row, event.altKey);
				if (hit.twistie) return;
			}
			selectOnly(row);
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
			const target = targetOf(event, ROW_CONTROLS);
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
				model.setFocus(hit.row.key);
				showContextMenu({ browserEvent: event, row: hit.row });
				return;
			}
			// A request without a pointer, such as assistive technology's, has no button and lands on
			// the tree itself: it is for the focused row.
			if (event.button < 0) {
				const focus = viewport.focusIndex;
				showContextMenu({ browserEvent: event, row: focus < 0 ? null : model.rowAt(focus) });
				return;
			}
			model.setFocus(null);
			showContextMenu({ browserEvent: event, row: null });
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
