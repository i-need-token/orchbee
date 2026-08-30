import { createHash } from 'node:crypto';
import path from 'node:path';

import { z } from 'zod';

const DEVELOPMENT_SCOPE_DIGEST_LENGTH = 16;
const DEVELOPMENT_SCOPE_DIGEST_VERSION = 1;
const DESKTOP_DISTRIBUTION_DESCRIPTOR_SCHEMA_VERSION = 1;

export const DESKTOP_DISTRIBUTION_DESCRIPTOR_FILE_NAME = 'desktop-distribution-v1.json';
const DESKTOP_DISTRIBUTION_PROFILE_IDS = ['dev', 'local', 'release'] as const;

const desktopDistributionProfileIdSchema = z.enum(DESKTOP_DISTRIBUTION_PROFILE_IDS);
const developmentScopeSchema = z.strictObject({
	digest: z.string().regex(/^[0-9a-f]{16}$/u),
	digestVersion: z.literal(DEVELOPMENT_SCOPE_DIGEST_VERSION),
	sourceRoot: z.string().min(1),
});
const desktopDistributionDescriptorSchema = z.discriminatedUnion('profileId', [
	z.strictObject({
		developmentScope: developmentScopeSchema,
		profileId: z.literal('dev'),
		schemaVersion: z.literal(DESKTOP_DISTRIBUTION_DESCRIPTOR_SCHEMA_VERSION),
	}),
	z.strictObject({
		profileId: z.literal('local'),
		schemaVersion: z.literal(DESKTOP_DISTRIBUTION_DESCRIPTOR_SCHEMA_VERSION),
	}),
	z.strictObject({
		profileId: z.literal('release'),
		schemaVersion: z.literal(DESKTOP_DISTRIBUTION_DESCRIPTOR_SCHEMA_VERSION),
	}),
]);

type DesktopDistributionProfileId = (typeof DESKTOP_DISTRIBUTION_PROFILE_IDS)[number];

interface DesktopDevelopmentScope {
	readonly digest: string;
	readonly digestVersion: typeof DEVELOPMENT_SCOPE_DIGEST_VERSION;
	readonly sourceRoot: string;
}

interface DesktopDistributionProfileDefinition {
	readonly applicationName: string;
	readonly baseBundleIdentifier: string;
	readonly dataRootDirectoryName: string | null;
	readonly icon: {
		readonly accentColor: string | null;
		readonly variant: DesktopDistributionProfileId;
	};
	readonly outputDirectoryName: DesktopDistributionProfileId;
	readonly packageName: string;
	readonly releasePayload: 'development' | 'excluded' | 'required';
}

const DESKTOP_DISTRIBUTION_PROFILE_DEFINITIONS = {
	dev: {
		applicationName: 'Orchbee Dev',
		baseBundleIdentifier: 'com.electron.orchbee.dev',
		dataRootDirectoryName: null,
		icon: { accentColor: '#625bf6', variant: 'dev' },
		outputDirectoryName: 'dev',
		packageName: 'orchbee-dev',
		releasePayload: 'development',
	},
	local: {
		applicationName: 'Orchbee Local',
		baseBundleIdentifier: 'com.electron.orchbee.local',
		dataRootDirectoryName: '.orchbee-local',
		icon: { accentColor: '#168f5b', variant: 'local' },
		outputDirectoryName: 'local',
		packageName: 'orchbee-local',
		releasePayload: 'excluded',
	},
	release: {
		applicationName: 'Orchbee',
		baseBundleIdentifier: 'com.electron.orchbee',
		dataRootDirectoryName: '.orchbee',
		icon: { accentColor: null, variant: 'release' },
		outputDirectoryName: 'release',
		packageName: 'orchbee',
		releasePayload: 'required',
	},
} as const satisfies Record<DesktopDistributionProfileId, DesktopDistributionProfileDefinition>;

interface DesktopDistributionProfileBase extends DesktopDistributionProfileDefinition {
	readonly bundleIdentifier: string;
	readonly profileId: DesktopDistributionProfileId;
}

export type DesktopDistributionProfile =
	| (DesktopDistributionProfileBase & {
			readonly dataScope: DesktopDevelopmentScope & { readonly kind: 'development' };
			readonly profileId: 'dev';
	  })
	| (DesktopDistributionProfileBase & {
			readonly dataScope: { readonly kind: 'local' };
			readonly profileId: 'local';
	  })
	| (DesktopDistributionProfileBase & {
			readonly dataScope: { readonly kind: 'release' };
			readonly profileId: 'release';
	  });

type DesktopDistributionDescriptor = z.infer<typeof desktopDistributionDescriptorSchema>;

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

export function createDesktopDevelopmentScope(
	canonicalSourceRoot: string,
	platform: NodeJS.Platform,
): DesktopDevelopmentScope {
	const platformPath = platform === 'win32' ? path.win32 : path.posix;
	const sourceRoot = normalizePath(canonicalSourceRoot, platform);
	if (!platformPath.isAbsolute(sourceRoot)) {
		throw new Error('Desktop development source root must be an absolute canonical path.');
	}
	const digest = createHash('sha256')
		.update(`orchbee-development-data-root-v${DEVELOPMENT_SCOPE_DIGEST_VERSION}\0`)
		.update(sourceRoot)
		.digest('hex')
		.slice(0, DEVELOPMENT_SCOPE_DIGEST_LENGTH);
	return { digest, digestVersion: DEVELOPMENT_SCOPE_DIGEST_VERSION, sourceRoot };
}

export function resolveDesktopDistributionProfile(
	profileId: DesktopDistributionProfileId,
	developmentScope?: DesktopDevelopmentScope,
): DesktopDistributionProfile {
	const definition = DESKTOP_DISTRIBUTION_PROFILE_DEFINITIONS[profileId];
	if (profileId === 'dev') {
		if (developmentScope === undefined) {
			throw new Error('The dev Desktop distribution requires a development checkout scope.');
		}
		return {
			...definition,
			bundleIdentifier: `${definition.baseBundleIdentifier}.d${developmentScope.digest}`,
			dataScope: { ...developmentScope, kind: 'development' },
			profileId,
		};
	}
	if (developmentScope !== undefined) {
		throw new Error(`The ${profileId} Desktop distribution cannot carry a development scope.`);
	}
	if (profileId === 'local') {
		return {
			...definition,
			bundleIdentifier: definition.baseBundleIdentifier,
			dataScope: { kind: 'local' },
			profileId,
		};
	}
	return {
		...definition,
		bundleIdentifier: definition.baseBundleIdentifier,
		dataScope: { kind: 'release' },
		profileId,
	};
}

function createDesktopDistributionDescriptor(
	profile: DesktopDistributionProfile,
): DesktopDistributionDescriptor {
	if (profile.profileId === 'dev') {
		return {
			developmentScope: {
				digest: profile.dataScope.digest,
				digestVersion: profile.dataScope.digestVersion,
				sourceRoot: profile.dataScope.sourceRoot,
			},
			profileId: 'dev',
			schemaVersion: DESKTOP_DISTRIBUTION_DESCRIPTOR_SCHEMA_VERSION,
		};
	}
	return {
		profileId: profile.profileId,
		schemaVersion: DESKTOP_DISTRIBUTION_DESCRIPTOR_SCHEMA_VERSION,
	};
}

export function encodeDesktopDistributionDescriptor(profile: DesktopDistributionProfile): string {
	return `${JSON.stringify(createDesktopDistributionDescriptor(profile), null, 2)}\n`;
}

export function parseDesktopDistributionDescriptor(value: unknown): DesktopDistributionDescriptor {
	return desktopDistributionDescriptorSchema.parse(value);
}

export function resolveDesktopDistributionDescriptor(
	descriptor: DesktopDistributionDescriptor,
	platform: NodeJS.Platform,
): DesktopDistributionProfile {
	if (descriptor.profileId !== 'dev') {
		return resolveDesktopDistributionProfile(descriptor.profileId);
	}
	const scope = createDesktopDevelopmentScope(descriptor.developmentScope.sourceRoot, platform);
	if (
		scope.digest !== descriptor.developmentScope.digest ||
		scope.digestVersion !== descriptor.developmentScope.digestVersion
	) {
		throw new Error('The Desktop development distribution descriptor has an invalid scope digest.');
	}
	return resolveDesktopDistributionProfile('dev', scope);
}

function normalizePath(value: string, platform: NodeJS.Platform): string {
	if (platform === 'win32') {
		const normalized = path.win32.normalize(value).normalize('NFC');
		return /^[A-Z]:/u.test(normalized)
			? `${normalized[0]?.toLowerCase() ?? ''}${normalized.slice(1)}`
			: normalized;
	}
	return path.posix.normalize(value).normalize('NFC');
}
