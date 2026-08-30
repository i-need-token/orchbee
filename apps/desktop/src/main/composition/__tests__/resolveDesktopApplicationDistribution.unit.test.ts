import path from 'node:path';

import { describe, expect, it, vi } from 'vitest';

import {
	createDesktopDevelopmentScope,
	resolveDesktopDistributionProfile,
} from '../../../distribution';
import { resolveDesktopApplicationDistribution } from '../resolveDesktopApplicationDistribution';

type ResolverDependencies = NonNullable<
	Parameters<typeof resolveDesktopApplicationDistribution>[1]
>;

describe('resolveDesktopApplicationDistribution', () => {
	it('resolves live development from the canonical checkout path', () => {
		const dependencies = createDependencies();
		dependencies.realpath.mockReturnValue('/source/orchbee');

		const profile = resolveDesktopApplicationDistribution(
			{
				applicationPath: '/source/orchbee/apps/desktop',
				environment: { ORCHBEE_DESKTOP_DEVELOPMENT_SOURCE_ROOT: '/source/link' },
				packaged: false,
				platform: 'darwin',
			},
			dependencies,
		);

		expect(profile.profileId).toBe('dev');
		expect(profile.dataScope).toMatchObject({ kind: 'development', sourceRoot: '/source/orchbee' });
		expect(dependencies.readFile).not.toHaveBeenCalled();
	});

	it('requires an explicit checkout for live development', () => {
		expect(() =>
			resolveDesktopApplicationDistribution(
				{
					applicationPath: '/source/orchbee/apps/desktop',
					environment: {},
					packaged: false,
					platform: 'darwin',
				},
				createDependencies(),
			),
		).toThrow('canonical source checkout root');
	});

	it('loads a packaged local profile only from the bundle descriptor', () => {
		const dependencies = createDependencies();
		dependencies.readFile.mockReturnValue('{"profileId":"local","schemaVersion":1}\n');

		const profile = resolveDesktopApplicationDistribution(
			{
				applicationPath: '/Applications/Orchbee Local.app/Contents/Resources/app.asar',
				environment: { ORCHBEE_DESKTOP_DEVELOPMENT_SOURCE_ROOT: '/untrusted/override' },
				packaged: true,
				platform: 'darwin',
			},
			dependencies,
		);

		expect(profile).toEqual(resolveDesktopDistributionProfile('local'));
		expect(dependencies.readFile).toHaveBeenCalledWith(
			path.join(
				'/Applications/Orchbee Local.app/Contents/Resources/app.asar',
				'desktop-distribution-v1.json',
			),
			'utf8',
		);
		expect(dependencies.realpath).not.toHaveBeenCalled();
	});

	it('revalidates a packaged dev descriptor against the current checkout path', () => {
		const dependencies = createDependencies();
		const profile = resolveDesktopDistributionProfile(
			'dev',
			createDesktopDevelopmentScope('/source/orchbee', 'darwin'),
		);
		if (profile.profileId !== 'dev') throw new Error('Expected a dev distribution profile.');
		dependencies.readFile.mockReturnValue(
			`${JSON.stringify({
				developmentScope: {
					digest: profile.dataScope.digest,
					digestVersion: profile.dataScope.digestVersion,
					sourceRoot: profile.dataScope.sourceRoot,
				},
				profileId: 'dev',
				schemaVersion: 1,
			})}\n`,
		);
		dependencies.realpath.mockReturnValue('/source/orchbee');

		expect(
			resolveDesktopApplicationDistribution(
				{
					applicationPath: '/Applications/Orchbee Dev.app/Contents/Resources/app.asar',
					environment: {},
					packaged: true,
					platform: 'darwin',
				},
				dependencies,
			),
		).toEqual(profile);

		dependencies.realpath.mockReturnValue('/source/moved');
		expect(() =>
			resolveDesktopApplicationDistribution(
				{
					applicationPath: '/Applications/Orchbee Dev.app/Contents/Resources/app.asar',
					environment: {},
					packaged: true,
					platform: 'darwin',
				},
				dependencies,
			),
		).toThrow('does not match its current source checkout');
	});

	it('rejects malformed and unsupported packaged descriptors', () => {
		const dependencies = createDependencies();
		dependencies.readFile.mockReturnValue('{"profileId":"preview","schemaVersion":1}\n');

		expect(() =>
			resolveDesktopApplicationDistribution(
				{
					applicationPath: '/Applications/Orchbee.app/Contents/Resources/app.asar',
					environment: {},
					packaged: true,
					platform: 'darwin',
				},
				dependencies,
			),
		).toThrow('Invalid discriminator value');
	});
});

function createDependencies(): {
	readonly readFile: ReturnType<typeof vi.fn<ResolverDependencies['readFile']>>;
	readonly realpath: ReturnType<typeof vi.fn<ResolverDependencies['realpath']>>;
} {
	return {
		readFile: vi.fn<ResolverDependencies['readFile']>(),
		realpath: vi.fn<ResolverDependencies['realpath']>(),
	};
}
