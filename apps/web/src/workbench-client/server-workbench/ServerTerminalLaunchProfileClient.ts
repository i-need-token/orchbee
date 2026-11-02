import {
	BUILT_IN_USER_LAUNCH_PROFILE_ID,
	USER_LAUNCH_PROFILE_LIMITS,
	UserLaunchProfileCatalogSchema,
} from '@orchbee/workbench-client-protocol';
import type {
	UserLaunchProfile,
	UserLaunchProfileCatalog,
	UserLaunchProfileId,
	UserPreferencesResponse,
	UserPreferencesSnapshot,
} from '@orchbee/workbench-client-protocol';

import type {
	TerminalLaunchProfileClientOperationResult,
	TerminalLaunchProfileClientSnapshot,
	TerminalLaunchProfileControl,
	TerminalLaunchProfileFailureCode,
	TerminalLaunchProfileSnapshot,
} from '../../terminal-launch-profile-control';
import type { UserPreferencesControl } from '../../user-preferences';

type CatalogChange =
	| {
			readonly kind: 'create';
			readonly name: string;
			readonly sourceProfileId: UserLaunchProfileId;
	  }
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
 * The launch profiles are one of the Session's shared preferences, so each change saves the whole
 * catalog under the source this Client last observed: another Client's change in between is refused
 * as `source-changed` rather than overwritten. The Server decides what this machine can launch, and
 * the Worker launches the default profile for every terminal created after the save.
 */
export class ServerTerminalLaunchProfileClient implements TerminalLaunchProfileControl {
	readonly #capabilities: TerminalLaunchProfileSnapshot['capabilities'];
	readonly #listeners = new Set<() => void>();
	readonly #preferences: UserPreferencesControl;
	readonly #stopObserving: () => void;
	#disposed = false;
	#failureCode: TerminalLaunchProfileFailureCode | null = null;
	#observed: UserPreferencesSnapshot | null;
	#pending = false;
	#snapshot: TerminalLaunchProfileClientSnapshot;

	constructor(preferences: UserPreferencesControl, loginShellAvailable: boolean) {
		this.#capabilities = { loginShell: loginShellAvailable };
		this.#preferences = preferences;
		this.#observed = preferences.getSnapshot().snapshot;
		this.#snapshot = this.#project();
		this.#stopObserving = preferences.subscribe(() => {
			const observed = preferences.getSnapshot().snapshot;
			// A source names its record's content, so an unchanged one changes nothing shown here.
			if (observed?.source === this.#observed?.source) return;
			this.#observed = observed;
			this.#present();
		});
	}

	getSnapshot = (): TerminalLaunchProfileClientSnapshot => this.#snapshot;

	subscribe = (listener: () => void): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	createProfile(
		name: string,
		sourceProfileId: UserLaunchProfileId,
	): Promise<TerminalLaunchProfileClientOperationResult> {
		return this.#change({ kind: 'create', name, sourceProfileId });
	}

	updateProfile(
		profile: UserLaunchProfile,
		preferencesSource?: string,
	): Promise<TerminalLaunchProfileClientOperationResult> {
		return this.#change({ kind: 'update', profile }, preferencesSource);
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

	async #change(
		change: CatalogChange,
		draftSource?: string,
	): Promise<TerminalLaunchProfileClientOperationResult> {
		const observed = this.#observed;
		if (this.#disposed || observed === null) return { kind: 'unavailable' };
		if (this.#pending) {
			return { code: 'busy', kind: 'failed', snapshot: profileSnapshot(this.#snapshot) };
		}
		// A draft begun on an older catalog would overwrite the change it never saw.
		if (draftSource !== undefined && draftSource !== observed.source) {
			return this.#fail('source-changed');
		}
		let catalog: UserLaunchProfileCatalog | null = null;
		if (change.kind !== 'reset') {
			const transition = transitionCatalog(observed.values.launchProfiles, change);
			if (typeof transition === 'string') return this.#fail(transition);
			catalog = transition;
		}
		this.#pending = true;
		this.#failureCode = null;
		this.#present();
		const response = await this.#preferences.update({ launchProfiles: catalog }, observed.source);
		this.#pending = false;
		if (response.kind !== 'snapshot') return this.#fail(failureCode(response));
		this.#observed = response.snapshot;
		this.#present();
		return { kind: 'completed', snapshot: profileSnapshot(this.#snapshot) };
	}

	#fail(code: TerminalLaunchProfileFailureCode): TerminalLaunchProfileClientOperationResult {
		this.#failureCode = code;
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
				persistenceIssue: null,
				source: 'built-in',
			};
		}
		return {
			capabilities: this.#capabilities,
			catalog: observed.values.launchProfiles,
			failureCode: this.#failureCode,
			pending: this.#pending,
			persistenceIssue: null,
			preferencesSource: observed.source,
			source: 'stored',
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
	let profileId: string;
	if (change.kind === 'create') profileId = change.sourceProfileId;
	else if (change.kind === 'update') profileId = change.profile.id;
	else profileId = change.profileId;
	const index = profiles.findIndex((profile) => profile.id === profileId);
	const target = profiles[index];
	if (target === undefined) return 'profile-not-found';
	let candidate: unknown;
	switch (change.kind) {
		case 'create': {
			if (profiles.length >= USER_LAUNCH_PROFILE_LIMITS.maximumProfileCount) {
				return 'catalog-full';
			}
			const created = {
				command: structuredClone(target.command),
				id: nextProfileId(catalog),
				name: change.name,
			};
			candidate = {
				...catalog,
				profiles: [...profiles.slice(0, index + 1), created, ...profiles.slice(index + 1)],
			};
			break;
		}
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
	response: Exclude<UserPreferencesResponse, { readonly kind: 'snapshot' }>,
): TerminalLaunchProfileFailureCode {
	if (response.kind === 'outcome-unknown') return 'outcome-unknown';
	switch (response.reason) {
		case 'busy':
		case 'source-changed':
		case 'write-failed':
			return response.reason;
		case 'unavailable':
			return 'control-unavailable';
		// The Server refuses a profile this machine cannot launch, such as a relative executable.
		case 'invalid-changes':
		case 'invalid-request':
		case 'invalid-source':
			return 'invalid-profile';
	}
}

function profileSnapshot(
	snapshot: TerminalLaunchProfileClientSnapshot,
): TerminalLaunchProfileSnapshot {
	return {
		capabilities: snapshot.capabilities,
		catalog: snapshot.catalog,
		persistenceIssue: snapshot.persistenceIssue,
		...(snapshot.preferencesSource === undefined
			? {}
			: { preferencesSource: snapshot.preferencesSource }),
		source: snapshot.source,
	};
}
