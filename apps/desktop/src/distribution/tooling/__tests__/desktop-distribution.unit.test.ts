import { execFileSync } from 'node:child_process';

import { describe, expect, it } from 'vitest';

import {
	createDesktopDevelopmentScope,
	createDesktopTestArtifactStem,
	parseDesktopDistributionDescriptor,
	resolveDesktopCohortArtifactStem,
	resolveDesktopDevelopmentApplicationName,
	resolveDesktopDistributionDescriptor,
	resolveDesktopDistributionProfile,
} from '../../desktop-distribution-profile';
import {
	encodeDesktopDistributionDescriptor,
	parseDesktopDistributionProfileArguments,
	resolveDesktopTestApplicationName,
} from '../desktop-distribution-packaging';

const DESKTOP_DEVELOPMENT_APPLICATION_NAME_ENVIRONMENT_KEY =
	'ORCHBEE_DESKTOP_DEVELOPMENT_APPLICATION_NAME';
const DESKTOP_TEST_APPLICATION_NAME_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_TEST_APPLICATION_NAME';
const testArtifactStem = createDesktopTestArtifactStem('/Users/person');

describe('Desktop distribution profiles', () => {
	it('keeps packaging helpers importable by Node source-executed scripts', () => {
		const tooling = new URL('../index.ts', import.meta.url);
		const output = execFileSync(
			process.execPath,
			[
				'--input-type=module',
				'--eval',
				`const packaging = await import(process.argv[1]);
process.stdout.write(packaging.resolveDesktopTestApplicationName({}));`,
				tooling.href,
			],
			{ encoding: 'utf8', timeout: 10_000, windowsHide: true },
		);
		expect(output).toBe(resolveDesktopTestApplicationName({}));
	});

	it('resolves the development and release application identities and release policies', () => {
		const developmentScope = createDesktopDevelopmentScope(
			'/source/orchbee-worktrees/feature',
			'darwin',
		);
		const profiles = [
			resolveDesktopDistributionProfile('dev', developmentScope),
			resolveDesktopDistributionProfile('release'),
		];

		expect(
			profiles.map((profile) => ({
				applicationName: profile.applicationName,
				artifactStem: profile.artifactStem,
				bundleIdentifier: profile.bundleIdentifier,
				dataScope: profile.dataScope.kind,
				iconFileName: profile.iconFileName,
				outputDirectoryName: profile.outputDirectoryName,
			})),
		).toEqual([
			{
				applicationName: 'Orchbee Dev',
				artifactStem: 'ob-dev-4a884c37300b9d45',
				bundleIdentifier: 'com.electron.orchbee.dev.d4a884c37300b9d45',
				dataScope: 'development',
				iconFileName: 'orchbee-dev.icns',
				outputDirectoryName: 'dev',
			},
			{
				applicationName: 'Orchbee',
				artifactStem: 'orchbee',
				bundleIdentifier: 'com.electron.orchbee',
				dataScope: 'release',
				iconFileName: 'orchbee.icns',
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

	it('names Windows cohorts by environment stem and other platforms by the shipped names', () => {
		const developmentProfile = resolveDesktopDistributionProfile(
			'dev',
			createDesktopDevelopmentScope('/source/orchbee', 'win32'),
		);
		const testProfile = resolveDesktopDistributionProfile('test', undefined, {
			artifactStem: testArtifactStem,
		});
		const releaseProfile = resolveDesktopDistributionProfile('release');

		expect(resolveDesktopCohortArtifactStem(developmentProfile, 'win32')).toBe(
			developmentProfile.artifactStem,
		);
		expect(resolveDesktopCohortArtifactStem(testProfile, 'win32')).toBe(testArtifactStem);
		expect(resolveDesktopCohortArtifactStem(releaseProfile, 'win32')).toBe('orchbee');
		expect(resolveDesktopCohortArtifactStem(developmentProfile, 'darwin')).toBe('orchbee');
		expect(resolveDesktopCohortArtifactStem(testProfile, 'linux')).toBe('orchbee');
	});

	it('derives a test stem per home directory in the ob-test grammar', () => {
		expect(testArtifactStem).toMatch(/^ob-test-[0-9a-f]{16}$/u);
		expect(createDesktopTestArtifactStem('/Users/person')).toBe(testArtifactStem);
		expect(createDesktopTestArtifactStem('/Users/other')).not.toBe(testArtifactStem);
		expect(() => createDesktopTestArtifactStem('')).toThrow('requires the home directory');
	});

	it('overrides development and test application names from their own environment keys', () => {
		const developmentScope = createDesktopDevelopmentScope('/source/orchbee', 'darwin');
		const developmentApplicationName = 'Orchbee Dev Canary';
		const developmentProfile = resolveDesktopDistributionProfile('dev', developmentScope, {
			applicationName: developmentApplicationName,
		});
		const testApplicationName = 'Terminal';
		const testProfile = resolveDesktopDistributionProfile('test', undefined, {
			applicationName: testApplicationName,
			artifactStem: testArtifactStem,
		});
		const testDescriptor = parseDesktopDistributionDescriptor(
			JSON.parse(encodeDesktopDistributionDescriptor(testProfile)),
		);

		expect(
			resolveDesktopDevelopmentApplicationName({
				[DESKTOP_DEVELOPMENT_APPLICATION_NAME_ENVIRONMENT_KEY]: developmentApplicationName,
			}),
		).toBe(developmentApplicationName);
		expect(
			resolveDesktopTestApplicationName({
				[DESKTOP_TEST_APPLICATION_NAME_ENVIRONMENT_KEY]: testApplicationName,
			}),
		).toBe(testApplicationName);
		expect(resolveDesktopDistributionDescriptor(testDescriptor, 'darwin')).toEqual(testProfile);
		expect(developmentProfile.bundleIdentifier).toBe(
			resolveDesktopDistributionProfile('dev', developmentScope).bundleIdentifier,
		);
		expect(testProfile.bundleIdentifier).toBe(
			resolveDesktopDistributionProfile('test', undefined, {
				artifactStem: testArtifactStem,
			}).bundleIdentifier,
		);
	});

	it('uses profile defaults and rejects unsafe test overrides', () => {
		expect(resolveDesktopDevelopmentApplicationName({})).toBe('Orchbee Dev');
		expect(
			resolveDesktopDevelopmentApplicationName({
				[DESKTOP_TEST_APPLICATION_NAME_ENVIRONMENT_KEY]: 'Wrong Profile',
			}),
		).toBe('Orchbee Dev');
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
				resolveDesktopTestApplicationName({
					[DESKTOP_TEST_APPLICATION_NAME_ENVIRONMENT_KEY]: applicationName,
				}),
			).toThrow(DESKTOP_TEST_APPLICATION_NAME_ENVIRONMENT_KEY);
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
		expect(() => resolveDesktopDistributionProfile('test', scope)).toThrow(
			'cannot carry a development scope',
		);
		expect(() => resolveDesktopDistributionProfile('test')).toThrow(
			'requires its test artifact stem',
		);
	});

	it('requires one explicit packaging profile argument', () => {
		expect(
			['dev', 'test', 'release'].map((profileId) =>
				parseDesktopDistributionProfileArguments(['--profile', profileId]),
			),
		).toEqual(['dev', 'test', 'release']);
		expect(() => parseDesktopDistributionProfileArguments(['--profile', 'production'])).toThrow(
			'Invalid option',
		);
		expect(() => parseDesktopDistributionProfileArguments([])).toThrow('exactly one --profile');
		expect(() => parseDesktopDistributionProfileArguments(['--profile', 'test', 'extra'])).toThrow(
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

	it('round-trips a named test descriptor and rejects one without its test stem', () => {
		const profile = resolveDesktopDistributionProfile('test', undefined, {
			applicationName: 'Terminal',
			artifactStem: testArtifactStem,
		});
		const descriptor = parseDesktopDistributionDescriptor(
			JSON.parse(encodeDesktopDistributionDescriptor(profile)),
		);

		expect(resolveDesktopDistributionDescriptor(descriptor, 'darwin')).toEqual(profile);
		expect(() =>
			parseDesktopDistributionDescriptor({ profileId: 'test', schemaVersion: 1 }),
		).toThrow('Invalid input');
	});

	it('rejects missing and unknown descriptor fields', () => {
		expect(() =>
			parseDesktopDistributionDescriptor({ profileId: 'test', schemaVersion: 1, unknown: true }),
		).toThrow('Unrecognized key');
		expect(() =>
			parseDesktopDistributionDescriptor({ profileId: 'dev', schemaVersion: 1 }),
		).toThrow('Invalid input');
	});
});
