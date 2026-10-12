import type { DesktopUserPreferencesEvent } from '@orchbee/desktop-client-protocol';
import type {
	UserPreferencesRequest,
	UserPreferencesResponse,
	UserPreferencesSnapshot,
} from '@orchbee/workbench-client-protocol';

import type { NativeUserPreferencesClient } from './NativeUserPreferencesClient';

export type DesktopUserPreferencesObservation = Pick<
	DesktopUserPreferencesEvent,
	'response' | 'applicationIssue' | 'legacyIssue'
>;

/** One device authority; polling observes other writers and never replays a mutation. */
export class DesktopUserPreferencesController {
	readonly #client: Pick<NativeUserPreferencesClient, 'execute' | 'dispose'>;
	readonly #listeners = new Set<(observation: DesktopUserPreferencesObservation) => void>();
	#snapshot: UserPreferencesSnapshot | undefined;
	#active: Promise<DesktopUserPreferencesObservation> | undefined;
	#reading = false;
	#timer: ReturnType<typeof setTimeout> | undefined;
	#disposed = false;
	#apply: ((snapshot: UserPreferencesSnapshot) => Promise<void>) | undefined;
	#applicationIssue: DesktopUserPreferencesObservation['applicationIssue'] = null;
	#appliedSource: string | undefined;
	#legacyUnavailable = false;

	constructor(client: Pick<NativeUserPreferencesClient, 'execute' | 'dispose'>) {
		this.#client = client;
	}

	getSnapshot(): UserPreferencesSnapshot | undefined {
		return this.#snapshot;
	}
	noteLegacyUnavailable(): void {
		this.#legacyUnavailable = true;
	}

	subscribe(listener: (observation: DesktopUserPreferencesObservation) => void): () => void {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	async start(apply: (snapshot: UserPreferencesSnapshot) => Promise<void>): Promise<void> {
		this.#apply = apply;
		await this.execute({ operation: 'read' });
		this.#schedulePoll();
	}

	execute(request: UserPreferencesRequest): Promise<DesktopUserPreferencesObservation> {
		if (this.#disposed)
			return Promise.resolve(this.#observation({ kind: 'refused', reason: 'unavailable' }));
		if (this.#active !== undefined) {
			if (this.#reading && request.operation === 'read') return this.#active;
			if (this.#reading || request.operation === 'read') {
				return this.#active.then(() => this.execute(request));
			}
			return Promise.resolve(this.#observation({ kind: 'refused', reason: 'busy' }));
		}
		this.#reading = request.operation === 'read';
		const operation = this.#exchange(request).finally(() => {
			if (this.#active === operation) this.#active = undefined;
		});
		this.#active = operation;
		return operation;
	}

	async dispose(): Promise<void> {
		this.#disposed = true;
		clearTimeout(this.#timer);
		this.#listeners.clear();
		await this.#client.dispose();
		await this.#active;
	}

	async #exchange(request: UserPreferencesRequest): Promise<DesktopUserPreferencesObservation> {
		const exchange = await this.#client.execute(request);
		let response: UserPreferencesResponse = { kind: 'refused', reason: 'unavailable' };
		if (exchange.kind === 'response') response = exchange.response;
		else if (exchange.kind === 'outcome-unknown') response = { kind: 'outcome-unknown' };
		if (this.#disposed) return this.#observation(response);
		if (response.kind === 'snapshot') {
			this.#snapshot = response.snapshot;
			if (this.#apply !== undefined && this.#appliedSource !== response.snapshot.source) {
				try {
					await this.#apply(response.snapshot);
					this.#appliedSource = response.snapshot.source;
					this.#applicationIssue = null;
				} catch {
					this.#applicationIssue = 'host-rejected';
				}
			}
		}
		const observation = this.#observation(response);
		for (const listener of new Set(this.#listeners)) {
			if (!this.#listeners.has(listener)) continue;
			try {
				listener(observation);
			} catch {
				/* Isolate renderer lifetimes. */
			}
		}
		return observation;
	}

	#observation(response: UserPreferencesResponse): DesktopUserPreferencesObservation {
		return {
			response,
			applicationIssue: this.#applicationIssue,
			legacyIssue: this.#legacyUnavailable && this.#snapshot?.desktopMainImported !== true,
		};
	}

	#schedulePoll(): void {
		if (this.#disposed) return;
		this.#timer = setTimeout(() => {
			if (this.#active !== undefined) {
				this.#schedulePoll();
				return;
			}
			void this.execute({ operation: 'read' }).finally(() => this.#schedulePoll());
		}, 1000);
		this.#timer.unref();
	}
}
