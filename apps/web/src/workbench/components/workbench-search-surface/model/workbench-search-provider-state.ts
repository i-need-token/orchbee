import type { WorkbenchSearchFilter } from '../../../types/workbench-search';
import type { WorkbenchSearchSection } from './workbench-search-result';

export interface WorkbenchSearchProviderRequest {
	readonly filter: WorkbenchSearchFilter;
	readonly query: string;
	readonly queryRevision: number;
	readonly sessionId: number;
}

interface WorkbenchSearchProviderRunIdentity {
	readonly attempt: number;
	readonly providerId: string;
	readonly queryRevision: number;
	readonly sessionId: number;
}

export type WorkbenchSearchProviderRun =
	| (WorkbenchSearchProviderRunIdentity & {
			readonly phase: 'loading';
	  })
	| (WorkbenchSearchProviderRunIdentity & {
			readonly phase: 'ready';
			readonly sections: readonly WorkbenchSearchSection[];
	  })
	| (WorkbenchSearchProviderRunIdentity & {
			readonly failureCode: string;
			readonly phase: 'failed';
	  });

export interface WorkbenchSearchProviderState {
	readonly request: WorkbenchSearchProviderRequest | null;
	readonly runs: readonly WorkbenchSearchProviderRun[];
}

export type WorkbenchSearchProviderEvent =
	| {
			readonly providerIds: readonly string[];
			readonly request: WorkbenchSearchProviderRequest;
			readonly type: 'request-changed';
	  }
	| (WorkbenchSearchProviderRunIdentity & {
			readonly sections: readonly WorkbenchSearchSection[];
			readonly type: 'provider-completed';
	  })
	| (WorkbenchSearchProviderRunIdentity & {
			readonly failureCode: string;
			readonly type: 'provider-failed';
	  })
	| {
			readonly providerId: string;
			readonly type: 'retry-requested' | 'provider-model-changed';
	  }
	| { readonly type: 'closed' };

export function createWorkbenchSearchProviderState(): WorkbenchSearchProviderState {
	return { request: null, runs: [] };
}

export function transitionWorkbenchSearchProviderState(
	state: WorkbenchSearchProviderState,
	event: WorkbenchSearchProviderEvent,
): WorkbenchSearchProviderState {
	switch (event.type) {
		case 'request-changed': {
			const providerIds = uniqueProviderIds(event.providerIds);
			return {
				request: event.request,
				runs: providerIds.map((providerId) => ({
					attempt: 0,
					phase: 'loading',
					providerId,
					queryRevision: event.request.queryRevision,
					sessionId: event.request.sessionId,
				})),
			};
		}
		case 'provider-completed':
			return replaceMatchingLoadingRun(state, event, (run) => ({
				...run,
				phase: 'ready',
				sections: event.sections,
			}));
		case 'provider-failed':
			return replaceMatchingLoadingRun(state, event, (run) => ({
				...run,
				failureCode: event.failureCode,
				phase: 'failed',
			}));
		case 'retry-requested':
			return restartRun(state, event.providerId, true);
		case 'provider-model-changed':
			return restartRun(state, event.providerId, false);
		case 'closed':
			return state.request === null ? state : createWorkbenchSearchProviderState();
	}
}

function uniqueProviderIds(providerIds: readonly string[]): readonly string[] {
	const seen = new Set<string>();
	const uniqueIds: string[] = [];
	for (const providerId of providerIds) {
		if (seen.has(providerId)) {
			continue;
		}
		seen.add(providerId);
		uniqueIds.push(providerId);
	}
	return uniqueIds;
}

function replaceMatchingLoadingRun(
	state: WorkbenchSearchProviderState,
	identity: WorkbenchSearchProviderRunIdentity,
	replace: (
		run: Extract<WorkbenchSearchProviderRun, { readonly phase: 'loading' }>,
	) => WorkbenchSearchProviderRun,
): WorkbenchSearchProviderState {
	if (
		state.request?.sessionId !== identity.sessionId ||
		state.request.queryRevision !== identity.queryRevision
	) {
		return state;
	}
	const runIndex = state.runs.findIndex(
		(run) =>
			run.phase === 'loading' &&
			run.providerId === identity.providerId &&
			run.sessionId === identity.sessionId &&
			run.queryRevision === identity.queryRevision &&
			run.attempt === identity.attempt,
	);
	const run = state.runs[runIndex];
	if (runIndex === -1 || run?.phase !== 'loading') {
		return state;
	}
	const runs = state.runs.slice();
	runs[runIndex] = replace(run);
	return { ...state, runs };
}

function restartRun(
	state: WorkbenchSearchProviderState,
	providerId: string,
	requireFailure: boolean,
): WorkbenchSearchProviderState {
	const runIndex = state.runs.findIndex((run) => run.providerId === providerId);
	const run = state.runs[runIndex];
	if (state.request === null || run === undefined || (requireFailure && run.phase !== 'failed')) {
		return state;
	}
	const runs = state.runs.slice();
	runs[runIndex] = {
		attempt: run.attempt + 1,
		phase: 'loading',
		providerId: run.providerId,
		queryRevision: state.request.queryRevision,
		sessionId: state.request.sessionId,
	};
	return { ...state, runs };
}
