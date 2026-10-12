import {
	DesktopTerminalLaunchProfileCatalogSchema,
	DesktopTerminalLaunchProfileIdSchema,
} from '@orchbee/desktop-client-protocol';
import { describe, expect, it } from 'vitest';

import {
	createBuiltInTerminalLaunchProfileCatalog,
	transitionTerminalLaunchProfileCatalog,
	validateTerminalLaunchProfileCatalog,
} from '../terminal-launch-profile-catalog';

describe('terminal launch-profile catalog', () => {
	it.each([
		{ login: false, platform: 'darwin' },
		{ login: false, platform: 'linux' },
		{ login: false, platform: 'win32' },
	] satisfies readonly { readonly login: boolean; readonly platform: NodeJS.Platform }[])(
		'creates the $platform built-in System shell profile',
		(expected) => {
			expect(createBuiltInTerminalLaunchProfileCatalog()).toEqual({
				defaultProfileId: 'system-default',
				profiles: [
					{
						command: { kind: 'system-shell', login: expected.login },
						id: 'system-default',
						name: null,
					},
				],
				version: 1,
			});
		},
	);

	it.each([
		{ login: false, platform: 'darwin' },
		{ login: false, platform: 'linux' },
		{ login: false, platform: 'win32' },
	] satisfies readonly { readonly login: boolean; readonly platform: NodeJS.Platform }[])(
		'resets to the $platform built-in login default',
		(expected) => {
			const transition = transitionTerminalLaunchProfileCatalog(
				createBuiltInTerminalLaunchProfileCatalog(),
				{ kind: 'reset' },
				expected.platform,
			);

			expect(transition).toMatchObject({
				catalog: {
					profiles: [{ command: { kind: 'system-shell', login: expected.login } }],
				},
				kind: 'applied',
			});
		},
	);

	it('creates a stable command copy immediately after its source and allows duplicate names', () => {
		const initial = createBuiltInTerminalLaunchProfileCatalog();
		const created = transitionTerminalLaunchProfileCatalog(
			initial,
			{
				id: DesktopTerminalLaunchProfileIdSchema.parse('profile-1'),
				kind: 'create',
				name: 'Default',
				sourceProfileId: initial.defaultProfileId,
			},
			'darwin',
		);

		expect(created).toMatchObject({
			catalog: {
				profiles: [
					{ id: 'system-default', name: null },
					{ command: { kind: 'system-shell', login: false }, id: 'profile-1', name: 'Default' },
				],
			},
			kind: 'applied',
		});
	});

	it('updates, selects, and deletes by ID without reordering unrelated profiles', () => {
		const catalog = DesktopTerminalLaunchProfileCatalogSchema.parse({
			defaultProfileId: 'system-default',
			profiles: [
				{ command: { kind: 'system-shell', login: false }, id: 'system-default', name: null },
				{ command: { kind: 'system-shell', login: false }, id: 'profile-1', name: 'One' },
				{ command: { kind: 'system-shell', login: false }, id: 'profile-2', name: 'Two' },
			],
			version: 1,
		});
		const updated = transitionTerminalLaunchProfileCatalog(
			catalog,
			{
				kind: 'update',
				profile: {
					command: {
						arguments: ['--flag', 'two words'],
						executable: '/usr/bin/example',
						kind: 'custom-executable',
					},
					id: DesktopTerminalLaunchProfileIdSchema.parse('profile-1'),
					name: 'Updated',
				},
			},
			'linux',
		);
		if (updated.kind !== 'applied') throw new Error('Expected profile update.');
		const selected = transitionTerminalLaunchProfileCatalog(
			updated.catalog,
			{
				kind: 'select-default',
				profileId: DesktopTerminalLaunchProfileIdSchema.parse('profile-1'),
			},
			'linux',
		);
		if (selected.kind !== 'applied') throw new Error('Expected default selection.');
		const deleted = transitionTerminalLaunchProfileCatalog(
			selected.catalog,
			{
				kind: 'delete',
				profileId: DesktopTerminalLaunchProfileIdSchema.parse('profile-2'),
			},
			'linux',
		);

		expect(deleted).toMatchObject({
			catalog: {
				defaultProfileId: 'profile-1',
				profiles: [{ id: 'system-default' }, { id: 'profile-1', name: 'Updated' }],
			},
			kind: 'applied',
		});
	});

	it('rejects missing identities and protected deletion without changing the catalog', () => {
		const catalog = createBuiltInTerminalLaunchProfileCatalog();
		const missingId = DesktopTerminalLaunchProfileIdSchema.parse('missing');

		expect(
			transitionTerminalLaunchProfileCatalog(
				catalog,
				{ kind: 'select-default', profileId: missingId },
				'darwin',
			),
		).toEqual({ kind: 'rejected', reason: 'profile-not-found' });
		expect(
			transitionTerminalLaunchProfileCatalog(
				catalog,
				{ kind: 'delete', profileId: catalog.defaultProfileId },
				'darwin',
			),
		).toEqual({ kind: 'rejected', reason: 'last-profile-delete' });
	});

	it('rejects Windows login shells and relative custom executables at the platform boundary', () => {
		const loginCatalog = {
			...createBuiltInTerminalLaunchProfileCatalog(),
			profiles: [
				{
					command: { kind: 'system-shell' as const, login: true },
					id: DesktopTerminalLaunchProfileIdSchema.parse('system-default'),
					name: null,
				},
			],
		};
		const relativeCatalog = {
			...createBuiltInTerminalLaunchProfileCatalog(),
			profiles: [
				{
					command: { arguments: [], executable: 'bin/shell', kind: 'custom-executable' as const },
					id: DesktopTerminalLaunchProfileIdSchema.parse('system-default'),
					name: null,
				},
			],
		};

		expect(validateTerminalLaunchProfileCatalog(loginCatalog, 'win32')).toEqual({
			kind: 'invalid',
			reason: 'unsupported-platform',
		});
		expect(validateTerminalLaunchProfileCatalog(relativeCatalog, 'linux')).toEqual({
			kind: 'invalid',
			reason: 'invalid-profile',
		});
	});
});
