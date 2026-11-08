import { createHash } from 'node:crypto';

import { z } from 'zod';

import {
	DEFAULT_DESKTOP_TEST_APPLICATION_NAME,
	resolveDesktopApplicationNameFromEnvironment,
} from '../desktop-application-name.ts';
import type {
	DesktopDistributionDescriptor,
	DesktopDistributionProfile,
} from '../desktop-distribution-profile.ts';

const DESKTOP_TEST_APPLICATION_NAME_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_TEST_APPLICATION_NAME';
const TEST_ARTIFACT_STEM_DIGEST_LENGTH = 16;
const TEST_ARTIFACT_STEM_DIGEST_PURPOSE = 'orchbee-test-artifact-v1';
const DESKTOP_DISTRIBUTION_PROFILE_IDS = ['dev', 'test', 'release'] as const;
const desktopDistributionProfileIdSchema = z.enum(DESKTOP_DISTRIBUTION_PROFILE_IDS);

type DesktopDistributionProfileId = DesktopDistributionProfile['profileId'];

function parseDesktopDistributionProfileId(value: unknown): DesktopDistributionProfileId {
	return desktopDistributionProfileIdSchema.parse(value);
}

export function parseDesktopDistributionProfileArguments(
	arguments_: readonly string[],
): DesktopDistributionProfileId {
	if (arguments_.length !== 2 || arguments_[0] !== '--profile') {
		throw new Error('Desktop packaging requires exactly one --profile dev|test|release selection.');
	}
	return parseDesktopDistributionProfileId(arguments_[1]);
}

export function resolveDesktopTestApplicationName(
	environment: Readonly<Record<string, string | undefined>>,
): string {
	return resolveDesktopApplicationNameFromEnvironment(
		environment,
		DESKTOP_TEST_APPLICATION_NAME_ENVIRONMENT_KEY,
		DEFAULT_DESKTOP_TEST_APPLICATION_NAME,
	);
}

/** The stem of this home directory's test cohort, so two accounts on one machine never share names. */
export function createDesktopTestArtifactStem(homeDirectory: string): string {
	if (homeDirectory.length === 0) {
		throw new Error('The test artifact stem requires the home directory.');
	}
	const digest = createHash('sha256')
		.update(`${TEST_ARTIFACT_STEM_DIGEST_PURPOSE}\0${homeDirectory}`)
		.digest('hex')
		.slice(0, TEST_ARTIFACT_STEM_DIGEST_LENGTH);
	return `ob-test-${digest}`;
}

function createDesktopDistributionDescriptor(
	profile: DesktopDistributionProfile,
): DesktopDistributionDescriptor {
	if (profile.profileId === 'dev') {
		return {
			applicationName: profile.applicationName,
			developmentScope: {
				digest: profile.dataScope.digest,
				digestVersion: profile.dataScope.digestVersion,
				sourceRoot: profile.dataScope.sourceRoot,
			},
			profileId: 'dev',
			schemaVersion: 1,
		};
	}
	if (profile.profileId === 'test') {
		return {
			applicationName: profile.applicationName,
			artifactStem: profile.artifactStem,
			profileId: 'test',
			schemaVersion: 1,
		};
	}
	return {
		profileId: profile.profileId,
		schemaVersion: 1,
	};
}

export function encodeDesktopDistributionDescriptor(profile: DesktopDistributionProfile): string {
	return `${JSON.stringify(createDesktopDistributionDescriptor(profile), null, 2)}\n`;
}
