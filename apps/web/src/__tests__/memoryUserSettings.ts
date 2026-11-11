import {
	BUILT_IN_USER_LAUNCH_PROFILE_ID,
	SETTING_KEYS,
	SettingsSnapshotSchema,
	UserLaunchProfileCatalogSchema,
	settingDefault,
	type SettingChange,
	type SettingEntries,
	type SettingKey,
	type SettingsFile,
	type SettingsPlatform,
	type SettingsRefusal,
	type SettingsSnapshot,
	type SettingValues,
	type UserLaunchProfileCatalog,
} from '@orchbee/workbench-client-protocol';

import {
	launchProfileKeys,
	settingChange,
	type UserSettingsControl,
	type UserSettingsResult,
	type UserSettingsState,
} from '../user-settings';

export interface MemoryUserSettingsOptions {
	readonly file?: SettingsFile;
	readonly filePath?: string | null;
	readonly launchProfiles?: UserLaunchProfileCatalog;
	readonly platform?: SettingsPlatform;
	/** Values the file sets, in the file's spelling. */
	readonly values?: Partial<SettingValues>;
}

export interface MemoryUserSettings extends UserSettingsControl {
	/** Publishes the file in a new state, as the Server does when the file changes under it. */
	setFile(file: SettingsFile): void;
	/** Refuses the next edits with `refusal` until called with null. */
	refuseEdits(refusal: SettingsRefusal | null): void;
}

/**
 * The settings file held in memory. As through the Server, an edit reports itself pending at once
 * and its snapshot arrives a microtask later, before the edit settles. While the file does not
 * parse or cannot be read, every edit is refused as the Server refuses it.
 */
export function memoryUserSettings(options: MemoryUserSettingsOptions = {}): MemoryUserSettings {
	const listeners = new Set<() => void>();
	const platform = options.platform ?? 'windows';
	let refusal: SettingsRefusal | null = null;
	const set = new Set<string>();
	const values: Record<string, SettingsSnapshot['values'][string]> = {};
	for (const key of SETTING_KEYS) values[key] = settingDefault(key);
	for (const [key, value] of Object.entries(options.values ?? {})) {
		if (value === undefined) continue;
		values[key] = value;
		set.add(key);
	}
	let snapshot = SettingsSnapshotSchema.parse({
		diagnostics: { items: [], omittedCount: 0 },
		file: options.file ?? { status: set.size === 0 ? 'missing' : 'ok' },
		filePath:
			options.filePath === undefined
				? 'C:\\Users\\person\\.orchbee\\config.toml'
				: options.filePath,
		launchProfiles: options.launchProfiles ?? builtInCatalog(),
		platform,
		revision: '1',
		set: [...set].sort(),
		values,
	});
	let state: UserSettingsState = { available: true, failure: null, pending: false, snapshot };

	const publish = (next: UserSettingsState): void => {
		state = next;
		for (const listener of new Set(listeners)) listener();
	};
	const nextSnapshot = (changes: Partial<SettingsSnapshot>): SettingsSnapshot =>
		SettingsSnapshotSchema.parse({
			...snapshot,
			...changes,
			revision: (BigInt(snapshot.revision) + 1n).toString(),
		});
	const edit = async (changes: readonly SettingChange[]): Promise<UserSettingsResult> => {
		const keys = changes.map((change) => change.key);
		publish({ ...state, failure: null, pending: true });
		await Promise.resolve();
		const refused = refusalFor(snapshot.file) ?? refusal;
		if (refused !== null) {
			publish({ ...state, failure: { keys, kind: 'refused', refusal: refused }, pending: false });
			return { kind: 'refused', refusal: refused };
		}
		const nextValues = { ...snapshot.values };
		const nextSet = new Set(snapshot.set);
		let launchProfiles = snapshot.launchProfiles;
		const profileKeys = launchProfileKeys(snapshot.platform);
		for (const change of changes) {
			if (change.key === profileKeys.profiles) {
				launchProfiles = withProfiles(
					launchProfiles,
					'value' in change && Array.isArray(change.value) ? change.value : null,
				);
			} else if (change.key === profileKeys.defaultProfile) {
				launchProfiles = UserLaunchProfileCatalogSchema.parse({
					...launchProfiles,
					defaultProfileId:
						'value' in change && typeof change.value === 'string'
							? change.value
							: BUILT_IN_USER_LAUNCH_PROFILE_ID,
				});
			}
			if ('restore' in change) {
				nextSet.delete(change.key);
				if (isKey(change.key)) nextValues[change.key] = settingDefault(change.key);
			} else {
				nextSet.add(change.key);
				if (!isEntries(change.value)) nextValues[change.key] = change.value;
			}
		}
		snapshot = nextSnapshot({
			file: { status: 'ok' },
			launchProfiles,
			set: [...nextSet].sort(),
			values: nextValues,
		});
		publish({ ...state, failure: null, pending: false, snapshot });
		return { changed: true, kind: 'edited', snapshot };
	};

	return {
		edit,
		getSnapshot: () => state,
		refresh: () => Promise.resolve({ kind: 'current', snapshot }),
		refuseEdits: (next) => {
			refusal = next;
		},
		restore: (keys) => edit(keys.map((key) => ({ key, restore: true }))),
		set: (key, value) => edit([settingChange(key, value)]),
		setFile: (file) => {
			snapshot = nextSnapshot({ file });
			publish({ ...state, snapshot });
		},
		subscribe: (listener) => {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
	};
}

function isEntries(value: unknown): value is SettingEntries {
	return Array.isArray(value);
}

function isKey(key: string): key is SettingKey {
	return SETTING_KEYS.some((candidate) => candidate === key);
}

function refusalFor(file: SettingsFile): SettingsRefusal | null {
	if (file.status === 'syntax-error') {
		return {
			column: file.column,
			detail: file.reason,
			line: file.line,
			outcome: 'refused',
			reason: 'syntax-error',
		};
	}
	if (file.status === 'unreadable') {
		return { detail: file.reason, outcome: 'refused', reason: 'unusable-file' };
	}
	return null;
}

function builtInCatalog(): UserLaunchProfileCatalog {
	return UserLaunchProfileCatalogSchema.parse({
		defaultProfileId: BUILT_IN_USER_LAUNCH_PROFILE_ID,
		profiles: [
			{
				command: { kind: 'system-shell', login: false },
				id: BUILT_IN_USER_LAUNCH_PROFILE_ID,
				name: null,
			},
		],
		version: 1,
	});
}

/** The catalog the reader builds from the file's entries; null restores the built-in one. */
function withProfiles(
	catalog: UserLaunchProfileCatalog,
	entries: SettingEntries | null,
): UserLaunchProfileCatalog {
	if (entries === null) return builtInCatalog();
	const profiles = entries.map((entry) => ({
		command:
			entry['kind'] === 'custom-executable'
				? {
						arguments: entry['arguments'],
						executable: entry['executable'],
						kind: 'custom-executable',
					}
				: { kind: 'system-shell', login: entry['login'] },
		id: entry['id'],
		name: entry['name'] ?? null,
	}));
	const [builtIn] = builtInCatalog().profiles;
	const leading =
		builtIn !== undefined && !profiles.some((profile) => profile.id === builtIn.id)
			? [builtIn]
			: [];
	return UserLaunchProfileCatalogSchema.parse({ ...catalog, profiles: [...leading, ...profiles] });
}
