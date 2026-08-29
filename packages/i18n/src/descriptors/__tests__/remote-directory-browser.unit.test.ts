import { describe, expect, it } from 'vitest';

import { enUSRemoteDirectoryBrowserMessages } from '../../locales';
import { remoteDirectoryBrowserMessages } from '../remote-directory-browser';

describe('remoteDirectoryBrowserMessages', () => {
	it('uses canonical English defaults for every remote directory browser descriptor', () => {
		for (const descriptor of Object.values(remoteDirectoryBrowserMessages)) {
			expect(descriptor.id).toBeTypeOf('string');
			expect(descriptor.defaultMessage).toBe(
				enUSRemoteDirectoryBrowserMessages[
					descriptor.id as keyof typeof enUSRemoteDirectoryBrowserMessages
				],
			);
		}
	});
});
