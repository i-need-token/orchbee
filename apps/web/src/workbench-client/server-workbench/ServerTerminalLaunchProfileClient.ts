import {
	BUILT_IN_USER_LAUNCH_PROFILE_ID,
	USER_LAUNCH_PROFILE_LIMITS,
	UserLaunchProfileCatalogSchema,
} from '@orchbee/workbench-client-protocol';
import type {
	SettingsRefusal,
	SettingsSnapshot,
	UserLaunchProfile,
	UserLaunchProfileCatalog,
	UserLaunchProfileId,
} from '@orchbee/workbench-client-protocol';

import type {
	TerminalLaunchProfileClientOperationResult,
	TerminalLaunchProfileClientSnapshot,
	TerminalLaunchProfileControl,
	TerminalLaunchProfileFailureCode,
	TerminalLaunchProfileSnapshot,
} from '../../terminal-launch-profile-control';
import {
	isSettingSet,
	isSettingsFileReadOnly,
	launchProfileChanges,
	launchProfileKeys,
	launchProfileRestore,
	type UserSettingsControl,
	type UserSettingsResult,
} from '../../user-settings';

type CatalogChange =
	| { readonly kind: 'create'; readonly name: string }
	| { readonly kind: 'delete'; readonly profileId: UserLaunchProfileId }
	| { readonly kind: 'reset' }
	| { readonly kind: 'select-default'; readonly profileId: UserLaunchProfileId }
	| { readonly kind: 'update'; readonly profile: UserLaunchProfile };

type CatalogRefusal = Extract<
	TerminalLaunchProfileFailureCode,
	| 'catalog-full'
	| 'default-profile-delete'
	| 'invalid-profile'
	| 'last-profile-delete'
	| 'profile-not-found'
>;

/**
 * The launch profiles are `terminal.profiles.<platform>` and `terminal.default_profile.<platform>`
 * in the settings file of the Server's machine, so each change is one edit of both keys. A profile
 * draft begun before another change to the catalog is refused as `source-changed` rather than
 * overwriting it. The Server decides what its machine can launch, and the Worker launches the
 * default profile for every terminal created after the edit.
 */
export class ServerTerminalLaunchProfileClient implements TerminalLaunchProfileControl {
	readonly #capabilities: TerminalLaunchProfileSnapshot['capabilities'];
	readonly #listeners = new Set<() => void>();
	readonly #settings: UserSettingsControl;
	readonly #stopObserving: () => void;
	#catalogRevision: string | null = null;
	#disposed = false;
	#failureCode: TerminalLaunchProfileFailureCode | null = null;
	#observed: SettingsSnapshot | null = null;
	#pending = false;
	#refusal: SettingsRefusal | null = null;
	#snapshot: TerminalLaunchProfileClientSnapshot;

	constructor(settings: UserSettingsControl, loginShellAvailable: boolean) {
		this.#capabilities = { loginShell: loginShellAvailable };
		this.#settings = settings;
		this.#observe(settings.getSnapshot().snapshot);
		this.#snapshot = this.#project();
		this.#stopObserving = settings.subscribe(() => {
			const observed = settings.getSnapshot().snapshot;
			if (observed === this.#observed) return;
			this.#observe(observed);
			this.#present();
		});
	}

	getSnapshot = (): TerminalLaunchProfileClientSnapshot => this.#snapshot;

	subscribe = (listener: () => void): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	createProfile(name: string): Promise<TerminalLaunchProfileClientOperationResult> {
		return this.#change({ kind: 'create', name });
	}

	updateProfile(
		profile: UserLaunchProfile,
		catalogRevision?: string,
	): Promise<TerminalLaunchProfileClientOperationResult> {
		return this.#change({ kind: 'update', profile }, catalogRevision);
	}

	selectDefault(
		profileId: UserLaunchProfileId,
	): Promise<TerminalLaunchProfileClientOperationResult> {
		return this.#change({ kind: 'select-default', profileId });
	}

	deleteProfile(
		profileId: UserLaunchProfileId,
	): Promise<TerminalLaunchProfileClientOperationResult> {
		return this.#change({ kind: 'delete', profileId });
	}

	reset(): Promise<TerminalLaunchProfileClientOperationResult> {
		return this.#change({ kind: 'reset' });
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#stopObserving();
		this.#listeners.clear();
	}

	/** The revision of the first snapshot that showed the current catalog. */
	#observe(observed: SettingsSnapshot | null): void {
		const previous = this.#observed;
		this.#observed = observed;
		if (observed === null) return;
		if (
			previous === null ||
			this.#catalogRevision === null ||
			JSON.stringify(previous.launchProfiles) !== JSON.stringify(observed.launchProfiles)
		) {
			this.#catalogRevision = observed.revision;
		}
	}

	async #change(
		change: CatalogChange,
		draftRevision?: string,
	): Promise<TerminalLaunchProfileClientOperationResult> {
		const observed = this.#observed;
		if (this.#disposed || observed === null) return { kind: 'unavailable' };
		if (this.#pending) {
			return { code: 'busy', kind: 'failed', snapshot: profileSnapshot(this.#snapshot) };
		}
		// A draft begun on an older catalog would overwrite the change it never saw.
		if (draftRevision !== undefined && draftRevision !== this.#catalogRevision) {
			return this.#fail('source-changed');
		}
		let changes = launchProfileRestore(observed.platform);
		if (change.kind !== 'reset') {
			const transition = transitionCatalog(observed.launchProfiles, change);
			if (typeof transition === 'string') return this.#fail(transition);
			changes = launchProfileChanges(observed.platform, transition);
		}
		this.#pending = true;
		this.#failureCode = null;
		this.#refusal = null;
		this.#present();
		const result = await this.#settings.edit(changes);
		this.#pending = false;
		if (result.kind !== 'edited' && result.kind !== 'current') {
			if (result.kind === 'refused') this.#refusal = result.refusal;
			return this.#fail(failureCode(result));
		}
		if (result.snapshot !== this.#observed) this.#observe(result.snapshot);
		this.#present();
		return { kind: 'completed', snapshot: profileSnapshot(this.#snapshot) };
	}

	#fail(code: TerminalLaunchProfileFailureCode): TerminalLaunchProfileClientOperationResult {
		this.#failureCode = code;
		if (code !== 'refused') this.#refusal = null;
		this.#present();
		return { code, kind: 'failed', snapshot: profileSnapshot(this.#snapshot) };
	}

	#project(): TerminalLaunchProfileClientSnapshot {
		const observed = this.#observed;
		if (observed === null) {
			return {
				capabilities: this.#capabilities,
				catalog: builtInCatalog(),
				failureCode: this.#failureCode ?? 'control-unavailable',
				pending: this.#pending,
				readOnly: false,
				refusal: this.#refusal,
				source: 'built-in',
			};
		}
		return {
			capabilities: this.#capabilities,
			catalog: observed.launchProfiles,
			...(this.#catalogRevision === null ? {} : { catalogRevision: this.#catalogRevision }),
			failureCode: this.#failureCode,
			pending: this.#pending,
			readOnly: isSettingsFileReadOnly(observed),
			refusal: this.#refusal,
			source: launchProfilesStored(observed) ? 'stored' : 'built-in',
		};
	}

	#present(): void {
		if (this.#disposed) return;
		this.#snapshot = this.#project();
		for (const listener of new Set(this.#listeners)) {
			if (!this.#listeners.has(listener)) continue;
			try {
				listener();
			} catch {
				// A presentation observer cannot strand a change or its answer.
			}
		}
	}
}

function transitionCatalog(
	catalog: UserLaunchProfileCatalog,
	change: Exclude<CatalogChange, { readonly kind: 'reset' }>,
): UserLaunchProfileCatalog | CatalogRefusal {
	const { profiles } = catalog;
	if (change.kind === 'create') {
		if (profiles.length >= USER_LAUNCH_PROFILE_LIMITS.maximumProfileCount) {
			return 'catalog-full';
		}
		// As in the TUI, a new profile starts as the system shell at the end of the list, never
		// as a copy of the profile last edited.
		const created = {
			command: { kind: 'system-shell', login: false },
			id: nextProfileId(catalog),
			name: change.name,
		};
		const parsed = UserLaunchProfileCatalogSchema.safeParse({
			...catalog,
			profiles: [...profiles, created],
		});
		return parsed.success ? parsed.data : 'invalid-profile';
	}
	const profileId = change.kind === 'update' ? change.profile.id : change.profileId;
	const index = profiles.findIndex((profile) => profile.id === profileId);
	if (index === -1) return 'profile-not-found';
	let candidate: unknown;
	switch (change.kind) {
		case 'update':
			candidate = {
				...catalog,
				profiles: profiles.map((profile, position) =>
					position === index ? change.profile : profile,
				),
			};
			break;
		case 'select-default':
			candidate = { ...catalog, defaultProfileId: change.profileId };
			break;
		case 'delete':
			if (profiles.length === 1) return 'last-profile-delete';
			if (catalog.defaultProfileId === change.profileId) return 'default-profile-delete';
			candidate = {
				...catalog,
				profiles: profiles.filter((_profile, position) => position !== index),
			};
			break;
	}
	const parsed = UserLaunchProfileCatalogSchema.safeParse(candidate);
	return parsed.success ? parsed.data : 'invalid-profile';
}

/** The TUI names a new profile the same way, so either Client takes the first free number. */
function nextProfileId(catalog: UserLaunchProfileCatalog): string {
	const taken = new Set<string>(catalog.profiles.map((profile) => profile.id));
	let number = 1;
	while (taken.has(`profile-${number}`)) number += 1;
	return `profile-${number}`;
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

function failureCode(
	result: Exclude<UserSettingsResult, { readonly kind: 'current' | 'edited' }>,
): TerminalLaunchProfileFailureCode {
	if (result.kind === 'outcome-unknown') return 'outcome-unknown';
	if (result.kind === 'unavailable') return 'control-unavailable';
	switch (result.refusal.reason) {
		case 'busy':
			return 'busy';
		case 'changed':
			return 'source-changed';
		case 'denied':
		case 'session-mismatch':
		case 'unavailable':
			return 'control-unavailable';
		// The Server refuses a profile its machine cannot launch, such as a relative executable.
		case 'invalid-value':
		case 'unknown-key':
			return 'invalid-profile';
		default:
			return 'refused';
	}
}

function profileSnapshot(
	snapshot: TerminalLaunchProfileClientSnapshot,
): TerminalLaunchProfileSnapshot {
	return {
		capabilities: snapshot.capabilities,
		catalog: snapshot.catalog,
		...(snapshot.catalogRevision === undefined
			? {}
			: { catalogRevision: snapshot.catalogRevision }),
		source: snapshot.source,
	};
}

/** Whether the settings file sets this platform's catalog or default, so Restore has work to do. */
function launchProfilesStored(snapshot: SettingsSnapshot): boolean {
	const keys = launchProfileKeys(snapshot.platform);
	return isSettingSet(snapshot, keys.defaultProfile) || isSettingSet(snapshot, keys.profiles);
}
