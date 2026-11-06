import {
	UserPreferencesPatchSchema,
	DEFAULT_USER_PREFERENCES,
	USER_PREFERENCE_KEYS,
	type UserPreferencesSnapshot,
	WORKBENCH_PROTOCOL_VERSION,
	parseWorkbenchRequest,
	type UserPreferenceKey,
	type UserPreferencesPatch,
	type UserPreferencesResponse,
	type WorkbenchResponse,
} from '@orchbee/workbench-client-protocol';

import type { UserPreferencesControl, UserPreferencesState } from '../../user-preferences';
import {
	type ServerConnection,
	type ServerSubscription,
	ServerStreamRefusedError,
	decodeWorkbenchResponse,
	encodeWorkbenchRequest,
} from '../server-connection';
import { createWorkbenchRequestId } from './createWorkbenchRequestId';

type SettingsResult = Extract<WorkbenchResponse, { readonly kind: 'workbench.settings.result' }>;
type SettingsOperation =
	| { readonly kind: 'open' }
	| {
			readonly changes: UserPreferencesPatch;
			readonly kind: 'save';
			readonly source: string;
	  };

const REQUEST_DEADLINE_MILLISECONDS = 5_000;

/** A refusal this Client can do nothing about reads the same as a Server it cannot reach. */
function readResponse(outcome: SettingsResult['outcome']): UserPreferencesResponse {
	switch (outcome.kind) {
		case 'opened':
			return { changed: false, kind: 'snapshot', snapshot: outcome.snapshot };
		case 'saved':
			return { changed: outcome.changed, kind: 'snapshot', snapshot: outcome.snapshot };
		case 'outcome-unknown':
			return { kind: 'outcome-unknown' };
		case 'refused':
			switch (outcome.reason) {
				case 'denied':
				case 'response-too-large':
				case 'session-mismatch':
					return { kind: 'refused', reason: 'unavailable' };
				default:
					return { kind: 'refused', reason: outcome.reason };
			}
	}
}

/**
 * The Session's shared preferences, through its Server. As in the TUI, a State stream of the settings
 * read carries every later change: another Client's save, and a write the Server finds in the
 * preference file. A save that raced another is refused as `source-changed` rather than overwriting
 * it.
 */
export class ServerUserPreferencesClient implements UserPreferencesControl {
	readonly #connection: Pick<ServerConnection, 'exchange' | 'subscribe'>;
	readonly #listeners = new Set<() => void>();
	readonly #sessionId: string;
	#disposed = false;
	#observation: ServerSubscription | null = null;
	#requesting = false;
	/** A failed or uncertain save stays reported until the person refreshes, whatever arrives after. */
	#retainFailure = false;
	#state: UserPreferencesState = {
		applicationIssue: null,
		available: false,
		draftResetRevision: 0,
		failure: null,
		pending: false,
		snapshot: null,
	};

	constructor(connection: Pick<ServerConnection, 'exchange' | 'subscribe'>, sessionId: string) {
		this.#connection = connection;
		this.#sessionId = sessionId;
	}

	getSnapshot = (): UserPreferencesState => this.#state;

	subscribe = (listener: () => void): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	async refresh(): Promise<UserPreferencesResponse> {
		const response = await this.#request({ kind: 'open' });
		if (
			!this.#disposed &&
			(response.kind === 'snapshot' ||
				(response.kind === 'refused' && response.reason === 'invalid-source'))
		) {
			this.#publish({ ...this.#state, draftResetRevision: this.#state.draftResetRevision + 1 });
			// Observing starts after this answer, so the stream's first snapshot is never the older one.
			if (this.#observation === null) this.#observe();
		}
		return response;
	}

	update(changes: UserPreferencesPatch, source?: string): Promise<UserPreferencesResponse> {
		if (this.#disposed) return Promise.resolve({ kind: 'refused', reason: 'unavailable' });
		const { available, snapshot } = this.#state;
		if (snapshot === null || !available) {
			const reason = this.#state.failure === 'invalid-source' ? 'invalid-source' : 'unavailable';
			this.#publish({ ...this.#state, failure: reason });
			return Promise.resolve({ kind: 'refused', reason });
		}
		return this.#request({ changes, kind: 'save', source: source ?? snapshot.source });
	}

	reset(keys: readonly UserPreferenceKey[]): Promise<UserPreferencesResponse> {
		const changes = UserPreferencesPatchSchema.parse(
			Object.fromEntries(keys.map((key) => [key, null])),
		);
		if (
			this.#state.failure === 'invalid-source' &&
			Object.keys(changes).length === USER_PREFERENCE_KEYS.length
		) {
			return this.#request({ kind: 'save', source: 'unreadable', changes });
		}
		return this.update(changes);
	}

	dispose(): void {
		this.#disposed = true;
		this.#observation?.close();
		this.#observation = null;
		this.#listeners.clear();
	}

	async #request(operation: SettingsOperation): Promise<UserPreferencesResponse> {
		if (this.#disposed) return { kind: 'refused', reason: 'unavailable' };
		if (this.#requesting) return { kind: 'refused', reason: 'busy' };
		this.#requesting = true;
		this.#publish({ ...this.#state, failure: null, pending: true });
		const response = await this.#exchange(operation);
		this.#requesting = false;
		if (this.#disposed) return response;
		let failure: UserPreferencesState['failure'] = null;
		if (response.kind === 'outcome-unknown') failure = 'outcome-unknown';
		else if (response.kind === 'refused') failure = response.reason;
		this.#retainFailure = operation.kind === 'save' && response.kind !== 'snapshot';
		this.#publish({
			...this.#state,
			available: response.kind === 'snapshot',
			failure,
			pending: false,
			snapshot: observedSnapshot(response, this.#state.snapshot),
		});
		// A source is a digest, not an ordered revision, so a snapshot observed around this save may be
		// older than its answer. A new stream's first snapshot is read after the save and has the last word.
		if (operation.kind === 'save') this.#observe();
		return response;
	}

	async #exchange(operation: SettingsOperation): Promise<UserPreferencesResponse> {
		const requestId = createWorkbenchRequestId();
		const request = this.#settingsRequest(operation, requestId);
		if (request === null) return { kind: 'refused', reason: 'invalid-changes' };
		const unanswered: UserPreferencesResponse =
			operation.kind === 'save'
				? { kind: 'outcome-unknown' }
				: { kind: 'refused', reason: 'unavailable' };
		let record: Uint8Array;
		try {
			record = await this.#connection.exchange(request, {
				timeoutMilliseconds: REQUEST_DEADLINE_MILLISECONDS,
			});
		} catch (error) {
			return error instanceof ServerStreamRefusedError
				? { kind: 'refused', reason: 'unavailable' }
				: unanswered;
		}
		try {
			const response = decodeWorkbenchResponse(record);
			return response.kind === 'workbench.settings.result' && response.requestId === requestId
				? readResponse(response.outcome)
				: unanswered;
		} catch {
			return unanswered;
		}
	}

	#observe(): void {
		this.#observation?.close();
		this.#observation = null;
		const requestId = createWorkbenchRequestId();
		const request = this.#settingsRequest({ kind: 'open' }, requestId);
		if (this.#disposed || request === null) return;
		let observation: ServerSubscription;
		try {
			observation = this.#connection.subscribe('state', request, {
				onClosed: () => {
					if (this.#observation !== observation) return;
					this.#observation = null;
					// Changes would stop arriving unseen, so saving waits for a refresh to observe again.
					this.#publish({ ...this.#state, available: false, failure: 'unavailable' });
				},
				onFrame: (_kind, record) => {
					if (this.#observation === observation) this.#observed(record, requestId);
				},
				// Every frame is a complete snapshot, so a gap loses nothing.
				onGap: () => undefined,
			});
		} catch {
			// The connection has ended, which its Workbench session reports.
			return;
		}
		this.#observation = observation;
	}

	#observed(record: Uint8Array, requestId: string): void {
		let response: WorkbenchResponse;
		try {
			response = decodeWorkbenchResponse(record);
		} catch {
			return;
		}
		if (response.kind !== 'workbench.settings.result' || response.requestId !== requestId) return;
		const observed = readResponse(response.outcome);
		if (observed.kind === 'outcome-unknown') return;
		const available = observed.kind === 'snapshot';
		const snapshot = observedSnapshot(observed, this.#state.snapshot);
		let failure: UserPreferencesState['failure'] = null;
		if (this.#retainFailure) failure = this.#state.failure;
		else if (observed.kind === 'refused') failure = observed.reason;
		// A source names its record's content, so an unchanged source is the snapshot already shown.
		if (
			available === this.#state.available &&
			failure === this.#state.failure &&
			snapshot?.source === this.#state.snapshot?.source
		) {
			return;
		}
		this.#publish({ ...this.#state, available, failure, snapshot });
	}

	#settingsRequest(operation: SettingsOperation, requestId: string): Uint8Array | null {
		try {
			const request = parseWorkbenchRequest({
				kind: 'workbench.settings',
				operation,
				protocolVersion: WORKBENCH_PROTOCOL_VERSION,
				requestId,
				sessionId: this.#sessionId,
			});
			return request.kind === 'workbench.settings' ? encodeWorkbenchRequest(request) : null;
		} catch {
			return null;
		}
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

/** A refused read has a virtual recovery basis, never an ordinary save source. */
function observedSnapshot(
	response: UserPreferencesResponse,
	previous: UserPreferencesSnapshot | null,
): UserPreferencesSnapshot | null {
	if (response.kind === 'snapshot') return response.snapshot;
	if (response.kind === 'refused' && response.reason === 'invalid-source') {
		return { source: 'unreadable', values: { ...DEFAULT_USER_PREFERENCES }, claimed: [] };
	}
	return previous;
}
