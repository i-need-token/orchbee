import { describe, expect, it } from 'vitest';

import { enUSSshConnectionManagementMessages } from '../../locales';
import { sshConnectionManagementMessages } from '../ssh-connection-management';

describe('sshConnectionManagementMessages', () => {
	it('uses canonical English defaults for every SSH-connection descriptor', () => {
		for (const descriptor of Object.values(sshConnectionManagementMessages)) {
			expect(descriptor.id).toBeTypeOf('string');
			expect(descriptor.defaultMessage).toBe(
				enUSSshConnectionManagementMessages[
					descriptor.id as keyof typeof enUSSshConnectionManagementMessages
				],
			);
		}
	});
});
