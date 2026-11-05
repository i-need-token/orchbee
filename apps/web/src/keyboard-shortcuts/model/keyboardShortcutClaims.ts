import type { IntlShape } from '@orchbee/i18n';
import type { WorkbenchShortcutClaim } from '../../workbench';

import { formatKeyboardShortcutActionName } from './formatKeyboardShortcutActionName';
import { KEYBOARD_SHORTCUT_CATALOG } from './keyboard-shortcut-catalog';
import {
	keyboardShortcutBindingSource,
	type KeyboardShortcutSettingsSnapshot,
} from './keyboard-shortcut-settings-controller';

/**
 * Every shortcut's current chord for the Workbench, named as the shortcut settings name it, so a
 * configured command can take a default's chord and yield to one the person set. A claim's id is the
 * shortcut's, so the settings can name the command that took it.
 */
export function keyboardShortcutClaims(
	snapshot: KeyboardShortcutSettingsSnapshot,
	intl: IntlShape,
): readonly WorkbenchShortcutClaim[] {
	return KEYBOARD_SHORTCUT_CATALOG.map((declaration) => ({
		action: formatKeyboardShortcutActionName(intl, declaration),
		chord: snapshot.bindings[declaration.shortcutId],
		id: declaration.shortcutId,
		personal: keyboardShortcutBindingSource(snapshot, declaration.shortcutId) === 'custom',
	}));
}
