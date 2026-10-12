import {
	DesktopTerminalLaunchProfileIdSchema,
	type DesktopTerminalLaunchProfileCatalog,
} from '@orchbee/desktop-client-protocol';
import { TerminalLaunchSchema } from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import { createDefaultTerminalProfile } from '../../host';
import { DesktopTerminalLaunchProfileController } from '../DesktopTerminalLaunchProfileController';
import type { AutomaticShellIntegrationConfiguration } from '../resolveAutomaticShellIntegration';
import { createBuiltInTerminalLaunchProfileCatalog } from '../terminal-launch-profile-catalog';
import type {
	TerminalLaunchProfileStore,
	TerminalLaunchProfileStoreLoadResult,
} from '../terminal-launch-profile-store';

const baseLaunch = TerminalLaunchSchema.parse({
	arguments: [],
	cellPixelHeight: 16,
	cellPixelWidth: 8,
	columns: 80,
	configuration: null,
	currentDirectory: '/users/orchbee',
	environment: [{ name: 'TERM', value: 'xterm-256color' }],
	executable: '/bin/zsh',
	rows: 24,
	terminalProfile: createDefaultTerminalProfile(),
});

const automaticShellIntegration: AutomaticShellIntegrationConfiguration = {
	availableShells: ['zsh'],
	directory: '/app/shell-integration',
	inheritedEnvironment: {
		xdgDataDirectories: null,
		zdotdir: null,
	},
};

describe('DesktopTerminalLaunchProfileController', () => {
	it('loads and launches a custom profile without a discoverable system shell', async () => {
		const builtIn = createBuiltInTerminalLaunchProfileCatalog();
		const catalog: DesktopTerminalLaunchProfileCatalog = {
			...builtIn,
			profiles: [
				{
					id: builtIn.defaultProfileId,
					name: null,
					command: {
						kind: 'custom-executable',
						executable: '/opt/example',
						arguments: ['literal; argument'],
					},
				},
			],
		};
		const resolveSystemShell = vi.fn<() => Promise<string | null>>(async () => null);
		const controller = new DesktopTerminalLaunchProfileController({
			allocateProfileId: () => 'created',
			automaticShellIntegration: null,
			baseLaunch: { ...baseLaunch, executable: null },
			platform: 'linux',
			resolveSystemShell,
			store: storeWith({ kind: 'available', catalog }),
		});
		await expect(controller.initialize()).resolves.toMatchObject({
			catalog,
			persistenceIssue: null,
		});
		await expect(controller.resolveDefaultLaunch()).resolves.toMatchObject({
			executable: '/opt/example',
			arguments: ['literal; argument'],
		});
		expect(resolveSystemShell).not.toHaveBeenCalled();
	});

	it('discovers the system shell for each new launch and refuses unavailable discovery', async () => {
		const resolveSystemShell = vi
			.fn<() => Promise<string | null>>()
			.mockResolvedValueOnce('/bin/bash')
			.mockResolvedValueOnce('/bin/zsh')
			.mockResolvedValue(null);
		const controller = new DesktopTerminalLaunchProfileController({
			allocateProfileId: () => 'created',
			automaticShellIntegration: null,
			baseLaunch: { ...baseLaunch, executable: null },
			platform: 'linux',
			resolveSystemShell,
			store: storeWith({ kind: 'missing' }),
		});
		await controller.initialize();
		await expect(controller.resolveDefaultLaunch()).resolves.toMatchObject({
			executable: '/bin/bash',
		});
		await expect(controller.resolveDefaultLaunch()).resolves.toMatchObject({
			executable: '/bin/zsh',
		});
		await expect(controller.resolveDefaultLaunch()).rejects.toThrow('system shell is unavailable');
	});
	it.each([
		{ arguments: [], login: false, platform: 'darwin' },
		{ arguments: [], login: false, platform: 'linux' },
		{ arguments: [], login: false, platform: 'win32' },
	] satisfies readonly {
		readonly arguments: readonly string[];
		readonly login: boolean;
		readonly platform: NodeJS.Platform;
	}[])('loads the $platform built-in profile when storage is missing', async (expected) => {
		const controller = createController(storeWith({ kind: 'missing' }), expected.platform);

		await expect(controller.initialize()).resolves.toMatchObject({
			catalog: {
				defaultProfileId: 'system-default',
				profiles: [{ command: { kind: 'system-shell', login: expected.login } }],
			},
			persistenceIssue: null,
			source: 'built-in',
		});
		expect(await controller.resolveDefaultLaunch()).toEqual({
			...baseLaunch,
			arguments: expected.arguments,
		});
	});

	it('keeps a stored non-login choice authoritative on Darwin', async () => {
		const stored = createBuiltInTerminalLaunchProfileCatalog();
		const replace = vi.fn<TerminalLaunchProfileStore['replace']>();
		const controller = createController(
			{ load: async () => ({ catalog: stored, kind: 'available' }), replace },
			'darwin',
		);

		await expect(controller.initialize()).resolves.toMatchObject({
			catalog: { profiles: [{ command: { kind: 'system-shell', login: false } }] },
			source: 'stored',
		});
		expect(await controller.resolveDefaultLaunch()).toEqual(baseLaunch);
		expect(replace).not.toHaveBeenCalled();
	});

	it('uses the startup-captured automatic integration policy for every new default launch', async () => {
		const controller = createController(
			storeWith({ kind: 'missing' }),
			'darwin',
			() => 'profile-created',
			automaticShellIntegration,
		);
		await controller.initialize();

		const resolved = await controller.resolveDefaultLaunch();
		expect(resolved.arguments).toEqual([]);
		expect(resolved.environment).toContainEqual({
			name: 'ZDOTDIR',
			value: '/app/shell-integration/automatic/zsh',
		});
	});

	it('resets a Darwin catalog to the common System shell only after persistence commits', async () => {
		const stored = createBuiltInTerminalLaunchProfileCatalog();
		const replace = vi.fn<TerminalLaunchProfileStore['replace']>(async () => undefined);
		const controller = createController({
			load: async () => ({ catalog: stored, kind: 'available' }),
			replace,
		});
		await controller.initialize();

		await expect(controller.mutate({ kind: 'reset' })).resolves.toMatchObject({
			kind: 'completed',
			snapshot: {
				catalog: { profiles: [{ command: { kind: 'system-shell', login: false } }] },
				source: 'stored',
			},
		});
		expect(replace).toHaveBeenCalledWith(
			expect.objectContaining({
				profiles: [expect.objectContaining({ command: { kind: 'system-shell', login: false } })],
			}),
		);
		expect(await controller.resolveDefaultLaunch()).toEqual({ ...baseLaunch, arguments: [] });
	});

	it('reports invalid stored platform behavior without rewriting it', async () => {
		const replace = vi.fn<TerminalLaunchProfileStore['replace']>();
		const invalid = {
			...createBuiltInTerminalLaunchProfileCatalog(),
			profiles: [
				{
					command: { kind: 'system-shell' as const, login: true },
					id: DesktopTerminalLaunchProfileIdSchema.parse('system-default'),
					name: null,
				},
			],
		};
		const controller = createController(
			{ load: async () => ({ catalog: invalid, kind: 'available' }), replace },
			'win32',
		);

		await expect(controller.initialize()).resolves.toMatchObject({
			persistenceIssue: 'invalid-record',
			source: 'built-in',
		});
		expect(replace).not.toHaveBeenCalled();
	});

	it('uses the shared catalog bounds independently of a future Host creation envelope', async () => {
		const stored = createBuiltInTerminalLaunchProfileCatalog();
		const replace = vi.fn<TerminalLaunchProfileStore['replace']>();
		const controller = createController({
			load: async () => ({ catalog: stored, kind: 'available' }),
			replace,
		});
		await controller.initialize();

		await expect(
			controller.mutate({
				kind: 'update',
				profile: {
					command: {
						arguments: ['a'.repeat(32 * 1024), 'b'.repeat(32 * 1024)],
						executable: '/usr/bin/example',
						kind: 'custom-executable',
					},
					id: stored.defaultProfileId,
					name: null,
				},
			}),
		).resolves.toMatchObject({ kind: 'completed' });
		expect(replace).toHaveBeenCalledOnce();
		expect(await controller.resolveDefaultLaunch()).toMatchObject({
			executable: '/usr/bin/example',
		});
	});

	it('commits a new default only after atomic persistence succeeds', async () => {
		let releaseWrite: (() => void) | undefined;
		const replace = vi.fn<TerminalLaunchProfileStore['replace']>(
			async () =>
				new Promise<void>((resolve) => {
					releaseWrite = resolve;
				}),
		);
		const stored = createBuiltInTerminalLaunchProfileCatalog();
		const controller = createController({
			load: async () => ({ catalog: stored, kind: 'available' }),
			replace,
		});
		await controller.initialize();
		const update = controller.mutate({
			kind: 'update',
			profile: {
				command: {
					arguments: ['--flag'],
					executable: '/usr/bin/example',
					kind: 'custom-executable',
				},
				id: stored.defaultProfileId,
				name: null,
			},
		});
		await vi.waitFor(() => expect(replace).toHaveBeenCalledOnce());

		expect((await controller.resolveDefaultLaunch()).executable).toBe('/bin/zsh');
		releaseWrite?.();
		await expect(update).resolves.toMatchObject({ kind: 'completed' });
		expect(await controller.resolveDefaultLaunch()).toMatchObject({
			arguments: ['--flag'],
			executable: '/usr/bin/example',
		});
	});

	it('preserves the committed catalog and exposes a retryable issue after write failure', async () => {
		const stored = createBuiltInTerminalLaunchProfileCatalog();
		const controller = createController({
			load: async () => ({ catalog: stored, kind: 'available' }),
			replace: async () => Promise.reject(new Error('disk unavailable')),
		});
		await controller.initialize();

		await expect(
			controller.mutate({
				kind: 'update',
				profile: {
					command: { kind: 'system-shell', login: true },
					id: stored.defaultProfileId,
					name: null,
				},
			}),
		).resolves.toEqual({
			code: 'write-failed',
			kind: 'failed',
			snapshot: {
				capabilities: { loginShell: true },
				catalog: stored,
				persistenceIssue: 'write-failed',
				source: 'stored',
			},
		});
		expect(await controller.resolveDefaultLaunch()).toEqual(baseLaunch);
	});

	it('allocates profile identity in Main and rejects malformed or colliding identities', async () => {
		const store = storeWith({ kind: 'missing' });
		const malformed = createController(store, 'darwin', () => 'bad identity');
		await malformed.initialize();
		await expect(
			malformed.mutate({
				kind: 'create',
				name: 'Copy',
				sourceProfileId: DesktopTerminalLaunchProfileIdSchema.parse('system-default'),
			}),
		).resolves.toMatchObject({ code: 'identity-allocation-failed', kind: 'failed' });

		const colliding = createController(store, 'darwin', () => 'system-default');
		await colliding.initialize();
		await expect(
			colliding.mutate({
				kind: 'create',
				name: 'Copy',
				sourceProfileId: DesktopTerminalLaunchProfileIdSchema.parse('system-default'),
			}),
		).resolves.toMatchObject({ code: 'identity-allocation-failed', kind: 'failed' });
	});
});

function createController(
	store: TerminalLaunchProfileStore,
	platform: NodeJS.Platform = 'darwin',
	allocateProfileId: () => string = () => 'profile-created',
	automaticIntegration: AutomaticShellIntegrationConfiguration | null = null,
): DesktopTerminalLaunchProfileController {
	return new DesktopTerminalLaunchProfileController({
		allocateProfileId,
		automaticShellIntegration: automaticIntegration,
		baseLaunch,
		platform,
		store,
	});
}

function storeWith(loadResult: TerminalLaunchProfileStoreLoadResult): TerminalLaunchProfileStore {
	let catalog: DesktopTerminalLaunchProfileCatalog | undefined;
	return {
		load: async () => (catalog === undefined ? loadResult : { catalog, kind: 'available' }),
		replace: async (candidate) => {
			catalog = candidate;
		},
	};
}
