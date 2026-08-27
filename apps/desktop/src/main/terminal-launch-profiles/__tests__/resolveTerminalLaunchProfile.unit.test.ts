import {
	DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID,
	DesktopTerminalLaunchProfileIdSchema,
} from '@orchbee/desktop-client-protocol';
import { TerminalLaunchSchema } from '@orchbee/terminal-protocol';
import { describe, expect, it } from 'vitest';

import { createDefaultTerminalProfile } from '../../host';
import type { AutomaticShellIntegrationConfiguration } from '../resolveAutomaticShellIntegration';
import { resolveTerminalLaunchProfile } from '../resolveTerminalLaunchProfile';

const baseLaunch = TerminalLaunchSchema.parse({
	arguments: [],
	cellPixelHeight: 16,
	cellPixelWidth: 8,
	columns: 80,
	configuration: null,
	currentDirectory: '/users/orchbee',
	environment: [
		{ name: 'TERM', value: 'xterm-256color' },
		{ name: 'ORCHBEE_SHELL_INTEGRATION_DIR', value: '/app/shell-integration' },
	],
	executable: '/bin/zsh',
	rows: 24,
	terminalProfile: createDefaultTerminalProfile(),
});

const automaticShellIntegration: AutomaticShellIntegrationConfiguration = {
	availableShells: ['bash', 'fish', 'zsh'],
	directory: '/app/shell-integration',
	inheritedEnvironment: {
		xdgDataDirectories: null,
		zdotdir: null,
	},
};

describe('resolveTerminalLaunchProfile', () => {
	it('keeps the complete trusted launch for the normal system shell', () => {
		expect(
			resolveTerminalLaunchProfile(
				baseLaunch,
				{
					command: { kind: 'system-shell', login: false },
					id: DesktopTerminalLaunchProfileIdSchema.parse(
						DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID,
					),
					name: null,
				},
				'darwin',
			),
		).toEqual({ kind: 'resolved', launch: baseLaunch });
	});

	it('adds only the POSIX login argument to the system shell', () => {
		const result = resolveTerminalLaunchProfile(
			baseLaunch,
			{
				command: { kind: 'system-shell', login: true },
				id: DesktopTerminalLaunchProfileIdSchema.parse(DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID),
				name: null,
			},
			'linux',
		);

		expect(result).toMatchObject({
			kind: 'resolved',
			launch: {
				arguments: ['-l'],
				currentDirectory: '/users/orchbee',
				environment: baseLaunch.environment,
				executable: '/bin/zsh',
				terminalProfile: baseLaunch.terminalProfile,
			},
		});
	});

	it('overlays a custom executable and literal arguments without changing protected fields', () => {
		const result = resolveTerminalLaunchProfile(
			baseLaunch,
			{
				command: {
					arguments: ['--name', 'two words', ''],
					executable: '/opt/example/bin/tool',
					kind: 'custom-executable',
				},
				id: DesktopTerminalLaunchProfileIdSchema.parse('profile-custom'),
				name: 'Custom',
			},
			'darwin',
		);

		expect(result).toMatchObject({
			kind: 'resolved',
			launch: {
				arguments: ['--name', 'two words', ''],
				currentDirectory: '/users/orchbee',
				environment: baseLaunch.environment,
				executable: '/opt/example/bin/tool',
				terminalProfile: baseLaunch.terminalProfile,
			},
		});
	});

	it('applies automatic integration only after the profile selects a supported shell', () => {
		const result = resolveTerminalLaunchProfile(
			baseLaunch,
			{
				command: {
					arguments: ['-i'],
					executable: '/opt/homebrew/bin/zsh',
					kind: 'custom-executable',
				},
				id: DesktopTerminalLaunchProfileIdSchema.parse('profile-zsh'),
				name: 'Zsh',
			},
			'darwin',
			automaticShellIntegration,
		);

		expect(result).toMatchObject({
			kind: 'resolved',
			launch: {
				arguments: ['-i'],
				executable: '/opt/homebrew/bin/zsh',
			},
		});
		if (result.kind !== 'resolved') throw new Error('Expected a resolved Zsh profile.');
		expect(result.launch.environment).toContainEqual({
			name: 'ZDOTDIR',
			value: '/app/shell-integration/automatic/zsh',
		});

		const unsupported = resolveTerminalLaunchProfile(
			baseLaunch,
			{
				command: {
					arguments: ['--name', 'two words'],
					executable: '/opt/example/bin/tool',
					kind: 'custom-executable',
				},
				id: DesktopTerminalLaunchProfileIdSchema.parse('profile-tool'),
				name: 'Tool',
			},
			'darwin',
			automaticShellIntegration,
		);
		expect(unsupported).toMatchObject({
			kind: 'resolved',
			launch: {
				arguments: ['--name', 'two words'],
				environment: baseLaunch.environment,
				executable: '/opt/example/bin/tool',
			},
		});
	});

	it('rejects unsupported Windows login mode and relative platform paths', () => {
		expect(
			resolveTerminalLaunchProfile(
				baseLaunch,
				{
					command: { kind: 'system-shell', login: true },
					id: DesktopTerminalLaunchProfileIdSchema.parse(
						DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID,
					),
					name: null,
				},
				'win32',
			),
		).toEqual({ kind: 'rejected', reason: 'unsupported-platform' });
		expect(
			resolveTerminalLaunchProfile(
				baseLaunch,
				{
					command: { arguments: [], executable: 'bin/tool', kind: 'custom-executable' },
					id: DesktopTerminalLaunchProfileIdSchema.parse('profile-relative'),
					name: 'Relative',
				},
				'linux',
			),
		).toEqual({ kind: 'rejected', reason: 'invalid-profile' });
	});
});
