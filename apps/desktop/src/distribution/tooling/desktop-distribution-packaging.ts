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

const PACKAGING_USAGE =
	'Usage: package.mjs --profile dev|test|release [--installer] [--build-id <id>] ' +
	'[--key-file <absolute PEM> | --key-env <VARIABLE>]...';

interface DesktopPackagingArguments {
	readonly profileId: DesktopDistributionProfileId;
	/** Also compile the Windows installer from the packaged application. */
	readonly installer: boolean;
	/** The release manifest's `buildId`; the release workflow passes `<commit>.<run number>`. */
	readonly buildId: string | undefined;
	/** `--key-file` and `--key-env` pairs, in order, passed on to the manifest signing script. */
	readonly signingKeys: readonly string[];
}

export function parseDesktopPackagingArguments(
	arguments_: readonly string[],
): DesktopPackagingArguments {
	let profileId: DesktopDistributionProfileId | undefined;
	let installer = false;
	let buildId: string | undefined;
	const signingKeys: string[] = [];
	for (let index = 0; index < arguments_.length; index += 1) {
		const option = arguments_[index];
		if (option === '--installer' && !installer) {
			installer = true;
			continue;
		}
		const value = arguments_[index + 1];
		if (value === undefined || value.startsWith('--')) throw new Error(PACKAGING_USAGE);
		index += 1;
		if (option === '--profile' && profileId === undefined) {
			profileId = parseDesktopDistributionProfileId(value);
		} else if (option === '--build-id' && buildId === undefined) {
			buildId = value;
		} else if (option === '--key-file' || option === '--key-env') {
			signingKeys.push(option, value);
		} else {
			throw new Error(PACKAGING_USAGE);
		}
	}
	if (profileId === undefined) throw new Error(PACKAGING_USAGE);
	return { buildId, installer, profileId, signingKeys };
}

/**
 * The names the Windows installer gives a profile, from the release distribution freeze sheet: a
 * release is `Orchbee` in `%LOCALAPPDATA%\Programs\Orchbee`, registered as
 * `com.orchbee.desktop_is1`; a test build replaces `orchbee` and `Orchbee` by its stem in every one,
 * as `orchbee uninstall` of that build expects. A checkout's build has no installer.
 */
interface DesktopWindowsInstallerIdentity {
	/** Inno Setup's `AppId`; the uninstall registration is this name plus `_is1`. */
	readonly appId: string;
	/** The Start menu shortcut and the name Settings › Apps shows. */
	readonly displayName: string;
	/** The store root's folder under `%LOCALAPPDATA%\Programs`. */
	readonly storeFolderName: string;
	/** The Client's file name without `.exe`, in the bundled runtime and as the store launcher. */
	readonly clientName: string;
	/** The setup file's name without `.exe`. */
	readonly setupBaseName: string;
}

export function resolveDesktopWindowsInstallerIdentity(
	profile: DesktopDistributionProfile,
	version: string,
): DesktopWindowsInstallerIdentity {
	if (profile.profileId === 'dev') {
		throw new Error('A development checkout has no Windows installer; package test or release.');
	}
	const [name, displayName] =
		profile.profileId === 'release'
			? ['orchbee', 'Orchbee']
			: [profile.artifactStem, profile.artifactStem];
	return {
		appId: `com.${name}.desktop`,
		clientName: name,
		displayName,
		setupBaseName: `${displayName}-${version}-windows-x86_64-setup`,
		storeFolderName: displayName,
	};
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
