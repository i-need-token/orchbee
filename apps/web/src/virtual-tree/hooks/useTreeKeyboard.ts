import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import * as React from 'react';

import { runTreeKeyCommand, type OpenTreeRow } from '../interaction/tree-commands';
import { isContextMenuKey, treeKeyCommands } from '../interaction/tree-keys';
import {
	TreeTypeNavigation,
	typesCharacter,
	type VirtualTreeKeyboardNavigationLabel,
} from '../interaction/type-navigation';
import type { TreeViewport } from '../layout/TreeViewport';

/**
 * Handles the keys pressed while the tree itself has focus, as VS Code's lists do: its list
 * commands, type navigation when the view supplies labels, Tab into the focused row's first control,
 * and Shift+F10 or the context menu key, which open the context menu beside the focused row once the
 * key is released. Keys pressed in a control inside a row stay with the control, and a key a wider
 * owner took first is left alone. A key the tree uses has its default prevented and still bubbles.
 */
export function useTreeKeyboard<K, T>(
	frameReference: React.RefObject<HTMLElement | null>,
	viewport: TreeViewport<K, T>,
	platform: KeyboardShortcutPlatform,
	open: OpenTreeRow<K, T>,
	label: VirtualTreeKeyboardNavigationLabel<K, T> | null,
): void {
	React.useEffect(() => {
		const frame = frameReference.current;
		if (frame === null) return undefined;
		const typeNavigation = new TreeTypeNavigation<K, T>();
		// The context menu key, or F10, between its press with Shift and its release.
		let contextMenuCode: string | null = null;
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.target !== frame || event.defaultPrevented) return;
			if (event.isComposing || event.keyCode === 229) return;
			if (isContextMenuKey(event)) {
				event.preventDefault();
				contextMenuCode = event.code;
				return;
			}
			if (event.code === 'Tab') {
				if (!event.shiftKey && !event.ctrlKey && !event.altKey && !event.metaKey) {
					if (focusRowControl(frame, viewport)) event.preventDefault();
				}
				return;
			}
			for (const command of treeKeyCommands(event, platform)) {
				if (!runTreeKeyCommand(viewport, command, open)) continue;
				event.preventDefault();
				return;
			}
			if (label !== null && typesCharacter(event)) {
				event.preventDefault();
				typeNavigation.type(viewport, label, event.key);
			}
		};
		const onKeyUp = (event: KeyboardEvent) => {
			if (contextMenuCode === null || event.code !== contextMenuCode) return;
			contextMenuCode = null;
			event.preventDefault();
			showContextMenu(frame, viewport);
		};
		const onBlur = () => {
			contextMenuCode = null;
		};
		frame.addEventListener('keydown', onKeyDown);
		frame.addEventListener('keyup', onKeyUp);
		frame.addEventListener('blur', onBlur);
		return () => {
			frame.removeEventListener('keydown', onKeyDown);
			frame.removeEventListener('keyup', onKeyUp);
			frame.removeEventListener('blur', onBlur);
			typeNavigation.dispose();
		};
	}, [frameReference, label, open, platform, viewport]);
}

/** The element showing a row, which the tree's `aria-activedescendant` names while it is focused. */
function rowElement(frame: HTMLElement, index: number): Element | null {
	return index < 0 ? null : frame.querySelector(`[data-row-index="${index}"]`);
}

/**
 * VS Code's Tab in a list: moves focus to the first element of the focused row that sets a
 * `tabindex` of its own, when it shows and Tab could reach it; otherwise Tab leaves the tree.
 */
function focusRowControl<K, T>(frame: HTMLElement, viewport: TreeViewport<K, T>): boolean {
	const view = frame.ownerDocument.defaultView;
	const control = rowElement(frame, viewport.focusIndex)?.querySelector('[tabindex]');
	if (!view || !(control instanceof view.HTMLElement) || control.tabIndex === -1) return false;
	const style = view.getComputedStyle(control);
	if (style.visibility === 'hidden' || style.display === 'none') return false;
	control.focus();
	return true;
}

/**
 * Asks for the context menu from the keyboard: a `contextmenu` event on the focused row, or on the
 * tree without one, at the row's bottom-left corner within the tree, where VS Code anchors its menu.
 * It bubbles through the tree's own handling to a view's menu trigger, as a right click does. The
 * browser's own menu for the key stays away, since the tree prevents the key's default on its press
 * and, for the context menu key on Windows, on its release.
 */
function showContextMenu<K, T>(frame: HTMLElement, viewport: TreeViewport<K, T>): void {
	const view = frame.ownerDocument.defaultView;
	if (!view) return;
	const target = rowElement(frame, viewport.focusIndex) ?? frame;
	const bounds = frame.getBoundingClientRect();
	const anchor = target.getBoundingClientRect();
	target.dispatchEvent(
		new view.MouseEvent('contextmenu', {
			bubbles: true,
			cancelable: true,
			clientX: Math.min(Math.max(anchor.left, bounds.left), bounds.right),
			clientY: Math.min(Math.max(anchor.bottom, bounds.top), bounds.bottom),
			composed: true,
			view,
		}),
	);
}
