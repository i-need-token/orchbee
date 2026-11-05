import type { KeyboardShortcutChord } from '@orchbee/keyboard-shortcuts';

/**
 * A chord one of the window's shortcuts holds now, with the name the shortcut settings give its
 * action. `personal` marks a chord the person set; any other is the shortcut's default.
 */
export interface WorkbenchShortcutClaim {
	readonly action: string;
	readonly chord: KeyboardShortcutChord;
	readonly personal: boolean;
}
