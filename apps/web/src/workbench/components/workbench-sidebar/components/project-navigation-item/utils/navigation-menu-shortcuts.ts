import type { KeyboardShortcutEvent } from '@orchbee/keyboard-shortcuts';

interface NavigationRowShortcut {
	readonly code: string;
	readonly shiftKey: boolean;
}

const NAVIGATION_RENAME_SHORTCUT: NavigationRowShortcut = { code: 'F2', shiftKey: false };

const NAVIGATION_MENU_SHORTCUTS: readonly NavigationRowShortcut[] = [
	{ code: 'F10', shiftKey: true },
	{ code: 'ContextMenu', shiftKey: false },
];

function ariaKeyShortcuts(shortcuts: readonly NavigationRowShortcut[]): string {
	return shortcuts.map(({ code, shiftKey }) => `${shiftKey ? 'Shift+' : ''}${code}`).join(' ');
}

/** The keys that open a row's menu, announced on the menu's own button. */
export const navigationMenuAriaKeyShortcuts = ariaKeyShortcuts(NAVIGATION_MENU_SHORTCUTS);

/** A row renames with F2 and opens its menu with Shift+F10 or the Menu key. */
export const navigationRowAriaKeyShortcuts = ariaKeyShortcuts([
	NAVIGATION_RENAME_SHORTCUT,
	...NAVIGATION_MENU_SHORTCUTS,
]);

function matchesAny(
	event: KeyboardShortcutEvent,
	shortcuts: readonly NavigationRowShortcut[],
): boolean {
	return (
		!event.isComposing &&
		event.keyCode !== 229 &&
		!event.repeat &&
		!event.altKey &&
		!event.ctrlKey &&
		!event.metaKey &&
		shortcuts.some(
			(shortcut) => shortcut.code === event.code && shortcut.shiftKey === event.shiftKey,
		)
	);
}

export function matchesNavigationMenuShortcut(event: KeyboardShortcutEvent): boolean {
	return matchesAny(event, NAVIGATION_MENU_SHORTCUTS);
}

export function matchesNavigationRenameShortcut(event: KeyboardShortcutEvent): boolean {
	return matchesAny(event, [NAVIGATION_RENAME_SHORTCUT]);
}
