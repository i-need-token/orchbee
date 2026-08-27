import { describe, expect, it } from 'vitest';

import { enUSBrowserAuthenticationMessages } from '../../locales';
import { browserAuthenticationMessages } from '../browser-authentication';

describe('browserAuthenticationMessages', () => {
	it('uses canonical English defaults for every Browser authentication descriptor', () => {
		for (const descriptor of Object.values(browserAuthenticationMessages)) {
			expect(descriptor.id).toBeTypeOf('string');
			expect(descriptor.defaultMessage).toBe(
				enUSBrowserAuthenticationMessages[
					descriptor.id as keyof typeof enUSBrowserAuthenticationMessages
				],
			);
		}
	});
});
