import { describe, expect, it } from 'vitest';

import { enUSKeyboardShortcutMessages } from '../../locales';
import { keyboardShortcutMessages } from '../keyboard-shortcuts';

describe('keyboardShortcutMessages', () => {
	it('uses canonical English defaults for every keyboard-shortcut descriptor', () => {
		for (const descriptor of Object.values(keyboardShortcutMessages)) {
			expect(descriptor.id).toBeTypeOf('string');
			expect(descriptor.defaultMessage).toBe(
				enUSKeyboardShortcutMessages[descriptor.id as keyof typeof enUSKeyboardShortcutMessages],
			);
		}
	});
});
