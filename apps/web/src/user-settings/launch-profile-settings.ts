import {
	BUILT_IN_USER_LAUNCH_PROFILE_ID,
	type SettingChange,
	type SettingEntries,
	type SettingsPlatform,
	type UserLaunchProfile,
	type UserLaunchProfileCatalog,
} from '@orchbee/workbench-client-protocol';

type LaunchProfileKeys = {
	readonly defaultProfile: `terminal.default_profile.${SettingsPlatform}`;
	readonly profiles: `terminal.profiles.${SettingsPlatform}`;
};

/** The two keys that hold one platform's launch catalog. */
export function launchProfileKeys(platform: SettingsPlatform): LaunchProfileKeys {
	return {
		defaultProfile: `terminal.default_profile.${platform}`,
		profiles: `terminal.profiles.${platform}`,
	};
}

/**
 * The one edit that writes `catalog` as `platform`'s profiles and default, in the file's form. A
 * leading built-in profile left as built is not written: the reader puts it back when it is absent.
 */
export function launchProfileChanges(
	platform: SettingsPlatform,
	catalog: UserLaunchProfileCatalog,
): readonly SettingChange[] {
	const keys = launchProfileKeys(platform);
	const [first, ...rest] = catalog.profiles;
	const written = first !== undefined && isUntouchedBuiltIn(first) ? rest : catalog.profiles;
	return [
		{ key: keys.profiles, value: written.map(profileEntry) },
		{ key: keys.defaultProfile, value: catalog.defaultProfileId },
	];
}

/** The edit that gives `platform`'s profiles and default back to the reader. */
export function launchProfileRestore(platform: SettingsPlatform): readonly SettingChange[] {
	const keys = launchProfileKeys(platform);
	return [
		{ key: keys.profiles, restore: true },
		{ key: keys.defaultProfile, restore: true },
	];
}

function isUntouchedBuiltIn(profile: UserLaunchProfile): boolean {
	return (
		profile.id === BUILT_IN_USER_LAUNCH_PROFILE_ID &&
		profile.name === null &&
		profile.command.kind === 'system-shell' &&
		!profile.command.login
	);
}

function profileEntry(profile: UserLaunchProfile): SettingEntries[number] {
	const name = profile.name === null ? {} : { name: profile.name };
	return profile.command.kind === 'system-shell'
		? { id: profile.id, ...name, kind: 'system-shell', login: profile.command.login }
		: {
				id: profile.id,
				...name,
				kind: 'custom-executable',
				executable: profile.command.executable,
				arguments: profile.command.arguments,
			};
}
