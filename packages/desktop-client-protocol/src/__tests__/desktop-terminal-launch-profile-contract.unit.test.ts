import { describe, expect, it } from 'vitest';

import {
	DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID,
	DesktopTerminalLaunchProfileCatalogSchema,
	DesktopTerminalLaunchProfileMainEventSchema,
	DesktopTerminalLaunchProfileRendererCommandSchema,
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
	it('parses the built-in catalog and correlated snapshot response', () => {
		const command = DesktopTerminalLaunchProfileRendererCommandSchema.parse({
			requestId: 'terminal-profile:request:1',
			type: 'desktop-terminal-launch-profile-snapshot-requested',
		});

		expect(
			DesktopTerminalLaunchProfileMainEventSchema.parse({
				requestId: command.requestId,
				result: {
					kind: 'completed',
					snapshot: {
						capabilities: { loginShell: true },
						catalog,
						persistenceIssue: null,
						source: 'built-in',
					},
				},
				type: 'desktop-terminal-launch-profile-responded',
			}),
		).toMatchObject({ result: { kind: 'completed', snapshot: { catalog } } });
	});

	it('requires canonical profile names and preserves literal custom arguments', () => {
		const command = DesktopTerminalLaunchProfileRendererCommandSchema.parse({
			profile: {
				command: {
					arguments: ['--name', 'two words', ''],
					executable: '/usr/bin/example',
					kind: 'custom-executable',
				},
				id: 'profile-1',
				name: 'Example',
			},
			requestId: 'terminal-profile:request:2',
			type: 'desktop-terminal-launch-profile-update-requested',
		});

		expect(command).toMatchObject({
			profile: {
				command: { arguments: ['--name', 'two words', ''] },
				name: 'Example',
			},
		});
		expect(
			DesktopTerminalLaunchProfileRendererCommandSchema.safeParse({
				name: '  Example  ',
				requestId: 'terminal-profile:request:untrimmed',
				sourceProfileId: DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID,
				type: 'desktop-terminal-launch-profile-create-requested',
			}).success,
		).toBe(false);
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
		expect(
			DesktopTerminalLaunchProfileRendererCommandSchema.safeParse({
				name: 'bad\0name',
				requestId: 'terminal-profile:request:3',
				sourceProfileId: DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID,
				type: 'desktop-terminal-launch-profile-create-requested',
			}).success,
		).toBe(false);
		expect(
			DesktopTerminalLaunchProfileRendererCommandSchema.safeParse({
				name: 'bad\ud800name',
				requestId: 'terminal-profile:request:4',
				sourceProfileId: DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID,
				type: 'desktop-terminal-launch-profile-create-requested',
			}).success,
		).toBe(false);
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
