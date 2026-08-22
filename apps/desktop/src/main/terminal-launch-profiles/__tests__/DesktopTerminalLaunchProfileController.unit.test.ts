import {
	DesktopTerminalLaunchProfileIdSchema,
	type DesktopTerminalLaunchProfileCatalog,
} from '@orchbee/desktop-client-protocol';
import { TerminalLaunchSchema } from '@orchbee/terminal-protocol';
import { describe, expect, it, vi } from 'vitest';

import { createDefaultTerminalProfile } from '../../host';
import { DesktopTerminalLaunchProfileController } from '../DesktopTerminalLaunchProfileController';
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

describe('DesktopTerminalLaunchProfileController', () => {
	it.each([
		{ arguments: ['-l'], login: true, platform: 'darwin' },
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
		expect(controller.resolveDefaultLaunch()).toEqual({
			...baseLaunch,
			arguments: expected.arguments,
		});
	});

	it('keeps a stored non-login choice authoritative on Darwin', async () => {
		const stored = createBuiltInTerminalLaunchProfileCatalog('linux');
		const replace = vi.fn<TerminalLaunchProfileStore['replace']>();
		const controller = createController(
			{ load: async () => ({ catalog: stored, kind: 'available' }), replace },
			'darwin',
		);

		await expect(controller.initialize()).resolves.toMatchObject({
			catalog: { profiles: [{ command: { kind: 'system-shell', login: false } }] },
			source: 'stored',
		});
		expect(controller.resolveDefaultLaunch()).toEqual(baseLaunch);
		expect(replace).not.toHaveBeenCalled();
	});

	it('resets a Darwin catalog to the login System shell only after persistence commits', async () => {
		const stored = createBuiltInTerminalLaunchProfileCatalog('linux');
		const replace = vi.fn<TerminalLaunchProfileStore['replace']>(async () => undefined);
		const controller = createController({
			load: async () => ({ catalog: stored, kind: 'available' }),
			replace,
		});
		await controller.initialize();

		await expect(controller.mutate({ kind: 'reset' })).resolves.toMatchObject({
			kind: 'completed',
			snapshot: {
				catalog: { profiles: [{ command: { kind: 'system-shell', login: true } }] },
				source: 'stored',
			},
		});
		expect(replace).toHaveBeenCalledWith(
			expect.objectContaining({
				profiles: [expect.objectContaining({ command: { kind: 'system-shell', login: true } })],
			}),
		);
		expect(controller.resolveDefaultLaunch()).toEqual({ ...baseLaunch, arguments: ['-l'] });
	});

	it('reports invalid stored platform behavior without rewriting it', async () => {
		const replace = vi.fn<TerminalLaunchProfileStore['replace']>();
		const invalid = {
			...createBuiltInTerminalLaunchProfileCatalog('win32'),
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

	it('rejects a catalog whose resolved launch cannot cross the Host control boundary', async () => {
		const stored = createBuiltInTerminalLaunchProfileCatalog('linux');
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
		).resolves.toMatchObject({ code: 'invalid-profile', kind: 'failed' });
		expect(replace).not.toHaveBeenCalled();
		expect(controller.resolveDefaultLaunch()).toEqual(baseLaunch);
	});

	it('commits a new default only after atomic persistence succeeds', async () => {
		let releaseWrite: (() => void) | undefined;
		const replace = vi.fn<TerminalLaunchProfileStore['replace']>(
			async () =>
				new Promise<void>((resolve) => {
					releaseWrite = resolve;
				}),
		);
		const stored = createBuiltInTerminalLaunchProfileCatalog('linux');
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

		expect(controller.resolveDefaultLaunch().executable).toBe('/bin/zsh');
		releaseWrite?.();
		await expect(update).resolves.toMatchObject({ kind: 'completed' });
		expect(controller.resolveDefaultLaunch()).toMatchObject({
			arguments: ['--flag'],
			executable: '/usr/bin/example',
		});
	});

	it('preserves the committed catalog and exposes a retryable issue after write failure', async () => {
		const stored = createBuiltInTerminalLaunchProfileCatalog('linux');
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
		expect(controller.resolveDefaultLaunch()).toEqual(baseLaunch);
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
): DesktopTerminalLaunchProfileController {
	return new DesktopTerminalLaunchProfileController({
		allocateProfileId,
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
