import {
	DesktopTerminalDefaultSettingsSnapshotSchema,
	type DesktopTerminalDefaultSettingsFailureCode,
	type DesktopTerminalDefaultSettingsSnapshot,
} from '@orchbee/desktop-client-protocol';
import {
	TerminalConfigurationSchema,
	type TerminalConfiguration,
} from '@orchbee/terminal-protocol';

import type { TerminalDefaultSettingsStore } from './terminal-default-settings-store';
import type { DesktopUserPreferencesController } from '../user-preferences';

export type DesktopTerminalDefaultSettingsMutationResult =
	| { readonly kind: 'completed'; readonly snapshot: DesktopTerminalDefaultSettingsSnapshot }
	| {
			readonly code: DesktopTerminalDefaultSettingsFailureCode;
			readonly kind: 'failed';
			readonly snapshot: DesktopTerminalDefaultSettingsSnapshot;
	  };

export interface DesktopTerminalDefaultSettingsControllerOptions {
	readonly runtimeConfiguration: TerminalConfiguration;
	readonly setHostDefaults: (
		configuration: TerminalConfiguration,
	) => Promise<TerminalConfiguration>;
	readonly store: TerminalDefaultSettingsStore;
	readonly userPreferences?: DesktopUserPreferencesController | undefined;
}

export class DesktopTerminalDefaultSettingsController {
	#initialization: Promise<DesktopTerminalDefaultSettingsSnapshot> | undefined;
	#mutationTail = Promise.resolve();
	readonly #runtimeConfiguration: TerminalConfiguration;
	readonly #setHostDefaults: (
		configuration: TerminalConfiguration,
	) => Promise<TerminalConfiguration>;
	#snapshot: DesktopTerminalDefaultSettingsSnapshot | undefined;
	readonly #store: TerminalDefaultSettingsStore;
	readonly #userPreferences: DesktopUserPreferencesController | undefined;

	constructor(options: DesktopTerminalDefaultSettingsControllerOptions) {
		this.#runtimeConfiguration = TerminalConfigurationSchema.parse(options.runtimeConfiguration);
		this.#setHostDefaults = options.setHostDefaults;
		this.#store = options.store;
		this.#userPreferences = options.userPreferences;
	}

	initialize(): Promise<DesktopTerminalDefaultSettingsSnapshot> {
		this.#initialization ??= this.#loadAndCommit();
		return this.#initialization;
	}

	getSnapshot(): DesktopTerminalDefaultSettingsSnapshot {
		const preferences = this.#userPreferences?.getSnapshot();
		if (preferences !== undefined)
			return {
				configuration: { scrollbackBytes: preferences.values.scrollbackBytes },
				persistenceIssue: null,
				source: 'stored',
				preferencesSource: preferences.source,
			};
		if (this.#snapshot === undefined) {
			throw new Error('Terminal-default settings must initialize before reading their snapshot.');
		}
		return this.#snapshot;
	}

	async update(
		configuration: TerminalConfiguration,
		preferencesSource?: string,
	): Promise<DesktopTerminalDefaultSettingsMutationResult> {
		const canonical = TerminalConfigurationSchema.parse(configuration);
		await this.initialize();
		const operation = this.#mutationTail.then(async () =>
			this.#applyUpdate(canonical, preferencesSource),
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

	async #loadAndCommit(): Promise<DesktopTerminalDefaultSettingsSnapshot> {
		const loaded = await this.#store.load();
		const configuration =
			loaded.kind === 'available' ? loaded.configuration : this.#runtimeConfiguration;
		const accepted = TerminalConfigurationSchema.parse(await this.#setHostDefaults(configuration));
		if (!sameConfiguration(accepted, configuration)) {
			throw new Error('Terminal Host did not accept the requested terminal defaults.');
		}
		this.#snapshot = DesktopTerminalDefaultSettingsSnapshotSchema.parse({
			configuration,
			persistenceIssue: loaded.kind === 'unavailable' ? loaded.reason : null,
			source: loaded.kind === 'available' ? 'stored' : 'runtime',
		});
		return this.#snapshot;
	}

	async #applyUpdate(
		configuration: TerminalConfiguration,
		preferencesSource?: string,
	): Promise<DesktopTerminalDefaultSettingsMutationResult> {
		const snapshot = this.getSnapshot();
		if (this.#userPreferences !== undefined) {
			if (preferencesSource === undefined || preferencesSource !== snapshot.preferencesSource)
				return { kind: 'failed', code: 'source-changed', snapshot };
			const { response, applicationIssue } = await this.#userPreferences.execute({
				operation: 'update',
				source: preferencesSource,
				changes: { scrollbackBytes: configuration.scrollbackBytes },
			});
			if (response.kind === 'snapshot')
				return applicationIssue === null
					? { kind: 'completed', snapshot: this.getSnapshot() }
					: { kind: 'failed', code: 'host-rejected', snapshot: this.getSnapshot() };
			let code: 'outcome-unknown' | 'source-changed' | 'busy' | 'write-failed' = 'write-failed';
			if (response.kind === 'outcome-unknown') code = 'outcome-unknown';
			else if (response.reason === 'source-changed' || response.reason === 'busy')
				code = response.reason;
			return { kind: 'failed', code, snapshot: this.getSnapshot() };
		}
		let accepted: TerminalConfiguration;
		try {
			accepted = TerminalConfigurationSchema.parse(await this.#setHostDefaults(configuration));
		} catch {
			return { code: 'host-rejected', kind: 'failed', snapshot };
		}
		if (!sameConfiguration(accepted, configuration)) {
			return { code: 'host-rejected', kind: 'failed', snapshot };
		}
		try {
			await this.#store.replace(configuration);
		} catch {
			this.#snapshot = DesktopTerminalDefaultSettingsSnapshotSchema.parse({
				configuration,
				persistenceIssue: 'write-failed',
				source: 'session',
			});
			return { kind: 'completed', snapshot: this.#snapshot };
		}
		this.#snapshot = DesktopTerminalDefaultSettingsSnapshotSchema.parse({
			configuration,
			persistenceIssue: null,
			source: 'stored',
		});
		return { kind: 'completed', snapshot: this.#snapshot };
	}
}

function sameConfiguration(left: TerminalConfiguration, right: TerminalConfiguration): boolean {
	return left.scrollbackBytes === right.scrollbackBytes;
}
