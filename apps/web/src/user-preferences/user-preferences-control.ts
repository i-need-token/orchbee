import type {
	UserPreferenceKey,
	UserPreferencesFailure,
	UserPreferencesPatch,
	UserPreferencesResponse,
	UserPreferencesSnapshot,
} from '@orchbee/workbench-client-protocol';

export interface UserPreferencesState {
	readonly snapshot: UserPreferencesSnapshot | null;
	readonly pending: boolean;
	readonly available: boolean;
	readonly failure: UserPreferencesFailure | 'outcome-unknown' | null;
	readonly applicationIssue: 'host-rejected' | null;
	readonly draftResetRevision: number;
}

/** Confirmed values only. A draft must keep its observed source until explicitly rebased. */
export interface UserPreferencesControl {
	getSnapshot(): UserPreferencesState;
	subscribe(listener: () => void): () => void;
	update(changes: UserPreferencesPatch, source?: string): Promise<UserPreferencesResponse>;
	reset(keys: readonly UserPreferenceKey[]): Promise<UserPreferencesResponse>;
	refresh(): Promise<UserPreferencesResponse>;
}
