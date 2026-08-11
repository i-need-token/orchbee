import { describe, expect, it } from 'vitest';

import { enUSDesktopBrowserAccessMessages } from '../../locales';
import { desktopBrowserAccessMessages } from '../desktop-browser-access';

describe('desktopBrowserAccessMessages', () => {
	it('uses canonical English defaults for every Browser-access descriptor', () => {
		for (const descriptor of Object.values(desktopBrowserAccessMessages)) {
			expect(descriptor.id).toBeTypeOf('string');
			expect(descriptor.defaultMessage).toBe(
				enUSDesktopBrowserAccessMessages[
					descriptor.id as keyof typeof enUSDesktopBrowserAccessMessages
				],
			);
		}
	});
});
