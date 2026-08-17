import { describe, expect, it } from 'vitest';

import { enUSDesktopSecureKeyboardEntryMessages } from '../../locales';
import { desktopSecureKeyboardEntryMessages } from '../desktop-secure-keyboard-entry';

describe('desktopSecureKeyboardEntryMessages', () => {
	it('uses canonical English defaults for every Secure Keyboard Entry descriptor', () => {
		for (const descriptor of Object.values(desktopSecureKeyboardEntryMessages)) {
			expect(descriptor.id).toBeTypeOf('string');
			expect(descriptor.defaultMessage).toBe(
				enUSDesktopSecureKeyboardEntryMessages[
					descriptor.id as keyof typeof enUSDesktopSecureKeyboardEntryMessages
				],
			);
		}
	});
});
