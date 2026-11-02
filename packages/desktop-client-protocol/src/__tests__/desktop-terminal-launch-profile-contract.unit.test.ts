import { describe, expect, it } from 'vitest';

import {
	DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID,
	DesktopTerminalLaunchProfileCatalogSchema,
	DesktopTerminalLaunchProfileNameSchema,
	DesktopTerminalLaunchProfileSchema,
} from '../desktop-terminal-launch-profile-contract';

const catalog = DesktopTerminalLaunchProfileCatalogSchema.parse({
	defaultProfileId: DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID,
	profiles: [
		{
			command: { kind: 'system-shell', login: false },
			id: DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID,
			name: null,
		},
	],
	version: 1,
});

describe('Desktop terminal launch-profile contract', () => {
	it('requires canonical profile names and preserves literal custom arguments', () => {
		expect(
			DesktopTerminalLaunchProfileSchema.parse({
				command: {
					arguments: ['--name', 'two words', ''],
					executable: '/usr/bin/example',
					kind: 'custom-executable',
				},
				id: 'profile-1',
				name: 'Example',
			}),
		).toMatchObject({
			command: { arguments: ['--name', 'two words', ''] },
			name: 'Example',
		});
		expect(DesktopTerminalLaunchProfileNameSchema.safeParse('  Example  ').success).toBe(false);
	});

	it('rejects duplicate IDs, a missing default, foreign localized markers, and invalid text', () => {
		expect(
			DesktopTerminalLaunchProfileCatalogSchema.safeParse({
				defaultProfileId: 'missing',
				profiles: [catalog.profiles[0], catalog.profiles[0]],
				version: 1,
			}).success,
		).toBe(false);
		expect(
			DesktopTerminalLaunchProfileCatalogSchema.safeParse({
				defaultProfileId: 'profile-1',
				profiles: [
					{ command: { kind: 'system-shell', login: false }, id: 'profile-1', name: null },
				],
				version: 1,
			}).success,
		).toBe(false);
		expect(DesktopTerminalLaunchProfileNameSchema.safeParse('bad\0name').success).toBe(false);
		expect(DesktopTerminalLaunchProfileNameSchema.safeParse('bad\ud800name').success).toBe(false);
	});

	it('rejects a catalog whose encoded profile data exceeds the storage boundary', () => {
		expect(
			DesktopTerminalLaunchProfileCatalogSchema.safeParse({
				defaultProfileId: 'system-default',
				profiles: Array.from({ length: 64 }, (_, index) => ({
					command: {
						arguments: Array.from({ length: 128 }, () => 'x'.repeat(512)),
						executable: '/usr/bin/example',
						kind: 'custom-executable',
					},
					id: index === 0 ? 'system-default' : `profile-${index}`,
					name: index === 0 ? null : `Profile ${index}`,
				})),
				version: 1,
			}).success,
		).toBe(false);
	});
});
