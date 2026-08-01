import path from 'node:path';

import { describe, expect, it } from 'vitest';

import { resolveDesktopRuntimeConfiguration } from '../resolveDesktopRuntimeConfiguration';

describe('resolveDesktopRuntimeConfiguration', () => {
	it('resolves the explicit Slice 5 development Host and a stable scoped creation identity', () => {
		const input = {
			applicationPath: path.resolve('/repo/apps/desktop'),
			environment: {
				ORCHBEE_DESKTOP_ENDPOINT_ID: 'ab'.repeat(16),
				ORCHBEE_DESKTOP_RUNTIME_DIRECTORY: path.resolve('/private/runtime'),
				ORCHBEE_TERMINAL_HOST_BINARY: path.resolve('/private/bin/terminal-host'),
				SHELL: '/bin/zsh',
			},
			homeDirectory: path.resolve('/users/orchbee'),
			platform: 'darwin' as const,
			userDataPath: path.resolve('/users/orchbee/library/orchbee'),
		};
		const first = resolveDesktopRuntimeConfiguration(input);
		const second = resolveDesktopRuntimeConfiguration(input);

		expect(first).toEqual(second);
		expect(first.binaryPath).toBe(path.resolve('/private/bin/terminal-host'));
		expect(first.runtimeDirectory).toBe(path.resolve('/private/runtime'));
		expect(first.createTerminal.launch).toMatchObject({
			currentDirectory: path.resolve('/users/orchbee'),
			executable: '/bin/zsh',
		});
		expect(first.createTerminal.launch.terminalProfile.palette).toHaveLength(256);
	});

	it('uses the built sibling Host and platform shell defaults when no override exists', () => {
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: path.resolve('/repo/apps/desktop'),
			environment: {},
			homeDirectory: path.resolve('/users/orchbee'),
			platform: 'linux',
			userDataPath: path.resolve('/users/orchbee/.config/orchbee'),
		});

		expect(configuration.binaryPath).toBe(
			path.resolve('/repo/apps/terminal-host/zig-out/bin/terminal-host'),
		);
		expect(configuration.createTerminal.launch.executable).toBe('/bin/sh');
		expect(configuration.endpointId).toMatch(/^[0-9a-f]{32}$/u);
		expect(configuration.runtimeDirectory).toMatch(/^\/tmp\/orchbee-desktop-[0-9a-f]{12}$/u);
	});
});
