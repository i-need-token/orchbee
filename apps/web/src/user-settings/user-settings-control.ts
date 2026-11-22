import {
	readSettingValue,
	settingDefault,
	type SettingChange,
	type SettingKey,
	type SettingsRefusal,
	type SettingsSnapshot,
	type SettingValues,
} from '@orchbee/workbench-client-protocol';

/**
 * Why the last edit did not land. `keys` names every key the edit carried, so each row it touched
 * can say so; the snapshot still shows what the file holds.
 */
export type UserSettingsFailure =
	| {
			readonly kind: 'refused';
			readonly keys: readonly string[];
			readonly refusal: SettingsRefusal;
	  }
	/** The edit's answer never arrived; the next published snapshot settles what the file holds. */
	| { readonly kind: 'outcome-unknown'; readonly keys: readonly string[] }
	/** The Server cannot be reached, or stopped publishing; values shown may be stale. */
	| { readonly kind: 'unavailable'; readonly keys: readonly string[] };

export interface UserSettingsState {
	/** The newest snapshot by revision; null until the first read answers. */
	readonly snapshot: SettingsSnapshot | null;
	/** Published snapshots are arriving, so an edit can be sent. */
	readonly available: boolean;
	/** An edit or read is in flight. */
	readonly pending: boolean;
	readonly failure: UserSettingsFailure | null;
}

export type UserSettingsResult =
	| { readonly kind: 'current'; readonly snapshot: SettingsSnapshot }
	| { readonly kind: 'edited'; readonly snapshot: SettingsSnapshot; readonly changed: boolean }
	| { readonly kind: 'refused'; readonly refusal: SettingsRefusal }
	| { readonly kind: 'outcome-unknown' }
	| { readonly kind: 'unavailable' };

/**
 * The settings file on the Server's machine, key by key. Every edit is one request that the Server
 * applies whole or refuses whole; the snapshot it answers with, and every one it publishes later,
 * replaces what is shown.
 */
export interface UserSettingsControl {
	getSnapshot(): UserSettingsState;
	subscribe(listener: () => void): () => void;
	/** One key; null restores the key's default. */
	set<Key extends SettingKey>(key: Key, value: SettingValues[Key]): Promise<UserSettingsResult>;
	restore(keys: readonly SettingKey[]): Promise<UserSettingsResult>;
	/** Several keys in one write, such as a launch catalog and its default profile. */
	edit(changes: readonly SettingChange[]): Promise<UserSettingsResult>;
	refresh(): Promise<UserSettingsResult>;
}

/** The change that gives `key` the value `value`; the wire carries no null, so null restores. */
export function settingChange<Key extends SettingKey>(
	key: Key,
	value: SettingValues[Key],
): SettingChange {
	return value === null ? { key, restore: true } : { key, value };
}

/** The value `key` has in `snapshot`, or its default before the first snapshot arrives. */
export function readSetting<Key extends SettingKey>(
	snapshot: SettingsSnapshot | null,
	key: Key,
): SettingValues[Key] {
	return snapshot === null ? settingDefault(key) : readSettingValue(snapshot.values, key);
}

/** Whether the file itself sets `key`, which is what the changed-setting dot shows. */
export function isSettingSet(snapshot: SettingsSnapshot | null, key: string): boolean {
	return snapshot?.set.includes(key) ?? false;
}

/**
 * While the file does not parse or cannot be read, the snapshot carries the last values that did,
 * and no edit can land; the page shows them without letting anyone change them.
 */
export function isSettingsFileReadOnly(snapshot: SettingsSnapshot | null): boolean {
	return snapshot?.file.status === 'syntax-error' || snapshot?.file.status === 'unreadable';
}

/** The failure of the last edit if it carried `key`. */
export function settingFailure(
	state: UserSettingsState,
	key: SettingKey,
): UserSettingsFailure | null {
	return state.failure?.keys.includes(key) === true ? state.failure : null;
}
