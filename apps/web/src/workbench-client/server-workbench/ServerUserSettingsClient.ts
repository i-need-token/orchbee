import {
	WORKBENCH_PROTOCOL_VERSION,
	parseWorkbenchRequest,
	type SettingChange,
	type SettingKey,
	type SettingsSnapshot,
	type SettingValues,
	type WorkbenchResponse,
} from '@orchbee/workbench-client-protocol';

import {
	settingChange,
	type UserSettingsControl,
	type UserSettingsFailure,
	type UserSettingsResult,
	type UserSettingsState,
} from '../../user-settings';
import {
	type ServerConnection,
	type ServerSubscription,
	ServerStreamRefusedError,
	decodeWorkbenchResponse,
	encodeWorkbenchRequest,
} from '../server-connection';
import { createWorkbenchRequestId } from './createWorkbenchRequestId';

type SettingsOutcome = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.settings.result' }
>['outcome'];
type SettingsOperation =
	| { readonly kind: 'read' }
	| { readonly kind: 'edit'; readonly changes: readonly SettingChange[] };

const REQUEST_DEADLINE_MILLISECONDS = 5_000;

function readOutcome(outcome: SettingsOutcome): UserSettingsResult {
	switch (outcome.outcome) {
		case 'current':
			return { kind: 'current', snapshot: outcome.snapshot };
		case 'edited':
			return { changed: outcome.changed, kind: 'edited', snapshot: outcome.snapshot };
		case 'refused':
			return { kind: 'refused', refusal: outcome };
	}
}

/** Revisions are decimal u64 strings, so they compare as integers, never as text. */
function isNewer(candidate: SettingsSnapshot, current: SettingsSnapshot | null): boolean {
	return current === null || BigInt(candidate.revision) > BigInt(current.revision);
}

/**
 * The settings file on the Session's Server machine. One read answers at once and a State stream
 * of that read then carries every later snapshot: another Client's edit and an edit made in the
 * file itself. Revisions order snapshots, so an older one arriving late is dropped. Edits run one
 * at a time in the order they were asked for.
 */
export class ServerUserSettingsClient implements UserSettingsControl {
	readonly #connection: Pick<ServerConnection, 'exchange' | 'subscribe'>;
	readonly #listeners = new Set<() => void>();
	readonly #sessionId: string;
	#disposed = false;
	#inFlight = 0;
	#observation: ServerSubscription | null = null;
	#queue: Promise<unknown> = Promise.resolve();
	#state: UserSettingsState = {
		available: false,
		failure: null,
		pending: false,
		snapshot: null,
	};

	constructor(connection: Pick<ServerConnection, 'exchange' | 'subscribe'>, sessionId: string) {
		this.#connection = connection;
		this.#sessionId = sessionId;
	}

	getSnapshot = (): UserSettingsState => this.#state;

	subscribe = (listener: () => void): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	refresh(): Promise<UserSettingsResult> {
		return this.#enqueue({ kind: 'read' });
	}

	set<Key extends SettingKey>(key: Key, value: SettingValues[Key]): Promise<UserSettingsResult> {
		return this.edit([settingChange(key, value)]);
	}

	restore(keys: readonly SettingKey[]): Promise<UserSettingsResult> {
		return this.edit(keys.map((key) => ({ key, restore: true })));
	}

	edit(changes: readonly SettingChange[]): Promise<UserSettingsResult> {
		if (changes.length === 0) {
			const { snapshot } = this.#state;
			return Promise.resolve(
				snapshot === null ? { kind: 'unavailable' } : { kind: 'current', snapshot },
			);
		}
		return this.#enqueue({ changes, kind: 'edit' });
	}

	dispose(): void {
		this.#disposed = true;
		this.#observation?.close();
		this.#observation = null;
		this.#listeners.clear();
	}

	#enqueue(operation: SettingsOperation): Promise<UserSettingsResult> {
		if (this.#disposed) return Promise.resolve({ kind: 'unavailable' });
		this.#inFlight += 1;
		this.#publish({ ...this.#state, pending: true });
		const result = this.#queue.then(() => this.#request(operation));
		this.#queue = result;
		return result;
	}

	async #request(operation: SettingsOperation): Promise<UserSettingsResult> {
		const keys = operation.kind === 'edit' ? operation.changes.map((change) => change.key) : [];
		const result = this.#disposed
			? { kind: 'unavailable' as const }
			: await this.#exchange(operation);
		this.#inFlight -= 1;
		if (this.#disposed) return result;
		const pending = this.#inFlight > 0;
		switch (result.kind) {
			case 'current':
			case 'edited':
				this.#publish({
					available: this.#observation !== null || operation.kind === 'read',
					failure: null,
					pending,
					snapshot: isNewer(result.snapshot, this.#state.snapshot)
						? result.snapshot
						: this.#state.snapshot,
				});
				// Observing starts after the first answer, so the stream never shows an older file.
				if (this.#observation === null) this.#observe();
				break;
			case 'refused':
				this.#publish({
					...this.#state,
					failure: { keys, kind: 'refused', refusal: result.refusal },
					pending,
				});
				break;
			case 'outcome-unknown':
			case 'unavailable':
				this.#publish({
					...this.#state,
					// A failed edit leaves the stream to report whether the Server is gone.
					available: operation.kind === 'edit' && this.#state.available,
					failure: { keys, kind: result.kind } satisfies UserSettingsFailure,
					pending,
				});
				break;
		}
		return result;
	}

	async #exchange(operation: SettingsOperation): Promise<UserSettingsResult> {
		const requestId = createWorkbenchRequestId();
		const request = this.#settingsRequest(operation, requestId);
		if (request === null) return { kind: 'unavailable' };
		const unanswered: UserSettingsResult =
			operation.kind === 'edit' ? { kind: 'outcome-unknown' } : { kind: 'unavailable' };
		let record: Uint8Array;
		try {
			record = await this.#connection.exchange(request, {
				timeoutMilliseconds: REQUEST_DEADLINE_MILLISECONDS,
			});
		} catch (error) {
			return error instanceof ServerStreamRefusedError ? { kind: 'unavailable' } : unanswered;
		}
		try {
			const response = decodeWorkbenchResponse(record);
			return response.kind === 'workbench.settings.result' && response.requestId === requestId
				? readOutcome(response.outcome)
				: unanswered;
		} catch {
			return unanswered;
		}
	}

	#observe(): void {
		const requestId = createWorkbenchRequestId();
		const request = this.#settingsRequest({ kind: 'read' }, requestId);
		if (this.#disposed || request === null) return;
		let observation: ServerSubscription;
		try {
			observation = this.#connection.subscribe('state', request, {
				onClosed: () => {
					if (this.#observation !== observation) return;
					this.#observation = null;
					// Changes would stop arriving unseen, so editing waits for a refresh to observe again.
					this.#publish({
						...this.#state,
						available: false,
						failure: { keys: [], kind: 'unavailable' },
					});
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
		if (!this.#state.available) this.#publish({ ...this.#state, available: true });
	}

	#observed(record: Uint8Array, requestId: string): void {
		let response: WorkbenchResponse;
		try {
			response = decodeWorkbenchResponse(record);
		} catch {
			return;
		}
		if (response.kind !== 'workbench.settings.result' || response.requestId !== requestId) return;
		const observed = readOutcome(response.outcome);
		if (observed.kind !== 'current' && observed.kind !== 'edited') return;
		if (!isNewer(observed.snapshot, this.#state.snapshot)) return;
		// A published file settles an edit whose answer was lost.
		const failure = this.#state.failure?.kind === 'outcome-unknown' ? null : this.#state.failure;
		this.#publish({ ...this.#state, available: true, failure, snapshot: observed.snapshot });
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

	#publish(state: UserSettingsState): void {
		this.#state = state;
		for (const listener of new Set(this.#listeners)) {
			if (!this.#listeners.has(listener)) continue;
			try {
				listener();
			} catch {
				/* Isolate settings consumers. */
			}
		}
	}
}
