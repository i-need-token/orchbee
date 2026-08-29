import { describe, expect, it } from 'vitest';

import { enUSDesktopSshProjectMessages } from '../../locales';
import { desktopSshProjectMessages } from '../desktop-ssh-project';

describe('desktopSshProjectMessages', () => {
	it('uses canonical English defaults for every SSH Project descriptor', () => {
		for (const descriptor of Object.values(desktopSshProjectMessages)) {
			expect(descriptor.id).toBeTypeOf('string');
			expect(descriptor.defaultMessage).toBe(
				enUSDesktopSshProjectMessages[descriptor.id as keyof typeof enUSDesktopSshProjectMessages],
			);
		}
	});
});
