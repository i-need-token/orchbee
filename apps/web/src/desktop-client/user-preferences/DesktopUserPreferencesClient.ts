import {
	DesktopUserPreferencesCommandSchema,
	DesktopUserPreferencesEventSchema,
} from '@orchbee/desktop-client-protocol';
import {
	UserPreferencesPatchSchema,
	type UserPreferenceKey,
	type UserPreferencesPatch,
	type UserPreferencesRequest,
	type UserPreferencesResponse,
} from '@orchbee/workbench-client-protocol';

import type { UserPreferencesControl, UserPreferencesState } from '../../user-preferences';

interface Pending {
	readonly id: number;
	readonly writing: boolean;
	readonly timer: ReturnType<typeof setTimeout>;
	resolve(response: UserPreferencesResponse): void;
}

export class DesktopUserPreferencesClient implements UserPreferencesControl {
	readonly #port: MessagePort;
	readonly #listeners = new Set<() => void>();
	#state: UserPreferencesState = {
		snapshot: null,
		pending: false,
		available: false,
		failure: null,
		applicationIssue: null,
		draftResetRevision: 0,
	};
	#pending: Pending | undefined;
	#sequence = 0;
	#disposed = false;
	#retainFailure = false;

	constructor(port: MessagePort) {
		this.#port = port;
		port.addEventListener('message', this.#handleMessage);
		port.addEventListener('messageerror', this.#handleError);
		port.start();
	}

	getSnapshot = (): UserPreferencesState => this.#state;
	subscribe = (listener: () => void): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	async refresh(): Promise<UserPreferencesResponse> {
		const response = await this.#request({ operation: 'read' });
		if (!this.#disposed && response.kind === 'snapshot')
			this.#publish({ ...this.#state, draftResetRevision: this.#state.draftResetRevision + 1 });
		return response;
	}

	update(changes: UserPreferencesPatch, source?: string): Promise<UserPreferencesResponse> {
		if (this.#disposed) return Promise.resolve({ kind: 'refused', reason: 'unavailable' });
		const snapshot = this.#state.snapshot;
		if (snapshot === null || !this.#state.available) {
			this.#publish({ ...this.#state, failure: 'unavailable' });
			return Promise.resolve({ kind: 'refused', reason: 'unavailable' });
		}
		return this.#request({ operation: 'update', source: source ?? snapshot.source, changes });
	}

	reset(keys: readonly UserPreferenceKey[]): Promise<UserPreferencesResponse> {
		return this.update(
			UserPreferencesPatchSchema.parse(Object.fromEntries(keys.map((key) => [key, null]))),
		);
	}

	dispose(): void {
		this.#retire();
	}

	#request(request: UserPreferencesRequest): Promise<UserPreferencesResponse> {
		if (this.#disposed) return Promise.resolve({ kind: 'refused', reason: 'unavailable' });
		if (this.#pending !== undefined) {
			return Promise.resolve({ kind: 'refused', reason: 'busy' });
		}
		const parsed = DesktopUserPreferencesCommandSchema.safeParse({
			type: 'desktop-user-preferences-requested',
			requestId: ++this.#sequence,
			request,
		});
		if (!parsed.success) return Promise.resolve({ kind: 'refused', reason: 'invalid-changes' });
		const command = parsed.data;
		const result = new Promise<UserPreferencesResponse>((resolve) => {
			this.#pending = {
				id: command.requestId,
				writing: request.operation !== 'read',
				resolve,
				timer: setTimeout(() => this.#retire(), 5000),
			};
		});
		this.#publish({ ...this.#state, pending: true, failure: null });
		try {
			this.#port.postMessage(command);
		} catch {
			this.#retire();
		}
		return result;
	}

	readonly #handleError = (): void => {
		this.#retire();
	};
	readonly #handleMessage = (event: MessageEvent<unknown>): void => {
		if (this.#disposed) return;
		const parsed = DesktopUserPreferencesEventSchema.safeParse(event.data);
		if (event.ports.length !== 0 || !parsed.success) {
			this.#retire();
			return;
		}
		const { requestId, response, applicationIssue } = parsed.data;
		const pending = this.#pending;
		if (requestId !== null && (pending === undefined || pending.id !== requestId)) {
			this.#retire();
			return;
		}
		if (requestId !== null) {
			clearTimeout(pending?.timer);
			this.#pending = undefined;
		}
		let failure: UserPreferencesState['failure'] = null;
		if (response.kind === 'outcome-unknown') failure = 'outcome-unknown';
		else if (response.kind === 'refused') failure = response.reason;
		if (requestId === null && this.#retainFailure) failure = this.#state.failure;
		else if (requestId !== null)
			this.#retainFailure = pending?.writing === true && response.kind !== 'snapshot';
		this.#publish({
			...this.#state,
			pending: this.#pending !== undefined,
			snapshot: response.kind === 'snapshot' ? response.snapshot : this.#state.snapshot,
			available: response.kind === 'snapshot',
			applicationIssue,
			// Polls clear transient read failures, but retain failed/uncertain write receipts.
			failure,
		});
		if (requestId !== null) pending?.resolve(response);
	};

	#retire(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.removeEventListener('message', this.#handleMessage);
		this.#port.removeEventListener('messageerror', this.#handleError);
		this.#port.close();
		const pending = this.#pending;
		clearTimeout(pending?.timer);
		this.#pending = undefined;
		const response: UserPreferencesResponse =
			pending?.writing === true
				? { kind: 'outcome-unknown' }
				: { kind: 'refused', reason: 'unavailable' };
		this.#publish({
			...this.#state,
			pending: false,
			available: false,
			failure: response.kind === 'outcome-unknown' ? 'outcome-unknown' : 'unavailable',
		});
		pending?.resolve(response);
		this.#listeners.clear();
	}

	#publish(state: UserPreferencesState): void {
		this.#state = state;
		for (const listener of new Set(this.#listeners)) {
			if (!this.#listeners.has(listener)) continue;
			try {
				listener();
			} catch {
				/* Isolate application preference consumers. */
			}
		}
	}
}
