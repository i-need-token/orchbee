import { describe, expect, it } from 'vitest';

import { resolveDesktopDevelopmentSshEnvironmentRelease } from '../resolveDesktopDevelopmentSshEnvironmentRelease';

describe('Desktop development SSH Environment release configuration', () => {
	it('accepts release paths only when both are absent or both are present', () => {
		expect(resolveDesktopDevelopmentSshEnvironmentRelease({})).toBeUndefined();
		expect(
			resolveDesktopDevelopmentSshEnvironmentRelease({
				ORCHBEE_DESKTOP_DEVELOPMENT_SSH_ENVIRONMENT_RELEASE_DIRECTORY: '/release',
				ORCHBEE_DESKTOP_DEVELOPMENT_SSH_ENVIRONMENT_TRUST_DESCRIPTOR: '/trust.json',
			}),
		).toEqual({
			releaseDirectoryPath: '/release',
			trustDescriptorFilePath: '/trust.json',
		});
		expect(() =>
			resolveDesktopDevelopmentSshEnvironmentRelease({
				ORCHBEE_DESKTOP_DEVELOPMENT_SSH_ENVIRONMENT_RELEASE_DIRECTORY: '/release',
			}),
		).toThrow('Development SSH Environment release configuration is incomplete.');
	});
});
