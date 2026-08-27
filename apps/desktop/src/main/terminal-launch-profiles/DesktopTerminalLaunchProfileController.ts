import {
	DesktopTerminalLaunchProfileIdSchema,
	DesktopTerminalLaunchProfileSnapshotSchema,
	type DesktopTerminalLaunchProfile,
	type DesktopTerminalLaunchProfileFailureCode,
	type DesktopTerminalLaunchProfileId,
	type DesktopTerminalLaunchProfileSnapshot,
} from '@orchbee/desktop-client-protocol';
import {
	CreationIdSchema,
	encodeControlRequest,
	MessageKind,
	TerminalLaunchSchema,
	type TerminalLaunch,
} from '@orchbee/terminal-protocol';

import {
	createBuiltInTerminalLaunchProfileCatalog,
	transitionTerminalLaunchProfileCatalog,
	validateTerminalLaunchProfileCatalog,
	type TerminalLaunchProfileCatalogMutation,
	type TerminalLaunchProfileCatalogValidation,
} from './terminal-launch-profile-catalog';
import type { AutomaticShellIntegrationConfiguration } from './resolveAutomaticShellIntegration';
import { resolveTerminalLaunchProfile } from './resolveTerminalLaunchProfile';
import type { TerminalLaunchProfileStore } from './terminal-launch-profile-store';

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
	readonly baseLaunch: TerminalLaunch;
	readonly platform: NodeJS.Platform;
	readonly store: TerminalLaunchProfileStore;
}

export class DesktopTerminalLaunchProfileController {
	readonly #allocateProfileId: () => string;
	readonly #automaticShellIntegration: AutomaticShellIntegrationConfiguration | null;
	readonly #baseLaunch: TerminalLaunch;
	#initialization: Promise<DesktopTerminalLaunchProfileSnapshot> | undefined;
	#mutationTail = Promise.resolve();
	readonly #platform: NodeJS.Platform;
	#snapshot: DesktopTerminalLaunchProfileSnapshot | undefined;
	readonly #store: TerminalLaunchProfileStore;

	constructor(options: DesktopTerminalLaunchProfileControllerOptions) {
		this.#allocateProfileId = options.allocateProfileId;
		this.#automaticShellIntegration = options.automaticShellIntegration;
		this.#baseLaunch = TerminalLaunchSchema.parse(options.baseLaunch);
		this.#platform = options.platform;
		this.#store = options.store;
	}

	initialize(): Promise<DesktopTerminalLaunchProfileSnapshot> {
		this.#initialization ??= this.#load();
		return this.#initialization;
	}

	getSnapshot(): DesktopTerminalLaunchProfileSnapshot {
		if (this.#snapshot === undefined) {
			throw new Error('Terminal launch profiles must initialize before reading their snapshot.');
		}
		return this.#snapshot;
	}

	resolveDefaultLaunch(): TerminalLaunch {
		const snapshot = this.getSnapshot();
		const profile = snapshot.catalog.profiles.find(
			(candidate) => candidate.id === snapshot.catalog.defaultProfileId,
		);
		if (profile === undefined) {
			throw new Error('The terminal launch-profile catalog has no default profile.');
		}
		const resolution = resolveTerminalLaunchProfile(
			this.#baseLaunch,
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
	): Promise<DesktopTerminalLaunchProfileMutationResult> {
		await this.initialize();
		const operation = this.#mutationTail.then(async () => this.#applyMutation(mutation));
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
			const validation = validateLaunchableTerminalLaunchProfileCatalog(
				loaded.catalog,
				this.#automaticShellIntegration,
				this.#baseLaunch,
				this.#platform,
			);
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
	): Promise<DesktopTerminalLaunchProfileMutationResult> {
		const snapshot = this.getSnapshot();
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
		const launchValidation = validateLaunchableTerminalLaunchProfileCatalog(
			transition.catalog,
			this.#automaticShellIntegration,
			this.#baseLaunch,
			this.#platform,
		);
		if (launchValidation.kind === 'invalid') {
			return { code: launchValidation.reason, kind: 'failed', snapshot };
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

const PROFILE_VALIDATION_CREATION_ID = CreationIdSchema.parse('11111111111111111111111111111111');

function validateLaunchableTerminalLaunchProfileCatalog(
	input: unknown,
	automaticShellIntegration: AutomaticShellIntegrationConfiguration | null,
	baseLaunch: TerminalLaunch,
	platform: NodeJS.Platform,
): TerminalLaunchProfileCatalogValidation {
	const validation = validateTerminalLaunchProfileCatalog(input, platform);
	if (validation.kind === 'invalid') return validation;
	for (const profile of validation.catalog.profiles) {
		const resolution = resolveTerminalLaunchProfile(
			baseLaunch,
			profile,
			platform,
			automaticShellIntegration,
		);
		if (resolution.kind === 'rejected') {
			return { kind: 'invalid', reason: resolution.reason };
		}
		const encoded = encodeControlRequest(MessageKind.createTerminal, {
			creationId: PROFILE_VALIDATION_CREATION_ID,
			launch: resolution.launch,
		});
		if (!encoded.ok) return { kind: 'invalid', reason: 'invalid-profile' };
	}
	return validation;
}

function fallbackSnapshot(
	persistenceIssue: DesktopTerminalLaunchProfileSnapshot['persistenceIssue'],
	platform: NodeJS.Platform,
): DesktopTerminalLaunchProfileSnapshot {
	return DesktopTerminalLaunchProfileSnapshotSchema.parse({
		capabilities: capabilities(platform),
		catalog: createBuiltInTerminalLaunchProfileCatalog(platform),
		persistenceIssue,
		source: 'built-in',
	});
}

function capabilities(
	platform: NodeJS.Platform,
): DesktopTerminalLaunchProfileSnapshot['capabilities'] {
	return { loginShell: platform !== 'win32' };
}
