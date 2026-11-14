import type { KeyboardEvent, MouseEvent } from 'react';

import {
	matchesNavigationMenuShortcut,
	matchesNavigationRenameShortcut,
} from './navigation-menu-shortcuts';

/** Marks a menu trigger that a row shortcut is opening, so the menu opens beside that row. */
const KEYBOARD_ORIGIN = 'data-navigation-menu-keyboard-origin';

/** The row whose shortcut opened this trigger's menu, read once; null when the pointer opened it. */
export function takeNavigationRowMenuKeyboardOrigin(trigger: HTMLElement): HTMLElement | null {
	if (!trigger.hasAttribute(KEYBOARD_ORIGIN)) return null;
	trigger.removeAttribute(KEYBOARD_ORIGIN);
	return trigger.closest<HTMLElement>('[data-navigation-reorder-row], [data-recents-row]');
}

export function findNavigationRowMenuTrigger(row: HTMLElement): HTMLButtonElement | null {
	const trigger = row.querySelector<HTMLButtonElement>('button[data-navigation-menu-trigger]');
	return trigger?.isConnected === true &&
		!trigger.disabled &&
		trigger.getAttribute('aria-disabled') !== 'true'
		? trigger
		: null;
}

/** F2 renames the row; Shift+F10 and the Menu key open its menu beside it. */
export function handleNavigationRowKeyDown(
	event: KeyboardEvent<HTMLDivElement>,
	rename: () => void,
): void {
	const NodeConstructor = event.currentTarget.ownerDocument.defaultView?.Node;
	if (
		event.defaultPrevented ||
		NodeConstructor === undefined ||
		!(event.target instanceof NodeConstructor) ||
		!event.currentTarget.contains(event.target) ||
		event.currentTarget.hasAttribute('data-popup-open')
	) {
		return;
	}
	if (matchesNavigationRenameShortcut(event.nativeEvent)) {
		event.preventDefault();
		rename();
		return;
	}
	if (!matchesNavigationMenuShortcut(event.nativeEvent)) return;
	const trigger = findNavigationRowMenuTrigger(event.currentTarget);
	if (trigger === null || trigger.getAttribute('aria-expanded') === 'true') return;
	event.preventDefault();
	trigger.focus({ preventScroll: true });
	if (!trigger.isConnected) return;
	trigger.setAttribute(KEYBOARD_ORIGIN, '');
	trigger.click();
}

export function suppressDuplicateNavigationContextMenu(event: MouseEvent<HTMLDivElement>): void {
	const NodeConstructor = event.currentTarget.ownerDocument.defaultView?.Node;
	if (
		event.defaultPrevented ||
		event.button !== -1 ||
		NodeConstructor === undefined ||
		!(event.target instanceof NodeConstructor)
	) {
		return;
	}
	const trigger = findNavigationRowMenuTrigger(event.currentTarget);
	if (trigger?.getAttribute('aria-expanded') !== 'true') return;
	if (!event.currentTarget.contains(event.target)) {
		const popupId = trigger.getAttribute('aria-controls');
		const popup = popupId === null ? null : trigger.ownerDocument.getElementById(popupId);
		if (
			popup?.getAttribute('data-slot') !== 'dropdown-menu-content' ||
			!popup.hasAttribute('data-open') ||
			!popup.contains(event.target)
		) {
			return;
		}
	}
	// A native Menu-key release can synthesize contextmenu after keydown opened this dropdown.
	event.preventDefault();
	event.stopPropagation();
}
