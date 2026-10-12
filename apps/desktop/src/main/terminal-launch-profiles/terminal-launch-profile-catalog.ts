import path from 'node:path';

import {
	DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID,
	DESKTOP_TERMINAL_LAUNCH_PROFILE_LIMITS,
	DesktopTerminalLaunchProfileCatalogSchema,
	type DesktopTerminalLaunchProfile,
	type DesktopTerminalLaunchProfileCatalog,
	type DesktopTerminalLaunchProfileFailureCode,
	type DesktopTerminalLaunchProfileId,
} from '@orchbee/desktop-client-protocol';

export type TerminalLaunchProfileCatalogMutation =
	| {
			readonly id: DesktopTerminalLaunchProfileId;
			readonly kind: 'create';
			readonly name: string;
			readonly sourceProfileId: DesktopTerminalLaunchProfileId;
	  }
	| {
			readonly kind: 'update';
			readonly profile: DesktopTerminalLaunchProfile;
	  }
	| {
			readonly kind: 'select-default';
			readonly profileId: DesktopTerminalLaunchProfileId;
	  }
	| {
			readonly kind: 'delete';
			readonly profileId: DesktopTerminalLaunchProfileId;
	  }
	| { readonly kind: 'reset' };

export type TerminalLaunchProfileCatalogTransition =
	| { readonly catalog: DesktopTerminalLaunchProfileCatalog; readonly kind: 'applied' }
	| {
			readonly kind: 'rejected';
			readonly reason: Exclude<
				DesktopTerminalLaunchProfileFailureCode,
				'busy' | 'control-unavailable' | 'write-failed'
			>;
	  };

export type TerminalLaunchProfileCatalogValidation =
	| { readonly catalog: DesktopTerminalLaunchProfileCatalog; readonly kind: 'valid' }
	| { readonly kind: 'invalid'; readonly reason: 'invalid-profile' | 'unsupported-platform' };

export function createBuiltInTerminalLaunchProfileCatalog(): DesktopTerminalLaunchProfileCatalog {
	return DesktopTerminalLaunchProfileCatalogSchema.parse({
		defaultProfileId: DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID,
		profiles: [
			{
				command: { kind: 'system-shell', login: false },
				id: DESKTOP_BUILT_IN_TERMINAL_LAUNCH_PROFILE_ID,
				name: null,
			},
		],
		version: 1,
	});
}

export function validateTerminalLaunchProfileCatalog(
	input: unknown,
	platform: NodeJS.Platform,
): TerminalLaunchProfileCatalogValidation {
	const parsed = DesktopTerminalLaunchProfileCatalogSchema.safeParse(input);
	if (!parsed.success) return { kind: 'invalid', reason: 'invalid-profile' };
	for (const profile of parsed.data.profiles) {
		if (profile.command.kind === 'system-shell') {
			if (platform === 'win32' && profile.command.login) {
				return { kind: 'invalid', reason: 'unsupported-platform' };
			}
			continue;
		}
		if (!platformPath(platform).isAbsolute(profile.command.executable)) {
			return { kind: 'invalid', reason: 'invalid-profile' };
		}
	}
	return { catalog: parsed.data, kind: 'valid' };
}

export function transitionTerminalLaunchProfileCatalog(
	catalog: DesktopTerminalLaunchProfileCatalog,
	mutation: TerminalLaunchProfileCatalogMutation,
	platform: NodeJS.Platform,
): TerminalLaunchProfileCatalogTransition {
	const current = validateTerminalLaunchProfileCatalog(catalog, platform);
	if (current.kind === 'invalid') {
		throw new Error('Terminal launch-profile transition requires a valid current catalog.');
	}
	if (mutation.kind === 'reset') {
		return { catalog: createBuiltInTerminalLaunchProfileCatalog(), kind: 'applied' };
	}
	if (mutation.kind === 'create') return createProfile(current.catalog, mutation, platform);
	if (mutation.kind === 'update') return updateProfile(current.catalog, mutation.profile, platform);
	if (mutation.kind === 'select-default') {
		if (!findProfile(current.catalog, mutation.profileId)) return rejected('profile-not-found');
		return canonicalTransition(
			{ ...current.catalog, defaultProfileId: mutation.profileId },
			platform,
		);
	}
	return deleteProfile(current.catalog, mutation.profileId, platform);
}

function createProfile(
	catalog: DesktopTerminalLaunchProfileCatalog,
	mutation: Extract<TerminalLaunchProfileCatalogMutation, { readonly kind: 'create' }>,
	platform: NodeJS.Platform,
): TerminalLaunchProfileCatalogTransition {
	if (catalog.profiles.length >= DESKTOP_TERMINAL_LAUNCH_PROFILE_LIMITS.maximumProfileCount) {
		return rejected('catalog-full');
	}
	const sourceIndex = catalog.profiles.findIndex(
		(profile) => profile.id === mutation.sourceProfileId,
	);
	const source = catalog.profiles[sourceIndex];
	if (source === undefined) return rejected('profile-not-found');
	if (findProfile(catalog, mutation.id)) return rejected('identity-allocation-failed');
	const profile: DesktopTerminalLaunchProfile = {
		command: cloneCommand(source.command),
		id: mutation.id,
		name: mutation.name,
	};
	const profiles = [
		...catalog.profiles.slice(0, sourceIndex + 1),
		profile,
		...catalog.profiles.slice(sourceIndex + 1),
	];
	return canonicalTransition({ ...catalog, profiles }, platform);
}

function updateProfile(
	catalog: DesktopTerminalLaunchProfileCatalog,
	profile: DesktopTerminalLaunchProfile,
	platform: NodeJS.Platform,
): TerminalLaunchProfileCatalogTransition {
	const index = catalog.profiles.findIndex((candidate) => candidate.id === profile.id);
	if (index < 0) return rejected('profile-not-found');
	const profiles = catalog.profiles.map((candidate, candidateIndex) =>
		candidateIndex === index ? profile : candidate,
	);
	return canonicalTransition({ ...catalog, profiles }, platform);
}

function deleteProfile(
	catalog: DesktopTerminalLaunchProfileCatalog,
	profileId: DesktopTerminalLaunchProfileId,
	platform: NodeJS.Platform,
): TerminalLaunchProfileCatalogTransition {
	if (!findProfile(catalog, profileId)) return rejected('profile-not-found');
	if (catalog.profiles.length === 1) return rejected('last-profile-delete');
	if (catalog.defaultProfileId === profileId) return rejected('default-profile-delete');
	return canonicalTransition(
		{
			...catalog,
			profiles: catalog.profiles.filter((profile) => profile.id !== profileId),
		},
		platform,
	);
}

function canonicalTransition(
	candidate: unknown,
	platform: NodeJS.Platform,
): TerminalLaunchProfileCatalogTransition {
	const validation = validateTerminalLaunchProfileCatalog(candidate, platform);
	return validation.kind === 'valid'
		? { catalog: validation.catalog, kind: 'applied' }
		: rejected(validation.reason);
}

function rejected(
	reason: Exclude<
		DesktopTerminalLaunchProfileFailureCode,
		'busy' | 'control-unavailable' | 'write-failed'
	>,
): TerminalLaunchProfileCatalogTransition {
	return { kind: 'rejected', reason };
}

function findProfile(
	catalog: DesktopTerminalLaunchProfileCatalog,
	profileId: DesktopTerminalLaunchProfileId,
): DesktopTerminalLaunchProfile | undefined {
	return catalog.profiles.find((profile) => profile.id === profileId);
}

function cloneCommand(
	command: DesktopTerminalLaunchProfile['command'],
): DesktopTerminalLaunchProfile['command'] {
	return command.kind === 'system-shell'
		? { kind: 'system-shell', login: command.login }
		: {
				arguments: [...command.arguments],
				executable: command.executable,
				kind: 'custom-executable',
			};
}

function platformPath(platform: NodeJS.Platform): typeof path.posix | typeof path.win32 {
	return platform === 'win32' ? path.win32 : path.posix;
}
