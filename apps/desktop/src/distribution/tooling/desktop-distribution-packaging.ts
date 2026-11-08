import { z } from 'zod';

import { resolveDesktopApplicationNameFromEnvironment } from '../desktop-application-name.ts';
import {
	DEFAULT_DESKTOP_TEST_APPLICATION_NAME,
	type DesktopDistributionDescriptor,
	type DesktopDistributionProfile,
} from '../desktop-distribution-profile.ts';

const DESKTOP_TEST_APPLICATION_NAME_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_TEST_APPLICATION_NAME';
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
