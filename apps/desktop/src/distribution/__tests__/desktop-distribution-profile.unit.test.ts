import { describe, expect, it } from 'vitest';

import {
	createDesktopDevelopmentScope,
	encodeDesktopDistributionDescriptor,
	parseDesktopDistributionDescriptor,
	parseDesktopDistributionProfileArguments,
	resolveDesktopDevelopmentApplicationName,
	resolveDesktopDistributionDescriptor,
	resolveDesktopDistributionProfile,
	resolveDesktopLocalApplicationName,
} from '../desktop-distribution-profile';

const DESKTOP_DEVELOPMENT_APPLICATION_NAME_ENVIRONMENT_KEY =
	'ORCHBEE_DESKTOP_DEVELOPMENT_APPLICATION_NAME';
const DESKTOP_LOCAL_APPLICATION_NAME_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_LOCAL_APPLICATION_NAME';

describe('Desktop distribution profiles', () => {
	it('resolves the three explicit application identities and release policies', () => {
		const developmentScope = createDesktopDevelopmentScope(
			'/source/orchbee-worktrees/feature',
			'darwin',
		);
		const profiles = [
			resolveDesktopDistributionProfile('dev', developmentScope),
			resolveDesktopDistributionProfile('local'),
			resolveDesktopDistributionProfile('release'),
		];

		expect(
			profiles.map((profile) => ({
				applicationName: profile.applicationName,
				bundleIdentifier: profile.bundleIdentifier,
				dataScope: profile.dataScope.kind,
				icon: profile.icon,
				outputDirectoryName: profile.outputDirectoryName,
			})),
		).toEqual([
			{
				applicationName: 'Orchbee Dev',
				bundleIdentifier: 'com.electron.orchbee.dev.d4a884c37300b9d45',
				dataScope: 'development',
				icon: null,
				outputDirectoryName: 'dev',
			},
			{
				applicationName: 'Orchbee Local',
				bundleIdentifier: 'com.electron.orchbee.local',
				dataScope: 'local',
				icon: null,
				outputDirectoryName: 'local',
			},
			{
				applicationName: 'Orchbee',
				bundleIdentifier: 'com.electron.orchbee',
				dataScope: 'release',
				icon: null,
				outputDirectoryName: 'release',
			},
		]);
	});

	it('keeps the established development data-root digest stable', () => {
		expect(createDesktopDevelopmentScope('/source/orchbee-worktrees/feature', 'darwin')).toEqual({
			digest: '4a884c37300b9d45',
			digestVersion: 1,
			sourceRoot: '/source/orchbee-worktrees/feature',
		});
	});

	it('overrides development and local application names from their own environment keys', () => {
		const developmentScope = createDesktopDevelopmentScope('/source/orchbee', 'darwin');
		const developmentApplicationName = 'Orchbee Dev Canary';
		const developmentProfile = resolveDesktopDistributionProfile('dev', developmentScope, {
			applicationName: developmentApplicationName,
		});
		const localApplicationName = 'Terminal';
		const localProfile = resolveDesktopDistributionProfile('local', undefined, {
			applicationName: localApplicationName,
		});
		const localDescriptor = parseDesktopDistributionDescriptor(
			JSON.parse(encodeDesktopDistributionDescriptor(localProfile)),
		);

		expect(
			resolveDesktopDevelopmentApplicationName({
				[DESKTOP_DEVELOPMENT_APPLICATION_NAME_ENVIRONMENT_KEY]: developmentApplicationName,
			}),
		).toBe(developmentApplicationName);
		expect(
			resolveDesktopLocalApplicationName({
				[DESKTOP_LOCAL_APPLICATION_NAME_ENVIRONMENT_KEY]: localApplicationName,
			}),
		).toBe(localApplicationName);
		expect(resolveDesktopDistributionDescriptor(localDescriptor, 'darwin')).toEqual(localProfile);
		expect(developmentProfile.bundleIdentifier).toBe(
			resolveDesktopDistributionProfile('dev', developmentScope).bundleIdentifier,
		);
		expect(localProfile.bundleIdentifier).toBe(
			resolveDesktopDistributionProfile('local').bundleIdentifier,
		);
	});

	it('uses profile defaults and rejects unsafe local overrides', () => {
		expect(resolveDesktopDevelopmentApplicationName({})).toBe('Orchbee Dev');
		expect(resolveDesktopLocalApplicationName({})).toBe('Orchbee Local');
		expect(
			resolveDesktopDevelopmentApplicationName({
				[DESKTOP_LOCAL_APPLICATION_NAME_ENVIRONMENT_KEY]: 'Wrong Profile',
			}),
		).toBe('Orchbee Dev');
		expect(
			resolveDesktopLocalApplicationName({
				[DESKTOP_DEVELOPMENT_APPLICATION_NAME_ENVIRONMENT_KEY]: 'Wrong Profile',
			}),
		).toBe('Orchbee Local');
		const invalidNames = [
			'',
			' Orchbee Dev',
			'Orchbee Dev ',
			'Orchbee/Dev',
			'Orchbee"Dev',
			'Orchbee Dev.',
			'.',
			'CON',
			'con.txt',
			`Orchbee${String.fromCodePoint(0x85)}Dev`,
		];

		for (const applicationName of invalidNames) {
			expect(() =>
				resolveDesktopDevelopmentApplicationName({
					[DESKTOP_DEVELOPMENT_APPLICATION_NAME_ENVIRONMENT_KEY]: applicationName,
				}),
			).toThrow(DESKTOP_DEVELOPMENT_APPLICATION_NAME_ENVIRONMENT_KEY);
			expect(() =>
				resolveDesktopLocalApplicationName({
					[DESKTOP_LOCAL_APPLICATION_NAME_ENVIRONMENT_KEY]: applicationName,
				}),
			).toThrow(DESKTOP_LOCAL_APPLICATION_NAME_ENVIRONMENT_KEY);
		}
	});

	it('normalizes an accepted development name to the packaged filename representation', () => {
		const applicationName = 'Orchbée Dev';

		expect(
			resolveDesktopDevelopmentApplicationName({
				[DESKTOP_DEVELOPMENT_APPLICATION_NAME_ENVIRONMENT_KEY]: applicationName,
			}),
		).toBe(applicationName.normalize('NFD'));
	});

	it('normalizes Windows checkout identity before hashing', () => {
		const first = createDesktopDevelopmentScope('C:\\Source\\Orchbee', 'win32');
		const second = createDesktopDevelopmentScope('c:\\Source\\Orchbee\\.', 'win32');

		expect(first).toEqual(second);
		expect(first.sourceRoot).toBe('c:\\Source\\Orchbee');
	});

	it('requires development scope only for the dev profile', () => {
		const scope = createDesktopDevelopmentScope('/source/orchbee', 'darwin');

		expect(() => resolveDesktopDistributionProfile('dev')).toThrow(
			'requires a development checkout scope',
		);
		expect(() => resolveDesktopDistributionProfile('local', scope)).toThrow(
			'cannot carry a development scope',
		);
	});

	it('requires one explicit packaging profile argument', () => {
		expect(
			['dev', 'local', 'release'].map((profileId) =>
				parseDesktopDistributionProfileArguments(['--profile', profileId]),
			),
		).toEqual(['dev', 'local', 'release']);
		expect(() => parseDesktopDistributionProfileArguments(['--profile', 'production'])).toThrow(
			'Invalid option',
		);
		expect(() => parseDesktopDistributionProfileArguments([])).toThrow('exactly one --profile');
		expect(() => parseDesktopDistributionProfileArguments(['--profile', 'local', 'extra'])).toThrow(
			'exactly one --profile',
		);
	});

	it('round-trips a dev descriptor and rejects a mismatched scope digest', () => {
		const profile = resolveDesktopDistributionProfile(
			'dev',
			createDesktopDevelopmentScope('/source/orchbee', 'darwin'),
		);
		const descriptor = parseDesktopDistributionDescriptor(
			JSON.parse(encodeDesktopDistributionDescriptor(profile)),
		);
		if (descriptor.profileId !== 'dev') throw new Error('Expected a dev descriptor.');

		expect(resolveDesktopDistributionDescriptor(descriptor, 'darwin')).toEqual(profile);
		expect(() =>
			resolveDesktopDistributionDescriptor(
				{
					...descriptor,
					developmentScope: { ...descriptor.developmentScope, digest: '0'.repeat(16) },
				},
				'darwin',
			),
		).toThrow('invalid scope digest');
	});

	it('round-trips a named local descriptor and defaults an older local descriptor', () => {
		const profile = resolveDesktopDistributionProfile('local', undefined, {
			applicationName: 'Terminal',
		});
		const descriptor = parseDesktopDistributionDescriptor(
			JSON.parse(encodeDesktopDistributionDescriptor(profile)),
		);

		expect(resolveDesktopDistributionDescriptor(descriptor, 'darwin')).toEqual(profile);
		expect(
			resolveDesktopDistributionDescriptor(
				parseDesktopDistributionDescriptor({ profileId: 'local', schemaVersion: 1 }),
				'darwin',
			),
		).toEqual(resolveDesktopDistributionProfile('local'));
	});

	it('rejects missing and unknown descriptor fields', () => {
		expect(() =>
			parseDesktopDistributionDescriptor({ profileId: 'local', schemaVersion: 1, unknown: true }),
		).toThrow('Unrecognized key');
		expect(() =>
			parseDesktopDistributionDescriptor({ profileId: 'dev', schemaVersion: 1 }),
		).toThrow('Invalid input');
	});
});
