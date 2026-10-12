import {
	DesktopTerminalLaunchProfileIdSchema,
	DesktopTerminalLaunchProfileSnapshotSchema,
	type DesktopTerminalLaunchProfile,
	type DesktopTerminalLaunchProfileFailureCode,
	type DesktopTerminalLaunchProfileId,
	type DesktopTerminalLaunchProfileSnapshot,
} from '@orchbee/desktop-client-protocol';
import type { TerminalLaunch } from '@orchbee/terminal-protocol';

import {
	createBuiltInTerminalLaunchProfileCatalog,
	transitionTerminalLaunchProfileCatalog,
	validateTerminalLaunchProfileCatalog,
	type TerminalLaunchProfileCatalogMutation,
} from './terminal-launch-profile-catalog';
import type { AutomaticShellIntegrationConfiguration } from './resolveAutomaticShellIntegration';
import {
	resolveTerminalLaunchProfile,
	type DesktopTerminalLaunchTemplate,
} from './resolveTerminalLaunchProfile';
import type { TerminalLaunchProfileStore } from './terminal-launch-profile-store';
import type { DesktopUserPreferencesController } from '../user-preferences';

export type DesktopTerminalLaunchProfileMutation =
	| {
			readonly kind: 'create';
			readonly name: string;
			readonly sourceProfileId: DesktopTerminalLaunchProfileId;
	  }
	| { readonly kind: 'update'; readonly profile: DesktopTerminalLaunchProfile }
	| { readonly kind: 'select-default'; readonly profileId: DesktopTerminalLaunchProfileId }
	| { readonly kind: 'delete'; readonly profileId: DesktopTerminalLaunchProfileId }
	| { readonly kind: 'reset' };

export type DesktopTerminalLaunchProfileMutationResult =
	| { readonly kind: 'completed'; readonly snapshot: DesktopTerminalLaunchProfileSnapshot }
	| {
			readonly code: DesktopTerminalLaunchProfileFailureCode;
			readonly kind: 'failed';
			readonly snapshot: DesktopTerminalLaunchProfileSnapshot;
	  };

export interface DesktopTerminalLaunchProfileControllerOptions {
	readonly allocateProfileId: () => string;
	readonly automaticShellIntegration: AutomaticShellIntegrationConfiguration | null;
	readonly baseLaunch: DesktopTerminalLaunchTemplate;
	readonly resolveSystemShell?: (() => Promise<string | null>) | undefined;
	readonly platform: NodeJS.Platform;
	readonly store: TerminalLaunchProfileStore;
	readonly userPreferences?: DesktopUserPreferencesController | undefined;
}

export class DesktopTerminalLaunchProfileController {
	readonly #allocateProfileId: () => string;
	readonly #automaticShellIntegration: AutomaticShellIntegrationConfiguration | null;
	readonly #baseLaunch: DesktopTerminalLaunchTemplate;
	readonly #resolveSystemShell: (() => Promise<string | null>) | undefined;
	#initialization: Promise<DesktopTerminalLaunchProfileSnapshot> | undefined;
	#mutationTail = Promise.resolve();
	readonly #platform: NodeJS.Platform;
	#snapshot: DesktopTerminalLaunchProfileSnapshot | undefined;
	readonly #store: TerminalLaunchProfileStore;
	readonly #userPreferences: DesktopUserPreferencesController | undefined;

	constructor(options: DesktopTerminalLaunchProfileControllerOptions) {
		this.#allocateProfileId = options.allocateProfileId;
		this.#automaticShellIntegration = options.automaticShellIntegration;
		this.#baseLaunch = options.baseLaunch;
		this.#resolveSystemShell = options.resolveSystemShell;
		this.#platform = options.platform;
		this.#store = options.store;
		this.#userPreferences = options.userPreferences;
	}

	initialize(): Promise<DesktopTerminalLaunchProfileSnapshot> {
		this.#initialization ??= this.#load();
		return this.#initialization;
	}

	getSnapshot(): DesktopTerminalLaunchProfileSnapshot {
		const preferences = this.#userPreferences?.getSnapshot();
		if (preferences !== undefined)
			return {
				capabilities: capabilities(this.#platform),
				catalog: preferences.values.launchProfiles,
				persistenceIssue: null,
				source: 'stored',
				preferencesSource: preferences.source,
			};
		if (this.#snapshot === undefined) {
			throw new Error('Terminal launch profiles must initialize before reading their snapshot.');
		}
		return this.#snapshot;
	}

	async resolveDefaultLaunch(): Promise<TerminalLaunch> {
		const snapshot = this.getSnapshot();
		const profile = snapshot.catalog.profiles.find(
			(candidate) => candidate.id === snapshot.catalog.defaultProfileId,
		);
		if (profile === undefined) {
			throw new Error('The terminal launch-profile catalog has no default profile.');
		}
		// Freeze the selected profile before waiting. Later settings affect the next creation.
		const executable =
			profile.command.kind === 'system-shell' && this.#resolveSystemShell !== undefined
				? await this.#resolveSystemShell()
				: this.#baseLaunch.executable;
		if (profile.command.kind === 'system-shell' && executable === null) {
			throw new Error('The system shell is unavailable.');
		}
		const resolution = resolveTerminalLaunchProfile(
			{ ...this.#baseLaunch, executable },
			profile,
			this.#platform,
			this.#automaticShellIntegration,
		);
		if (resolution.kind === 'rejected') {
			throw new Error(`The default terminal launch profile is invalid: ${resolution.reason}.`);
		}
		return resolution.launch;
	}

	async mutate(
		mutation: DesktopTerminalLaunchProfileMutation,
		preferencesSource?: string,
	): Promise<DesktopTerminalLaunchProfileMutationResult> {
		await this.initialize();
		const operation = this.#mutationTail.then(async () =>
			this.#applyMutation(mutation, preferencesSource),
		);
		this.#mutationTail = operation.then(
			() => undefined,
			() => undefined,
		);
		return operation;
	}

	async flush(): Promise<void> {
		await this.initialize();
		await this.#mutationTail;
	}

	async #load(): Promise<DesktopTerminalLaunchProfileSnapshot> {
		const loaded = await this.#store.load();
		let snapshot: DesktopTerminalLaunchProfileSnapshot;
		if (loaded.kind === 'available') {
			const validation = validateTerminalLaunchProfileCatalog(loaded.catalog, this.#platform);
			snapshot =
				validation.kind === 'valid'
					? {
							capabilities: capabilities(this.#platform),
							catalog: validation.catalog,
							persistenceIssue: null,
							source: 'stored',
						}
					: fallbackSnapshot('invalid-record', this.#platform);
		} else if (loaded.kind === 'missing') {
			snapshot = fallbackSnapshot(null, this.#platform);
		} else {
			snapshot = fallbackSnapshot(loaded.reason, this.#platform);
		}
		this.#snapshot = DesktopTerminalLaunchProfileSnapshotSchema.parse(snapshot);
		return this.#snapshot;
	}

	async #applyMutation(
		mutation: DesktopTerminalLaunchProfileMutation,
		preferencesSource?: string,
	): Promise<DesktopTerminalLaunchProfileMutationResult> {
		const snapshot = this.getSnapshot();
		if (
			this.#userPreferences !== undefined &&
			(preferencesSource === undefined || snapshot.preferencesSource !== preferencesSource)
		) {
			return { kind: 'failed', code: 'source-changed', snapshot };
		}
		const domainMutation = this.#prepareDomainMutation(mutation);
		if (domainMutation.kind === 'failed') {
			return { code: domainMutation.code, kind: 'failed', snapshot };
		}
		const transition = transitionTerminalLaunchProfileCatalog(
			snapshot.catalog,
			domainMutation.mutation,
			this.#platform,
		);
		if (transition.kind === 'rejected') {
			return { code: transition.reason, kind: 'failed', snapshot };
		}
		if (this.#userPreferences !== undefined && preferencesSource !== undefined) {
			const { response } = await this.#userPreferences.execute({
				operation: 'update',
				source: preferencesSource,
				changes: { launchProfiles: mutation.kind === 'reset' ? null : transition.catalog },
			});
			if (response.kind === 'snapshot') return { kind: 'completed', snapshot: this.getSnapshot() };
			let code: 'outcome-unknown' | 'source-changed' | 'busy' | 'write-failed' = 'write-failed';
			if (response.kind === 'outcome-unknown') code = 'outcome-unknown';
			else if (response.reason === 'source-changed' || response.reason === 'busy')
				code = response.reason;
			return { kind: 'failed', code, snapshot: this.getSnapshot() };
		}
		try {
			await this.#store.replace(transition.catalog);
		} catch {
			this.#snapshot = DesktopTerminalLaunchProfileSnapshotSchema.parse({
				...snapshot,
				persistenceIssue: 'write-failed',
			});
			return { code: 'write-failed', kind: 'failed', snapshot: this.#snapshot };
		}
		this.#snapshot = DesktopTerminalLaunchProfileSnapshotSchema.parse({
			capabilities: snapshot.capabilities,
			catalog: transition.catalog,
			persistenceIssue: null,
			source: 'stored',
		});
		return { kind: 'completed', snapshot: this.#snapshot };
	}

	#prepareDomainMutation(
		mutation: DesktopTerminalLaunchProfileMutation,
	):
		| { readonly kind: 'ready'; readonly mutation: TerminalLaunchProfileCatalogMutation }
		| { readonly code: 'identity-allocation-failed'; readonly kind: 'failed' } {
		if (mutation.kind !== 'create') return { kind: 'ready', mutation };
		const profileId = DesktopTerminalLaunchProfileIdSchema.safeParse(this.#allocateProfileId());
		return profileId.success
			? {
					kind: 'ready',
					mutation: {
						id: profileId.data,
						kind: 'create',
						name: mutation.name,
						sourceProfileId: mutation.sourceProfileId,
					},
				}
			: { code: 'identity-allocation-failed', kind: 'failed' };
	}
}

function fallbackSnapshot(
	persistenceIssue: DesktopTerminalLaunchProfileSnapshot['persistenceIssue'],
	platform: NodeJS.Platform,
): DesktopTerminalLaunchProfileSnapshot {
	return DesktopTerminalLaunchProfileSnapshotSchema.parse({
		capabilities: capabilities(platform),
		catalog: createBuiltInTerminalLaunchProfileCatalog(),
		persistenceIssue,
		source: 'built-in',
	});
}

function capabilities(
	platform: NodeJS.Platform,
): DesktopTerminalLaunchProfileSnapshot['capabilities'] {
	return { loginShell: platform !== 'win32' };
}
