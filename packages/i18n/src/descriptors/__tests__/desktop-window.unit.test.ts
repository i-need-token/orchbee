import { describe, expect, it } from 'vitest';

import { enUSDesktopWindowMessages } from '../../locales';
import { desktopWindowMessages } from '../desktop-window';

describe('desktopWindowMessages', () => {
	it('uses canonical English defaults for every desktop-window descriptor', () => {
		for (const descriptor of Object.values(desktopWindowMessages)) {
			expect(descriptor.id).toBeTypeOf('string');
			expect(descriptor.defaultMessage).toBe(
				enUSDesktopWindowMessages[descriptor.id as keyof typeof enUSDesktopWindowMessages],
			);
		}
	});
});
