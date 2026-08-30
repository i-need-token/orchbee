import { describe, expect, it } from 'vitest';

import {
	createDesktopDevelopmentScope,
	encodeDesktopDistributionDescriptor,
	parseDesktopDistributionDescriptor,
	parseDesktopDistributionProfileArguments,
	resolveDesktopDistributionDescriptor,
	resolveDesktopDistributionProfile,
} from '../desktop-distribution-profile';

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
				releasePayload: profile.releasePayload,
			})),
		).toEqual([
			{
				applicationName: 'Orchbee Dev',
				bundleIdentifier: 'com.electron.orchbee.dev.d4a884c37300b9d45',
				dataScope: 'development',
				icon: { accentColor: '#625bf6', variant: 'dev' },
				outputDirectoryName: 'dev',
				releasePayload: 'development',
			},
			{
				applicationName: 'Orchbee Local',
				bundleIdentifier: 'com.electron.orchbee.local',
				dataScope: 'local',
				icon: { accentColor: '#168f5b', variant: 'local' },
				outputDirectoryName: 'local',
				releasePayload: 'excluded',
			},
			{
				applicationName: 'Orchbee',
				bundleIdentifier: 'com.electron.orchbee',
				dataScope: 'release',
				icon: null,
				outputDirectoryName: 'release',
				releasePayload: 'required',
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

	it('rejects missing and unknown descriptor fields', () => {
		expect(() =>
			parseDesktopDistributionDescriptor({ profileId: 'local', schemaVersion: 1, unknown: true }),
		).toThrow('Unrecognized key');
		expect(() =>
			parseDesktopDistributionDescriptor({ profileId: 'dev', schemaVersion: 1 }),
		).toThrow('Invalid input');
	});
});
