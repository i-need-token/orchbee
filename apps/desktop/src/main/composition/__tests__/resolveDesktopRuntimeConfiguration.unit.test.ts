import { accessSync } from 'node:fs';
import type * as NodeFileSystem from 'node:fs';
import path from 'node:path';

import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
	resolveDesktopRuntimeConfiguration as resolveRuntimeConfiguration,
	type DesktopRuntimeConfigurationInput,
} from '../resolveDesktopRuntimeConfiguration';

vi.mock('node:fs', async (importOriginal) => ({
	...(await importOriginal<typeof NodeFileSystem>()),
	accessSync: vi.fn<typeof accessSync>(),
}));

beforeEach(() => {
	vi.mocked(accessSync).mockImplementation(() => {
		throw new Error('Test resource is unavailable.');
	});
});

function resolveDesktopRuntimeConfiguration(
	input: Omit<DesktopRuntimeConfigurationInput, 'terminalHostName'>,
) {
	return resolveRuntimeConfiguration({ ...input, terminalHostName: 'orchbee-host' });
}

describe('resolveDesktopRuntimeConfiguration', () => {
	it('resolves the explicit Host, binding store, and launch profile', () => {
		const input = {
			applicationPath: path.posix.resolve('/repo/apps/desktop'),
			environment: {
				ORCHBEE_DESKTOP_ENDPOINT_ID: 'ab'.repeat(16),
				ORCHBEE_DESKTOP_RECOVERY_DIRECTORY: path.posix.resolve('/private/recovery'),
				ORCHBEE_DESKTOP_RUNTIME_DIRECTORY: path.posix.resolve('/private/runtime'),
				ORCHBEE_TERMINAL_HOST_BINARY: path.posix.resolve('/private/bin/orchbee-terminal-host'),
				SHELL: '/bin/zsh',
			},
			homeDirectory: path.posix.resolve('/users/orchbee'),
			platform: 'darwin' as const,
			userDataPath: path.posix.resolve('/users/orchbee/library/orchbee'),
		};
		const first = resolveDesktopRuntimeConfiguration(input);
		const second = resolveDesktopRuntimeConfiguration(input);

		expect(first).toEqual(second);
		expect(first.binaryPath).toBe(path.posix.resolve('/private/bin/orchbee-terminal-host'));
		expect(first.runtimeDirectory).toBe(path.posix.resolve('/private/runtime'));
		expect(first.managedWorktreeRoot).toBe(
			path.posix.resolve('/users/orchbee/library/orchbee/worktrees'),
		);
		expect(first.agentObservationCapsuleDirectory).toBe(
			path.posix.resolve('/users/orchbee/library/orchbee/agent-observation-adapters/0.4.12'),
		);
		expect(first.projectWorktreeOperationStorePath).toBe(
			path.posix.resolve('/users/orchbee/library/orchbee/project-worktree-operations-v1.json'),
		);
		expect(first.browserAccess).toEqual({ kind: 'disabled' });
		expect(first.environmentRegistryStorePath).toBe(
			path.posix.resolve('/users/orchbee/library/orchbee/environment-registry-v2.json'),
		);
		expect(first.environmentRegistryLegacyStorePath).toBe(
			path.posix.resolve('/users/orchbee/library/orchbee/environment-registry-v1.json'),
		);
		expect(first.terminalBindingStorePath).toBe(
			path.posix.resolve('/private/recovery/bindings-v3.json'),
		);
		expect(first.terminalArchiveDirectory).toBe(
			path.posix.resolve('/private/recovery/terminal-archives-v1'),
		);
		expect(first.terminalLaunchProfileStorePath).toBe(
			path.posix.resolve('/users/orchbee/library/orchbee/terminal-launch-profiles-v1.json'),
		);
		expect(first.terminalDefaultSettingsStorePath).toBe(
			path.posix.resolve('/users/orchbee/library/orchbee/terminal-default-settings-v1.json'),
		);
		expect(first.terminalExitPublicationLedgerPath).toBe(
			path.posix.resolve('/private/recovery/terminal-exit-publications-v1.json'),
		);
		expect(first.workbenchDocumentStorePath).toBe(
			path.posix.resolve('/private/recovery/workbench-shared-document-v2.json'),
		);
		expect(first.workbenchDocumentLegacyStorePath).toBe(
			path.posix.resolve('/private/recovery/workbench-shared-document-v1.json'),
		);
		expect(first.windowStateStorePath).toBe(
			path.posix.resolve('/users/orchbee/library/orchbee/window-state-v1.json'),
		);
		expect(first.terminalLaunch).toMatchObject({
			cellPixelHeight: null,
			cellPixelWidth: null,
			columns: null,
			configuration: null,
			currentDirectory: path.posix.resolve('/users/orchbee'),
			executable: '/bin/zsh',
			rows: null,
		});
		expect(first.terminalConfiguration).toEqual({ scrollbackBytes: 10_000_000 });
		expect(first.terminalHostName).toBe('orchbee-host');
		expect(first.terminalLaunch.terminalProfile.palette).toHaveLength(256);
	});

	it('enables only the explicit Browser access resource set in development', () => {
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: path.posix.resolve('/repo/apps/desktop'),
			environment: {
				ORCHBEE_DESKTOP_GATEWAY: '1',
				ORCHBEE_GATEWAY_ENTRY_PATH: path.posix.resolve('/private/gateway.cjs'),
				ORCHBEE_SERVER_ENTRY_PATH: path.posix.resolve('/private/server.cjs'),
				ORCHBEE_WEB_RENDERER_ROOT: path.posix.resolve('/private/web'),
			},
			homeDirectory: path.posix.resolve('/users/orchbee'),
			platform: 'darwin',
			posixRuntimeBaseDirectory: path.posix.resolve('/private/var/folders/user/T'),
			userDataPath: path.posix.resolve('/users/orchbee/library/orchbee'),
		});

		expect(configuration.browserAccess).toEqual({
			gatewayEntryPath: path.posix.resolve('/private/gateway.cjs'),
			kind: 'enabled',
			personalAccessTokenStorePath: path.posix.resolve(
				'/users/orchbee/library/orchbee/personal-access-token-v1.json',
			),
			rendererRoot: path.posix.resolve('/private/web'),
			serverEntryPath: path.posix.resolve('/private/server.cjs'),
		});
		expect(() =>
			resolveDesktopRuntimeConfiguration({
				applicationPath: path.posix.resolve('/repo/apps/desktop'),
				environment: { ORCHBEE_DESKTOP_GATEWAY: 'yes' },
				homeDirectory: path.posix.resolve('/users/orchbee'),
				platform: 'darwin',
				posixRuntimeBaseDirectory: path.posix.resolve('/private/var/folders/user/T'),
				userDataPath: path.posix.resolve('/users/orchbee/library/orchbee'),
			}),
		).toThrow('ORCHBEE_DESKTOP_GATEWAY must be 0 or 1.');
	});

	it('uses the built sibling Host and platform shell defaults when no override exists', () => {
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: path.posix.resolve('/repo/apps/desktop'),
			environment: {},
			homeDirectory: path.posix.resolve('/users/orchbee'),
			platform: 'linux',
			posixRuntimeBaseDirectory: path.posix.resolve('/run/user/501'),
			userDataPath: path.posix.resolve('/users/orchbee/.config/orchbee'),
		});

		expect(configuration.binaryPath).toBe(
			path.posix.resolve('/repo/apps/terminal-host/zig-out/bin/orchbee-terminal-host'),
		);
		expect(configuration.posixHostEndpointAddonPath).toBe(
			path.posix.resolve('/repo/apps/desktop/zig-out/lib/orchbee-posix-host-endpoint.node'),
		);
		expect(configuration.terminalLaunch.executable).toBe('/bin/sh');
		expect(configuration.endpointId).toMatch(/^[0-9a-f]{32}$/u);
		expect(configuration.terminalBindingStorePath).toBe(
			path.posix.resolve('/users/orchbee/.config/orchbee/terminal-recovery/bindings-v3.json'),
		);
		expect(configuration.terminalArchiveDirectory).toBe(
			path.posix.resolve('/users/orchbee/.config/orchbee/terminal-recovery/terminal-archives-v1'),
		);
		expect(configuration.workbenchDocumentStorePath).toBe(
			path.posix.resolve(
				'/users/orchbee/.config/orchbee/terminal-recovery/workbench-shared-document-v2.json',
			),
		);
		expect(configuration.workbenchDocumentLegacyStorePath).toBe(
			path.posix.resolve(
				'/users/orchbee/.config/orchbee/terminal-recovery/workbench-shared-document-v1.json',
			),
		);
		expect(configuration.windowStateStorePath).toBe(
			path.posix.resolve('/users/orchbee/.config/orchbee/window-state-v1.json'),
		);
		expect(configuration.runtimeDirectory).toMatch(/^\/run\/user\/501\/ob-[0-9a-f]{12}$/u);
		expect(configuration.runtimeDirectory).not.toMatch(/^\/(?:private\/)?tmp(?:\/|$)/u);
		expect(configuration.terminalHostLaunchMode).toBe('connect-or-launch');
	});

	it('uses connect-only mode for an explicitly supervised development Host', () => {
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: path.posix.resolve('/repo/apps/desktop'),
			environment: { ORCHBEE_TERMINAL_HOST_SUPERVISED: '1' },
			homeDirectory: path.posix.resolve('/users/orchbee'),
			platform: 'darwin',
			posixRuntimeBaseDirectory: path.posix.resolve('/private/var/folders/user/T'),
			userDataPath: path.posix.resolve('/users/orchbee/library/orchbee'),
		});

		expect(configuration.terminalHostLaunchMode).toBe('connect-only');
		expect(() =>
			resolveDesktopRuntimeConfiguration({
				applicationPath: path.posix.resolve('/repo/apps/desktop'),
				environment: { ORCHBEE_TERMINAL_HOST_SUPERVISED: 'yes' },
				homeDirectory: path.posix.resolve('/users/orchbee'),
				platform: 'darwin',
				posixRuntimeBaseDirectory: path.posix.resolve('/private/var/folders/user/T'),
				userDataPath: path.posix.resolve('/users/orchbee/library/orchbee'),
			}),
		).toThrow('ORCHBEE_TERMINAL_HOST_SUPERVISED must be 0 or 1.');
	});

	it('keeps the macOS product rendezvous stable beneath its trusted per-user base', () => {
		const input = {
			applicationPath: path.posix.resolve('/repo/apps/desktop'),
			environment: {},
			homeDirectory: path.posix.resolve('/users/orchbee'),
			platform: 'darwin' as const,
			posixRuntimeBaseDirectory: path.posix.resolve('/private/var/folders/user/T'),
			userDataPath: path.posix.resolve('/users/orchbee/library/orchbee'),
		};
		const first = resolveDesktopRuntimeConfiguration(input);
		const second = resolveDesktopRuntimeConfiguration(input);

		expect(first.runtimeDirectory).toBe(second.runtimeDirectory);
		expect(first.runtimeDirectory).toMatch(/^\/private\/var\/folders\/user\/T\/ob-[0-9a-f]{12}$/u);
	});

	it('uses the packaged Host resource outside the application archive', () => {
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: path.posix.resolve('/Applications/Orchbee.app/Contents/Resources/app.asar'),
			environment: {},
			homeDirectory: path.posix.resolve('/users/orchbee'),
			packagedResourcesPath: path.posix.resolve('/Applications/Orchbee.app/Contents/Resources'),
			platform: 'darwin',
			posixRuntimeBaseDirectory: path.posix.resolve('/private/var/folders/user/T'),
			userDataPath: path.posix.resolve('/users/orchbee/library/orchbee'),
		});

		expect(configuration.binaryPath).toBe(
			path.posix.resolve('/Applications/Orchbee.app/Contents/Resources/orchbee-terminal-host'),
		);
		expect(configuration.posixHostEndpointAddonPath).toBe(
			path.posix.resolve(
				'/Applications/Orchbee.app/Contents/Resources/orchbee-posix-host-endpoint.node',
			),
		);
		expect(configuration.browserAccess).toEqual({
			gatewayEntryPath: path.posix.resolve(
				'/Applications/Orchbee.app/Contents/Resources/orchbee-gateway.cjs',
			),
			kind: 'enabled',
			personalAccessTokenStorePath: path.posix.resolve(
				'/users/orchbee/library/orchbee/personal-access-token-v1.json',
			),
			rendererRoot: path.posix.resolve('/Applications/Orchbee.app/Contents/Resources/dist'),
			serverEntryPath: path.posix.resolve(
				'/Applications/Orchbee.app/Contents/Resources/orchbee-server.cjs',
			),
		});
		expect(configuration.terminalLaunch.environment).toContainEqual({
			name: 'ORCHBEE_SHELL_INTEGRATION_DIR',
			value: path.posix.resolve('/Applications/Orchbee.app/Contents/Resources/shell-integration'),
		});
		expect(configuration.terminalLaunch.environment).toContainEqual({
			name: 'ORCHBEE_CLI',
			value: path.posix.resolve('/Applications/Orchbee.app/Contents/Resources/orchbee-cli'),
		});
		expect(configuration.terminalLaunch.environment).toContainEqual({
			name: 'ORCHBEE_CLI_RUNTIME_DIRECTORY',
			value: configuration.runtimeDirectory,
		});
	});

	it('accepts the bundle-resolved development shell integration directory', () => {
		const developmentPosixHostEndpointAddonPath = path.posix.resolve(
			'/repo/apps/desktop/zig-out/lib/orchbee-posix-host-endpoint.node',
		);
		const developmentShellIntegrationDirectory = path.posix.resolve(
			'/repo/apps/desktop/resources/shell-integration',
		);
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: path.posix.resolve('/unexpected/electron/application/path'),
			developmentPosixHostEndpointAddonPath,
			developmentShellIntegrationDirectory,
			environment: {},
			homeDirectory: path.posix.resolve('/users/orchbee'),
			platform: 'darwin',
			posixRuntimeBaseDirectory: path.posix.resolve('/private/var/folders/user/T'),
			userDataPath: path.posix.resolve('/users/orchbee/library/orchbee'),
		});
		expect(configuration.posixHostEndpointAddonPath).toBe(developmentPosixHostEndpointAddonPath);

		expect(configuration.terminalLaunch.environment).toContainEqual({
			name: 'ORCHBEE_SHELL_INTEGRATION_DIR',
			value: developmentShellIntegrationDirectory,
		});
	});

	it('admits only readable automatic loaders and captures the Main environment exactly', () => {
		const shellIntegrationDirectory = '/repo/apps/desktop/resources/shell-integration';
		const readablePaths = new Set([
			`${shellIntegrationDirectory}/orchbee.bash`,
			`${shellIntegrationDirectory}/automatic/bash/orchbee.bash`,
			`${shellIntegrationDirectory}/orchbee.fish`,
			`${shellIntegrationDirectory}/automatic/fish/fish/vendor_conf.d/orchbee-shell-integration.fish`,
			`${shellIntegrationDirectory}/orchbee.zsh`,
			`${shellIntegrationDirectory}/automatic/zsh/.zshenv`,
		]);
		vi.mocked(accessSync).mockImplementation((resourcePath) => {
			if (typeof resourcePath !== 'string' || !readablePaths.has(resourcePath)) {
				throw new Error('Test resource is unavailable.');
			}
		});
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: path.posix.resolve('/repo/apps/desktop'),
			developmentShellIntegrationDirectory: shellIntegrationDirectory,
			environment: {
				XDG_DATA_DIRS: '/opt/share:/usr/share',
				ZDOTDIR: '/users/orchbee/.config/zsh',
			},
			homeDirectory: path.posix.resolve('/users/orchbee'),
			platform: 'darwin',
			posixRuntimeBaseDirectory: path.posix.resolve('/private/var/folders/user/T'),
			userDataPath: path.posix.resolve('/users/orchbee/library/orchbee'),
		});

		expect(configuration.automaticShellIntegration).toEqual({
			availableShells: ['bash', 'fish', 'zsh'],
			directory: shellIntegrationDirectory,
			inheritedEnvironment: {
				xdgDataDirectories: '/opt/share:/usr/share',
				zdotdir: '/users/orchbee/.config/zsh',
			},
		});
	});

	it('keeps automatic integration disabled when the trusted loaders are unavailable', () => {
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: path.posix.resolve('/repo/apps/desktop'),
			developmentShellIntegrationDirectory: path.posix.resolve('/missing/shell-integration'),
			environment: {},
			homeDirectory: path.posix.resolve('/users/orchbee'),
			platform: 'linux',
			posixRuntimeBaseDirectory: path.posix.resolve('/run/user/501'),
			userDataPath: path.posix.resolve('/users/orchbee/.config/orchbee'),
		});

		expect(configuration.automaticShellIntegration).toBeNull();
		expect(configuration.terminalLaunch.environment).toContainEqual({
			name: 'ORCHBEE_SHELL_INTEGRATION_DIR',
			value: path.posix.resolve('/missing/shell-integration'),
		});
	});

	it('projects an authoritative Darwin UTF-8 locale when the Desktop locale is absent', () => {
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: path.posix.resolve('/repo/apps/desktop'),
			environment: {},
			homeDirectory: path.posix.resolve('/users/orchbee'),
			platform: 'darwin',
			posixRuntimeBaseDirectory: path.posix.resolve('/private/var/folders/user/T'),
			userDataPath: path.posix.resolve('/users/orchbee/library/orchbee'),
		});

		expect(
			configuration.terminalLaunch.environment.filter(
				(entry) => entry.name === 'LANG' || entry.name.startsWith('LC_'),
			),
		).toEqual([
			{ name: 'LC_ALL', value: null },
			{ name: 'LANG', value: 'en_US.UTF-8' },
			{ name: 'LC_CTYPE', value: null },
		]);
	});

	it('derives a Darwin LANG from the native system locale', () => {
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: path.posix.resolve('/repo/apps/desktop'),
			darwinSupportedLocales: ['en_US.UTF-8', 'zh_CN.UTF-8'],
			darwinSystemLocale: 'zh-Hans-CN',
			environment: {},
			homeDirectory: path.posix.resolve('/users/orchbee'),
			platform: 'darwin',
			posixRuntimeBaseDirectory: path.posix.resolve('/private/var/folders/user/T'),
			userDataPath: path.posix.resolve('/users/orchbee/library/orchbee'),
		});

		expect(configuration.terminalLaunch.environment).toContainEqual({
			name: 'LANG',
			value: 'zh_CN.UTF-8',
		});
	});

	it('falls back when the derived Darwin locale is not installed', () => {
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: path.posix.resolve('/repo/apps/desktop'),
			darwinSupportedLocales: ['en_US.UTF-8', 'zh_CN.UTF-8'],
			darwinSystemLocale: 'zh-US',
			environment: {},
			homeDirectory: path.posix.resolve('/users/orchbee'),
			platform: 'darwin',
			posixRuntimeBaseDirectory: path.posix.resolve('/private/var/folders/user/T'),
			userDataPath: path.posix.resolve('/users/orchbee/library/orchbee'),
		});

		expect(configuration.terminalLaunch.environment).toContainEqual({
			name: 'LANG',
			value: 'en_US.UTF-8',
		});
	});

	it('falls back when the Darwin system locale has no usable country', () => {
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: path.posix.resolve('/repo/apps/desktop'),
			darwinSystemLocale: 'en',
			environment: {},
			homeDirectory: path.posix.resolve('/users/orchbee'),
			platform: 'darwin',
			posixRuntimeBaseDirectory: path.posix.resolve('/private/var/folders/user/T'),
			userDataPath: path.posix.resolve('/users/orchbee/library/orchbee'),
		});

		expect(configuration.terminalLaunch.environment).toContainEqual({
			name: 'LANG',
			value: 'en_US.UTF-8',
		});
	});

	it('supplies a Darwin LANG even when a valid LC_ALL controls the local locale', () => {
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: path.posix.resolve('/repo/apps/desktop'),
			darwinSupportedLocales: ['en_US.UTF-8'],
			darwinSystemLocale: 'en-US',
			environment: { LC_ALL: 'zh_CN.UTF-8' },
			homeDirectory: path.posix.resolve('/users/orchbee'),
			platform: 'darwin',
			posixRuntimeBaseDirectory: path.posix.resolve('/private/var/folders/user/T'),
			userDataPath: path.posix.resolve('/users/orchbee/library/orchbee'),
		});

		expect(
			configuration.terminalLaunch.environment.filter(
				(entry) => entry.name === 'LANG' || entry.name.startsWith('LC_'),
			),
		).toEqual([
			{ name: 'LC_ALL', value: 'zh_CN.UTF-8' },
			{ name: 'LANG', value: 'en_US.UTF-8' },
			{ name: 'LC_CTYPE', value: null },
		]);
	});

	it('keeps the Linux UTF-8 character-type fallback without inventing LANG', () => {
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: path.posix.resolve('/repo/apps/desktop'),
			environment: {},
			homeDirectory: path.posix.resolve('/users/orchbee'),
			platform: 'linux',
			posixRuntimeBaseDirectory: path.posix.resolve('/run/user/501'),
			userDataPath: path.posix.resolve('/users/orchbee/.config/orchbee'),
		});

		expect(
			configuration.terminalLaunch.environment.filter(
				(entry) => entry.name === 'LANG' || entry.name.startsWith('LC_'),
			),
		).toEqual([{ name: 'LC_CTYPE', value: 'C.UTF-8' }]);
	});

	it('repairs an unsupported Darwin UTF-8 locale before launching the shell', () => {
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: path.posix.resolve('/repo/apps/desktop'),
			environment: {
				LANG: 'en_US.UTF-8',
				LC_ALL: 'C.UTF-8',
				LC_CTYPE: 'C.UTF-8',
			},
			homeDirectory: path.posix.resolve('/users/orchbee'),
			platform: 'darwin',
			posixRuntimeBaseDirectory: path.posix.resolve('/private/var/folders/user/T'),
			userDataPath: path.posix.resolve('/users/orchbee/library/orchbee'),
		});

		expect(configuration.terminalLaunch.environment).toEqual([
			{ name: 'COLORTERM', value: 'truecolor' },
			{ name: 'TERM', value: 'xterm-256color' },
			{ name: 'TERM_PROGRAM', value: 'ghostty' },
			{
				name: 'ORCHBEE_CLI',
				value: path.posix.resolve('/repo/apps/cli/resources/orchbee-cli'),
			},
			{ name: 'ORCHBEE_CLI_RUNTIME_DIRECTORY', value: configuration.runtimeDirectory },
			{
				name: 'ORCHBEE_SHELL_INTEGRATION_DIR',
				value: path.posix.resolve('/repo/apps/desktop/resources/shell-integration'),
			},
			{ name: 'NO_COLOR', value: null },
			{ name: 'FORCE_COLOR', value: null },
			{ name: 'NODE_DISABLE_COLORS', value: null },
			{ name: 'LC_ALL', value: null },
			{ name: 'LANG', value: 'en_US.UTF-8' },
			{ name: 'LC_CTYPE', value: 'en_US.UTF-8' },
		]);
	});

	it('repairs a Darwin C.UTF-8 LANG even when LC_ALL is absent', () => {
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: path.posix.resolve('/repo/apps/desktop'),
			environment: { LANG: 'C.UTF-8' },
			homeDirectory: path.posix.resolve('/users/orchbee'),
			platform: 'darwin',
			posixRuntimeBaseDirectory: path.posix.resolve('/private/var/folders/user/T'),
			userDataPath: path.posix.resolve('/users/orchbee/library/orchbee'),
		});

		expect(configuration.terminalLaunch.environment).toEqual([
			{ name: 'COLORTERM', value: 'truecolor' },
			{ name: 'TERM', value: 'xterm-256color' },
			{ name: 'TERM_PROGRAM', value: 'ghostty' },
			{
				name: 'ORCHBEE_CLI',
				value: path.posix.resolve('/repo/apps/cli/resources/orchbee-cli'),
			},
			{ name: 'ORCHBEE_CLI_RUNTIME_DIRECTORY', value: configuration.runtimeDirectory },
			{
				name: 'ORCHBEE_SHELL_INTEGRATION_DIR',
				value: path.posix.resolve('/repo/apps/desktop/resources/shell-integration'),
			},
			{ name: 'NO_COLOR', value: null },
			{ name: 'FORCE_COLOR', value: null },
			{ name: 'NODE_DISABLE_COLORS', value: null },
			{ name: 'LC_ALL', value: null },
			{ name: 'LANG', value: 'en_US.UTF-8' },
			{ name: 'LC_CTYPE', value: null },
		]);
	});

	it('preserves an explicit Darwin UTF-8 locale', () => {
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: path.posix.resolve('/repo/apps/desktop'),
			environment: {
				LANG: 'en_US.UTF-8',
				LC_ALL: 'zh_CN.UTF-8',
				LC_CTYPE: 'en_US.UTF-8',
			},
			homeDirectory: path.posix.resolve('/users/orchbee'),
			platform: 'darwin',
			posixRuntimeBaseDirectory: path.posix.resolve('/private/var/folders/user/T'),
			userDataPath: path.posix.resolve('/users/orchbee/library/orchbee'),
		});

		expect(configuration.terminalLaunch.environment).toEqual([
			{ name: 'COLORTERM', value: 'truecolor' },
			{ name: 'TERM', value: 'xterm-256color' },
			{ name: 'TERM_PROGRAM', value: 'ghostty' },
			{
				name: 'ORCHBEE_CLI',
				value: path.posix.resolve('/repo/apps/cli/resources/orchbee-cli'),
			},
			{ name: 'ORCHBEE_CLI_RUNTIME_DIRECTORY', value: configuration.runtimeDirectory },
			{
				name: 'ORCHBEE_SHELL_INTEGRATION_DIR',
				value: path.posix.resolve('/repo/apps/desktop/resources/shell-integration'),
			},
			{ name: 'NO_COLOR', value: null },
			{ name: 'FORCE_COLOR', value: null },
			{ name: 'NODE_DISABLE_COLORS', value: null },
			{ name: 'LC_ALL', value: 'zh_CN.UTF-8' },
			{ name: 'LANG', value: 'en_US.UTF-8' },
			{ name: 'LC_CTYPE', value: 'en_US.UTF-8' },
		]);
	});

	it('uses a stable product runtime directory on Windows', () => {
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: 'C:\\repo\\apps\\desktop',
			environment: {},
			homeDirectory: 'C:\\Users\\orchbee',
			platform: 'win32',
			userDataPath: 'C:\\Users\\orchbee\\AppData\\Roaming\\Orchbee',
		});

		expect(configuration.runtimeDirectory).toBe(
			path.win32.join('C:\\Users\\orchbee\\AppData\\Roaming\\Orchbee', 'terminal-host'),
		);
		expect(configuration.posixHostEndpointAddonPath).toBeNull();
	});

	it('sets the Ghostty compatibility identity and removes inherited parent color policy', () => {
		const configuration = resolveDesktopRuntimeConfiguration({
			applicationPath: 'C:\\repo\\apps\\desktop',
			environment: {
				FORCE_COLOR: '0',
				NODE_DISABLE_COLORS: '1',
				NO_COLOR: '1',
			},
			homeDirectory: 'C:\\Users\\orchbee',
			platform: 'win32',
			userDataPath: 'C:\\Users\\orchbee\\AppData\\Roaming\\Orchbee',
		});

		expect(configuration.terminalLaunch.environment).toEqual([
			{ name: 'COLORTERM', value: 'truecolor' },
			{ name: 'TERM', value: 'xterm-256color' },
			{ name: 'TERM_PROGRAM', value: 'ghostty' },
			{
				name: 'ORCHBEE_CLI',
				value: 'C:\\repo\\apps\\cli\\resources\\orchbee-cli.cmd',
			},
			{ name: 'ORCHBEE_CLI_RUNTIME_DIRECTORY', value: configuration.runtimeDirectory },
			{ name: 'NO_COLOR', value: null },
			{ name: 'FORCE_COLOR', value: null },
			{ name: 'NODE_DISABLE_COLORS', value: null },
		]);
	});

	it('scopes the singleton endpoint to the product profile and validates the startup default', () => {
		const first = resolveDesktopRuntimeConfiguration({
			applicationPath: '/repo/apps/desktop',
			environment: { ORCHBEE_TERMINAL_SCROLLBACK_BYTES: '25000000' },
			homeDirectory: '/users/orchbee',
			platform: 'linux',
			posixRuntimeBaseDirectory: path.posix.resolve('/run/user/501'),
			userDataPath: '/users/orchbee/.config/orchbee',
		});
		const second = resolveDesktopRuntimeConfiguration({
			applicationPath: '/repo/apps/desktop',
			environment: {},
			homeDirectory: '/users/orchbee',
			platform: 'linux',
			posixRuntimeBaseDirectory: path.posix.resolve('/run/user/501'),
			userDataPath: '/users/orchbee/.config/orchbee-beta',
		});

		expect(first.endpointId).not.toBe(second.endpointId);
		expect(first.runtimeDirectory).not.toBe(second.runtimeDirectory);
		expect(first.terminalConfiguration).toEqual({ scrollbackBytes: 25_000_000 });
		expect(() =>
			resolveDesktopRuntimeConfiguration({
				applicationPath: '/repo/apps/desktop',
				environment: { ORCHBEE_TERMINAL_SCROLLBACK_BYTES: '1000000001' },
				homeDirectory: '/users/orchbee',
				platform: 'linux',
				posixRuntimeBaseDirectory: path.posix.resolve('/run/user/501'),
				userDataPath: '/users/orchbee/.config/orchbee',
			}),
		).toThrow('exceeds the supported range');
	});

	it.each([
		'/tmp',
		'/tmp/',
		'/tmp/user-controlled',
		'/private/tmp',
		'/private/tmp/user-controlled',
		'/private/var/../tmp/user-controlled',
	])('rejects a deterministic default rendezvous beneath %s', (runtimeBase) => {
		expect(() =>
			resolveDesktopRuntimeConfiguration({
				applicationPath: '/repo/apps/desktop',
				environment: {},
				homeDirectory: '/users/orchbee',
				platform: 'darwin',
				posixRuntimeBaseDirectory: runtimeBase,
				userDataPath: '/users/orchbee/library/orchbee',
			}),
		).toThrow('must not be under shared temporary storage');
	});
	it.each(['/tmp-user', '/private/tmp-user'])(
		'does not reject a neighboring trusted base %s',
		(runtimeBase) => {
			const configuration = resolveDesktopRuntimeConfiguration({
				applicationPath: '/repo/apps/desktop',
				environment: {},
				homeDirectory: '/users/orchbee',
				platform: 'darwin',
				posixRuntimeBaseDirectory: runtimeBase,
				userDataPath: '/users/orchbee/library/orchbee',
			});
			expect(path.posix.dirname(configuration.runtimeDirectory)).toBe(runtimeBase);
		},
	);

	it.each(['C:\\Program Files\\Orchbee', '\\\\server\\share\\Orchbee'])(
		'keeps packaged Windows resources under %s',
		(resourcesPath) => {
			const configuration = resolveDesktopRuntimeConfiguration({
				applicationPath: path.win32.join(resourcesPath, 'app.asar'),
				environment: { ComSpec: 'C:\\Windows\\System32\\cmd.exe' },
				homeDirectory: 'C:\\Users\\orchbee',
				packagedResourcesPath: resourcesPath,
				platform: 'win32',
				userDataPath: 'C:\\Users\\orchbee\\AppData\\Roaming\\Orchbee',
			});
			expect(configuration.binaryPath).toBe(`${resourcesPath}\\orchbee-terminal-host.exe`);
			expect(configuration.terminalLaunch.environment).toContainEqual({
				name: 'ORCHBEE_CLI',
				value: `${resourcesPath}\\orchbee-cli.cmd`,
			});
			expect(configuration.terminalLaunch.executable).toBe('C:\\Windows\\System32\\cmd.exe');
			expect(configuration.automaticShellIntegration).toBeNull();
			expect(configuration.posixHostEndpointAddonPath).toBeNull();
			expect(configuration.browserAccess).toMatchObject({
				gatewayEntryPath: `${resourcesPath}\\orchbee-gateway.cjs`,
				rendererRoot: `${resourcesPath}\\dist`,
				serverEntryPath: `${resourcesPath}\\orchbee-server.cjs`,
			});
		},
	);
});
