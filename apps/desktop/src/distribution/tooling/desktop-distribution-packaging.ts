import { z } from 'zod';

import { resolveDesktopApplicationNameFromEnvironment } from '../desktop-application-name.ts';
import {
	resolveDesktopDistributionProfile,
	type DesktopDistributionDescriptor,
	type DesktopDistributionProfile,
} from '../desktop-distribution-profile.ts';

const DESKTOP_LOCAL_APPLICATION_NAME_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_LOCAL_APPLICATION_NAME';
const DESKTOP_DISTRIBUTION_PROFILE_IDS = ['dev', 'local', 'release'] as const;
const desktopDistributionProfileIdSchema = z.enum(DESKTOP_DISTRIBUTION_PROFILE_IDS);

type DesktopDistributionProfileId = DesktopDistributionProfile['profileId'];

function parseDesktopDistributionProfileId(value: unknown): DesktopDistributionProfileId {
	return desktopDistributionProfileIdSchema.parse(value);
}

export function parseDesktopDistributionProfileArguments(
	arguments_: readonly string[],
): DesktopDistributionProfileId {
	if (arguments_.length !== 2 || arguments_[0] !== '--profile') {
		throw new Error(
			'Desktop packaging requires exactly one --profile dev|local|release selection.',
		);
	}
	return parseDesktopDistributionProfileId(arguments_[1]);
}

export function resolveDesktopLocalApplicationName(
	environment: Readonly<Record<string, string | undefined>>,
): string {
	return resolveDesktopApplicationNameFromEnvironment(
		environment,
		DESKTOP_LOCAL_APPLICATION_NAME_ENVIRONMENT_KEY,
		resolveDesktopDistributionProfile('local').applicationName,
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
	if (profile.profileId === 'local') {
		return {
			applicationName: profile.applicationName,
			profileId: 'local',
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
