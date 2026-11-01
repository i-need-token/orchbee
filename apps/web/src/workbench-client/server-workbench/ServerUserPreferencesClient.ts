import {
	UserPreferencesPatchSchema,
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
 * The Session's shared preferences, through its Server. The Server publishes no preference changes,
 * so another Client's save appears at the next refresh, and a save that raced it is refused as
 * `source-changed` rather than overwriting it.
 */
export class ServerUserPreferencesClient implements UserPreferencesControl {
	readonly #connection: Pick<ServerConnection, 'exchange'>;
	readonly #listeners = new Set<() => void>();
	readonly #sessionId: string;
	#disposed = false;
	#requesting = false;
	#state: UserPreferencesState = {
		applicationIssue: null,
		available: false,
		draftResetRevision: 0,
		failure: null,
		pending: false,
		snapshot: null,
	};

	constructor(connection: Pick<ServerConnection, 'exchange'>, sessionId: string) {
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
		if (!this.#disposed && response.kind === 'snapshot') {
			this.#publish({ ...this.#state, draftResetRevision: this.#state.draftResetRevision + 1 });
		}
		return response;
	}

	update(changes: UserPreferencesPatch, source?: string): Promise<UserPreferencesResponse> {
		if (this.#disposed) return Promise.resolve({ kind: 'refused', reason: 'unavailable' });
		const { available, snapshot } = this.#state;
		if (snapshot === null || !available) {
			this.#publish({ ...this.#state, failure: 'unavailable' });
			return Promise.resolve({ kind: 'refused', reason: 'unavailable' });
		}
		return this.#request({ changes, kind: 'save', source: source ?? snapshot.source });
	}

	reset(keys: readonly UserPreferenceKey[]): Promise<UserPreferencesResponse> {
		return this.update(
			UserPreferencesPatchSchema.parse(Object.fromEntries(keys.map((key) => [key, null]))),
		);
	}

	dispose(): void {
		this.#disposed = true;
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
		this.#publish({
			...this.#state,
			available: response.kind === 'snapshot',
			failure,
			pending: false,
			snapshot: response.kind === 'snapshot' ? response.snapshot : this.#state.snapshot,
		});
		return response;
	}

	async #exchange(operation: SettingsOperation): Promise<UserPreferencesResponse> {
		const requestId = createWorkbenchRequestId();
		let request: ReturnType<typeof parseWorkbenchRequest>;
		try {
			request = parseWorkbenchRequest({
				kind: 'workbench.settings',
				operation,
				protocolVersion: WORKBENCH_PROTOCOL_VERSION,
				requestId,
				sessionId: this.#sessionId,
			});
		} catch {
			return { kind: 'refused', reason: 'invalid-changes' };
		}
		if (request.kind !== 'workbench.settings')
			return { kind: 'refused', reason: 'invalid-changes' };
		const unanswered: UserPreferencesResponse =
			operation.kind === 'save'
				? { kind: 'outcome-unknown' }
				: { kind: 'refused', reason: 'unavailable' };
		let record: Uint8Array;
		try {
			record = await this.#connection.exchange(encodeWorkbenchRequest(request), {
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
