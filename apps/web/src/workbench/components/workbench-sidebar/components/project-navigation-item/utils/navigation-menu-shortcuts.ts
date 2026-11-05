import type { KeyboardShortcutEvent } from '@orchbee/keyboard-shortcuts';

const NAVIGATION_MENU_SHORTCUTS = [
	{ code: 'F2', shiftKey: false },
	{ code: 'F10', shiftKey: true },
	{ code: 'ContextMenu', shiftKey: false },
] as const;

export const navigationMenuAriaKeyShortcuts = NAVIGATION_MENU_SHORTCUTS.map(
	({ code, shiftKey }) => `${shiftKey ? 'Shift+' : ''}${code}`,
).join(' ');

export function matchesNavigationMenuShortcut(event: KeyboardShortcutEvent): boolean {
	return (
		!event.isComposing &&
		event.keyCode !== 229 &&
		!event.repeat &&
		!event.altKey &&
		!event.ctrlKey &&
		!event.metaKey &&
		NAVIGATION_MENU_SHORTCUTS.some(
			(shortcut) => shortcut.code === event.code && shortcut.shiftKey === event.shiftKey,
		)
	);
}
